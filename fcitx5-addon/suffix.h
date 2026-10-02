// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <cstddef>
#include <string_view>

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/utf8.h>

namespace snipexpand {

enum class SuffixResult {
    Match,
    InvalidUtf8,
    InvalidCursor,
    Selection,
    Mismatch,
};

inline bool isPasswordField(fcitx::CapabilityFlags flags) {
    return flags.test(fcitx::CapabilityFlag::Password);
}

inline bool sensitiveHintAllowed(fcitx::CapabilityFlags flags,
                                 bool allowSensitiveHint) {
    return allowSensitiveHint ||
           !flags.test(fcitx::CapabilityFlag::Sensitive);
}

inline bool fallbackPermitMatches(bool sameInputContext, bool unexpired,
                                  std::string_view expected,
                                  std::string_view replacement,
                                  std::string_view permittedExpected,
                                  std::string_view permittedReplacement) {
    return sameInputContext && unexpired && expected == permittedExpected &&
           replacement == permittedReplacement;
}

inline SuffixResult exactSuffix(std::string_view text, unsigned int cursor,
                                unsigned int anchor,
                                std::string_view expected) {
    if (cursor != anchor) {
        return SuffixResult::Selection;
    }
    if (expected.empty()) {
        const auto textLength = text.empty()
                                    ? 0
                                    : fcitx::utf8::lengthValidated(text);
        if (textLength == fcitx::utf8::INVALID_LENGTH) {
            return SuffixResult::InvalidUtf8;
        }
        return cursor <= textLength ? SuffixResult::Match
                                    : SuffixResult::InvalidCursor;
    }
    if (text.empty()) {
        return SuffixResult::Mismatch;
    }

    const auto textLength = fcitx::utf8::lengthValidated(text);
    const auto expectedLength = fcitx::utf8::lengthValidated(expected);
    if (textLength == fcitx::utf8::INVALID_LENGTH ||
        expectedLength == fcitx::utf8::INVALID_LENGTH) {
        return SuffixResult::InvalidUtf8;
    }
    if (cursor > textLength) {
        return SuffixResult::InvalidCursor;
    }
    if (cursor < expectedLength) {
        return SuffixResult::Mismatch;
    }

    const auto start = fcitx::utf8::nextNChar(
        text.begin(), static_cast<std::size_t>(cursor) - expectedLength);
    const auto end = fcitx::utf8::nextNChar(text.begin(), cursor);
    return std::string_view(start, end) == expected ? SuffixResult::Match
                                                    : SuffixResult::Mismatch;
}

} // namespace snipexpand
