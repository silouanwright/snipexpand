// SPDX-License-Identifier: GPL-3.0-or-later

#include "suffix.h"

#include <chrono>
#include <cstdint>
#include <limits>
#include <optional>
#include <stdexcept>
#include <string>

#include <fcitx-module/dbus/dbus_public.h>
#include <fcitx-utils/dbus/objectvtable.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/utf8.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontext.h>
#include <fcitx/instance.h>

#ifndef SNIPEXPAND_BUILD_ID
#define SNIPEXPAND_BUILD_ID "development"
#endif

namespace snipexpand {

constexpr char ObjectPath[] = "/io/github/silouanwright/SnipExpand";
constexpr char Interface[] = "io.github.silouanwright.SnipExpand.Fcitx5";
constexpr std::size_t MaxReplacementBytes = 4000;
constexpr auto FallbackPermitLifetime = std::chrono::milliseconds(100);

enum class ReplaceStatus : std::uint32_t {
    Committed = 0,
    InvalidRequest = 1,
    NoFocus = 2,
    NoSurroundingText = 3,
    Selection = 4,
    TriggerMismatch = 5,
    PasswordField = 6,
    SensitiveHintSuppressed = 7,
};

class Bridge : public fcitx::dbus::ObjectVTable<Bridge> {
public:
    explicit Bridge(fcitx::Instance *instance) : instance_(instance) {}

    std::string buildId() const { return SNIPEXPAND_BUILD_ID; }

    std::uint32_t replace(const std::string &expected,
                          const std::string &replacement) {
        return replaceWithPolicy(expected, replacement, false);
    }

    std::uint32_t replaceWithPolicy(const std::string &expected,
                                    const std::string &replacement,
                                    bool allowSensitiveHint) {
        pendingFallback_.reset();
        if (!validRequest(expected, replacement)) {
            return value(ReplaceStatus::InvalidRequest);
        }

        auto *inputContext = instance_->lastFocusedInputContext();
        if (inputContext == nullptr || !inputContext->hasFocus()) {
            return value(ReplaceStatus::NoFocus);
        }
        if (isPasswordField(inputContext->capabilityFlags())) {
            return value(ReplaceStatus::PasswordField);
        }
        if (!sensitiveHintAllowed(inputContext->capabilityFlags(),
                                  allowSensitiveHint)) {
            return value(ReplaceStatus::SensitiveHintSuppressed);
        }

        const auto &surrounding = inputContext->surroundingText();
        if (!inputContext->capabilityFlags().test(
                fcitx::CapabilityFlag::SurroundingText) ||
            !surrounding.isValid()) {
            pendingFallback_ = PendingFallback{
                inputContext->uuid(), expected, replacement,
                allowSensitiveHint,
                std::chrono::steady_clock::now() + FallbackPermitLifetime};
            return value(ReplaceStatus::NoSurroundingText);
        }
        switch (exactSuffix(surrounding.text(), surrounding.cursor(),
                            surrounding.anchor(), expected)) {
        case SuffixResult::Match:
            break;
        case SuffixResult::Selection:
            return value(ReplaceStatus::Selection);
        case SuffixResult::InvalidUtf8:
        case SuffixResult::InvalidCursor:
        case SuffixResult::Mismatch:
            return value(ReplaceStatus::TriggerMismatch);
        }

        return replaceVerified(inputContext, expected, replacement);
    }

    std::uint32_t replaceWithoutSurrounding(
        const std::string &expected, const std::string &replacement) {
        return replaceWithoutSurroundingWithPolicy(expected, replacement,
                                                   false);
    }

    std::uint32_t replaceWithoutSurroundingWithPolicy(
        const std::string &expected, const std::string &replacement,
        bool allowSensitiveHint) {
        auto permit = std::move(pendingFallback_);
        pendingFallback_.reset();
        if (!validRequest(expected, replacement)) {
            return value(ReplaceStatus::InvalidRequest);
        }

        auto *inputContext = instance_->lastFocusedInputContext();
        if (inputContext == nullptr || !inputContext->hasFocus()) {
            return value(ReplaceStatus::NoFocus);
        }
        if (isPasswordField(inputContext->capabilityFlags())) {
            return value(ReplaceStatus::PasswordField);
        }
        if (!sensitiveHintAllowed(inputContext->capabilityFlags(),
                                  allowSensitiveHint)) {
            return value(ReplaceStatus::SensitiveHintSuppressed);
        }
        if (!permit ||
            !fallbackPermitMatches(
                permit->inputContext == inputContext->uuid(),
                std::chrono::steady_clock::now() <= permit->expires, expected,
                replacement, permit->expected, permit->replacement) ||
            permit->allowSensitiveHint != allowSensitiveHint) {
            return value(ReplaceStatus::TriggerMismatch);
        }

        const auto &surrounding = inputContext->surroundingText();
        if (inputContext->capabilityFlags().test(
                fcitx::CapabilityFlag::SurroundingText) &&
            surrounding.isValid()) {
            switch (exactSuffix(surrounding.text(), surrounding.cursor(),
                                surrounding.anchor(), expected)) {
            case SuffixResult::Match:
                return replaceVerified(inputContext, expected, replacement);
            case SuffixResult::Selection:
                return value(ReplaceStatus::Selection);
            case SuffixResult::InvalidUtf8:
            case SuffixResult::InvalidCursor:
            case SuffixResult::Mismatch:
                return value(ReplaceStatus::TriggerMismatch);
            }
        }

        const auto characters = characterCount(expected);
        if (!characters) {
            return value(ReplaceStatus::InvalidRequest);
        }
        for (std::size_t index = 0; index < *characters; ++index) {
            inputContext->forwardKey(fcitx::Key(FcitxKey_BackSpace));
        }
        inputContext->commitString(replacement);
        return value(ReplaceStatus::Committed);
    }

private:
    struct PendingFallback {
        fcitx::ICUUID inputContext;
        std::string expected;
        std::string replacement;
        bool allowSensitiveHint;
        std::chrono::steady_clock::time_point expires;
    };

    static bool validRequest(const std::string &expected,
                             const std::string &replacement) {
        return expected.size() <= MaxReplacementBytes &&
               replacement.size() <= MaxReplacementBytes &&
               (replacement.empty() || fcitx::utf8::validate(replacement)) &&
               (expected.empty() || fcitx::utf8::validate(expected));
    }

    static std::optional<std::size_t>
    characterCount(const std::string &text) {
        const auto characters = text.empty()
                                    ? 0
                                    : fcitx::utf8::lengthValidated(text);
        if (characters == fcitx::utf8::INVALID_LENGTH ||
            characters >
                static_cast<std::size_t>(std::numeric_limits<int>::max())) {
            return std::nullopt;
        }
        return characters;
    }

    static std::uint32_t replaceVerified(fcitx::InputContext *inputContext,
                                         const std::string &expected,
                                         const std::string &replacement) {
        const auto characters = characterCount(expected);
        if (!characters) {
            return value(ReplaceStatus::InvalidRequest);
        }
        if (*characters != 0) {
            inputContext->deleteSurroundingText(-static_cast<int>(*characters),
                                                *characters);
        }
        inputContext->commitString(replacement);
        return value(ReplaceStatus::Committed);
    }

    static std::uint32_t value(ReplaceStatus status) {
        return static_cast<std::uint32_t>(status);
    }

    FCITX_OBJECT_VTABLE_METHOD(buildId, "BuildId", "", "s");
    FCITX_OBJECT_VTABLE_METHOD(replace, "Replace", "ss", "u");
    FCITX_OBJECT_VTABLE_METHOD(replaceWithPolicy, "ReplaceWithPolicy", "ssb",
                              "u");
    FCITX_OBJECT_VTABLE_METHOD(replaceWithoutSurrounding,
                              "ReplaceWithoutSurrounding", "ss", "u");
    FCITX_OBJECT_VTABLE_METHOD(replaceWithoutSurroundingWithPolicy,
                              "ReplaceWithoutSurroundingWithPolicy", "ssb",
                              "u");

    fcitx::Instance *instance_;
    std::optional<PendingFallback> pendingFallback_;
};

class Addon : public fcitx::AddonInstance {
public:
    explicit Addon(fcitx::AddonManager *manager)
        : dbus_(manager->addon("dbus", true)), bridge_(manager->instance()) {
        if (dbus_ == nullptr) {
            throw std::runtime_error("Fcitx5 D-Bus addon is unavailable");
        }
        bus_ = dbus_->call<fcitx::IDBusModule::bus>();
        if (bus_ == nullptr || !bus_->addObjectVTable(ObjectPath, Interface,
                                                       bridge_)) {
            throw std::runtime_error("could not register SnipExpand D-Bus bridge");
        }
    }

private:
    fcitx::AddonInstance *dbus_;
    fcitx::dbus::Bus *bus_ = nullptr;
    Bridge bridge_;
};

class Factory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new Addon(manager);
    }
};

} // namespace snipexpand

FCITX_ADDON_FACTORY(snipexpand::Factory);
