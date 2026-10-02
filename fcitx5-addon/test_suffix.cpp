// SPDX-License-Identifier: GPL-3.0-or-later

#include "suffix.h"

#include <cassert>
#include <string>

using snipexpand::SuffixResult;
using snipexpand::exactSuffix;
using snipexpand::fallbackPermitMatches;
using snipexpand::isPasswordField;
using snipexpand::sensitiveHintAllowed;

int main() {
    assert(exactSuffix("before ;sm", 10, 10, ";sm") == SuffixResult::Match);
    assert(exactSuffix("before ;🙂", 9, 9, ";🙂") ==
           SuffixResult::Match);
    assert(exactSuffix("before ;sm", 10, 10, ";mail") ==
           SuffixResult::Mismatch);
    assert(exactSuffix("before ;sm", 10, 9, ";sm") ==
           SuffixResult::Selection);
    assert(exactSuffix("abc", 4, 4, "abc") == SuffixResult::InvalidCursor);
    assert(exactSuffix("before ", 7, 7, "") == SuffixResult::Match);

    assert(isPasswordField(fcitx::CapabilityFlag::Password));
    assert(!isPasswordField(fcitx::CapabilityFlag::Sensitive));
    assert(!isPasswordField(fcitx::CapabilityFlag::SurroundingText));
    assert(sensitiveHintAllowed(fcitx::CapabilityFlag::Sensitive, true));
    assert(!sensitiveHintAllowed(fcitx::CapabilityFlag::Sensitive, false));
    assert(sensitiveHintAllowed(fcitx::CapabilityFlag::SurroundingText,
                                false));

    assert(fallbackPermitMatches(true, true, ";sm", "🙂", ";sm", "🙂"));
    assert(!fallbackPermitMatches(false, true, ";sm", "🙂", ";sm", "🙂"));
    assert(!fallbackPermitMatches(true, false, ";sm", "🙂", ";sm", "🙂"));
    assert(!fallbackPermitMatches(true, true, ";sm", "🙂", ";mail", "🙂"));
    assert(!fallbackPermitMatches(true, true, ";sm", "🙂", ";sm", "🧐"));

    const std::string invalid("\xff", 1);
    assert(exactSuffix(invalid, 1, 1, invalid) == SuffixResult::InvalidUtf8);
}
