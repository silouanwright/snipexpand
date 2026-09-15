#!/bin/sh
set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH

# Called with the bundled rule as a literal argument, without shell interpolation.
rule=/etc/udev/rules.d/71-snipexpand-keyboard.rules
if [ -e "$rule" ] || [ -L "$rule" ]; then
    if ! printf '%s' "$1" | cmp -s - "$rule"; then
        printf 'Refusing to overwrite a different existing rule: %s\n' "$rule" >&2
        exit 1
    fi
else
    printf '%s' "$1" | install -Dm644 /dev/stdin "$rule"
fi
udevadm control --reload-rules
udevadm trigger --action=change --subsystem-match=input --property-match=ID_INPUT_KEYBOARD=1 --settle
