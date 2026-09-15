use std::io::IsTerminal;
use std::process::Command;

use anyhow::Context;

const KEYBOARD_RULE: &str = include_str!("../contrib/71-snipexpand-keyboard.rules");
const INSTALL_SCRIPT: &str = include_str!("../contrib/install-keyboard-access.sh");

pub fn install_keyboard_access() -> anyhow::Result<()> {
    let authorization = if std::io::stdin().is_terminal() {
        "sudo"
    } else {
        "pkexec"
    };
    println!("Installing a keyboard-only udev rule for the active local desktop session.");
    println!("Administrator authentication is required; input-group membership is unchanged.");
    let status = Command::new(authorization)
        .args([
            "/bin/sh",
            "-c",
            INSTALL_SCRIPT,
            "snipexpand-keyboard-access",
            KEYBOARD_RULE,
        ])
        .status()
        .with_context(|| {
            format!(
                "Could not run {authorization}; see the manual keyboard-access setup in the README"
            )
        })?;
    if !status.success() {
        anyhow::bail!("Keyboard permission setup failed ({status}). Check the authentication or udev error above and retry.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn rule_install_is_idempotent_and_preserves_existing_custom_rules() {
        let dir = tempfile::tempdir().unwrap();
        let rule = dir.path().join("keyboard.rules");
        let log = dir.path().join("udev-calls");
        let udev = dir.path().join("udevadm");
        std::fs::write(
            &udev,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$SNIPEXPAND_TEST_LOG\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&udev, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Exercise the bundled script with only its system destination and
        // udev executable redirected into an unprivileged test directory.
        let script = INSTALL_SCRIPT
            .replace(
                "rule=/etc/udev/rules.d/71-snipexpand-keyboard.rules",
                "rule=\"$2\"",
            )
            .replace("\nudevadm ", "\n\"$3\" ");
        let run = || {
            Command::new("/bin/sh")
                .args(["-c", &script, "test-keyboard-access", KEYBOARD_RULE])
                .arg(&rule)
                .arg(&udev)
                .env("SNIPEXPAND_TEST_LOG", &log)
                .output()
                .unwrap()
        };
        assert!(run().status.success());
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), KEYBOARD_RULE);
        assert_eq!(
            std::fs::metadata(&rule).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert!(run().status.success());
        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(calls.matches("control --reload-rules").count(), 2);
        assert_eq!(
            calls
                .matches("--property-match=ID_INPUT_KEYBOARD=1 --settle")
                .count(),
            2
        );

        std::fs::write(&rule, "# Custom administrator rule\n").unwrap();
        let conflict = run();
        assert!(!conflict.status.success());
        assert!(String::from_utf8_lossy(&conflict.stderr).contains("Refusing to overwrite"));
        assert_eq!(
            std::fs::read_to_string(&rule).unwrap(),
            "# Custom administrator rule\n"
        );
        assert_eq!(std::fs::read_to_string(&log).unwrap(), calls);
    }
}
