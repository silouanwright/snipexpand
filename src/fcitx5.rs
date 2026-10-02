use anyhow::{Context, Result};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SERVICE: &str = "org.fcitx.Fcitx5";
const OBJECT: &str = "/io/github/silouanwright/SnipExpand";
const INTERFACE: &str = "io.github.silouanwright.SnipExpand.Fcitx5";
const SURROUNDING_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(15);
const ADDON_SOURCE: &str = include_str!("../fcitx5-addon/snipexpand.cpp");
const SUFFIX_HEADER: &str = include_str!("../fcitx5-addon/suffix.h");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AddonInstallResult {
    Installed { loaded: bool },
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DirectCommitResult {
    Committed,
    NotCommitted(String),
    Suppressed(String),
    Indeterminate(String),
}

pub(crate) fn replace(
    expected: &str,
    replacement: &str,
    allow_sensitive_hint: bool,
) -> DirectCommitResult {
    replace_with(
        || {
            call_bridge(
                "ReplaceWithPolicy",
                expected,
                replacement,
                allow_sensitive_hint,
            )
        },
        || {
            call_bridge(
                "ReplaceWithoutSurroundingWithPolicy",
                expected,
                replacement,
                allow_sensitive_hint,
            )
        },
        SURROUNDING_RETRY_DELAY,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BridgeCallResult {
    Finished(DirectCommitResult),
    Retryable { kind: RetryKind, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryKind {
    NoSurroundingText,
    TriggerMismatch,
}

fn replace_with(
    mut replace_exact: impl FnMut() -> BridgeCallResult,
    mut replace_without_surrounding: impl FnMut() -> BridgeCallResult,
    retry_delay: std::time::Duration,
) -> DirectCommitResult {
    match replace_exact() {
        BridgeCallResult::Finished(result) => result,
        BridgeCallResult::Retryable { kind, .. } => {
            std::thread::sleep(retry_delay);
            let retry = match kind {
                RetryKind::NoSurroundingText => replace_without_surrounding(),
                RetryKind::TriggerMismatch => replace_exact(),
            };
            match retry {
                BridgeCallResult::Finished(result) => result,
                BridgeCallResult::Retryable { reason, .. } => {
                    DirectCommitResult::NotCommitted(reason)
                }
            }
        }
    }
}

fn call_bridge(
    method: &str,
    expected: &str,
    replacement: &str,
    allow_sensitive_hint: bool,
) -> BridgeCallResult {
    let allow_sensitive_hint = if allow_sensitive_hint {
        "true"
    } else {
        "false"
    };
    match Command::new("busctl")
        .args([
            "--user",
            "--auto-start=no",
            "--timeout=500ms",
            "call",
            SERVICE,
            OBJECT,
            INTERFACE,
            method,
            "ssb",
            expected,
            replacement,
            allow_sensitive_hint,
        ])
        .output()
    {
        Ok(output) => classify_output(&output),
        Err(error) => BridgeCallResult::Finished(DirectCommitResult::NotCommitted(format!(
            "could not start the Fcitx5 bridge client: {error}"
        ))),
    }
}

pub(crate) fn install_addon() -> Result<AddonInstallResult> {
    if !command_exists("fcitx5") {
        return Ok(AddonInstallResult::Unavailable(
            "Fcitx5 is not installed; keyboard compatibility paths remain active".into(),
        ));
    }
    if !command_exists("c++") || !command_exists("pkg-config") {
        return Ok(AddonInstallResult::Unavailable(
            "the optional Fcitx5 bridge needs c++ and pkg-config at installation time".into(),
        ));
    }

    let flags = Command::new("pkg-config")
        .args(["--cflags", "--libs", "Fcitx5Module"])
        .output()
        .context("query Fcitx5 development flags")?;
    if !flags.status.success() {
        return Ok(AddonInstallResult::Unavailable(format!(
            "Fcitx5 development files are unavailable: {}",
            String::from_utf8_lossy(&flags.stderr).trim()
        )));
    }

    let paths = addon_paths()?;
    let expected_config = addon_config(&paths.library)?;
    if paths.library.is_file()
        && std::fs::read_to_string(&paths.config).ok().as_deref() == Some(&expected_config)
    {
        let loaded = bridge_is_loaded() || (refresh_fcitx5() && wait_for_bridge());
        return Ok(AddonInstallResult::Installed { loaded });
    }

    let source_dir = tempfile::tempdir().context("create Fcitx5 bridge build directory")?;
    let source = source_dir.path().join("snipexpand.cpp");
    let header = source_dir.path().join("suffix.h");
    let output = source_dir.path().join("libsnipexpand-fcitx5.so");
    std::fs::write(&source, ADDON_SOURCE).context("write embedded Fcitx5 bridge source")?;
    std::fs::write(&header, SUFFIX_HEADER).context("write embedded Fcitx5 bridge header")?;

    let mut compiler = Command::new("c++");
    compiler.args([
        "-std=c++20",
        "-O2",
        "-fPIC",
        "-fvisibility=hidden",
        "-shared",
        "-Wl,--no-undefined",
    ]);
    compiler.arg(format!("-DSNIPEXPAND_BUILD_ID=\"{:016x}\"", source_hash()));
    compiler.arg(&source).arg("-o").arg(&output);
    compiler.args(String::from_utf8_lossy(&flags.stdout).split_whitespace());
    let compiled = compiler.output().context("compile Fcitx5 bridge")?;
    if !compiled.status.success() {
        return Ok(AddonInstallResult::Unavailable(format!(
            "the optional Fcitx5 bridge did not compile: {}",
            String::from_utf8_lossy(&compiled.stderr).trim()
        )));
    }

    std::fs::create_dir_all(
        paths
            .library
            .parent()
            .context("Fcitx5 bridge library path has no parent")?,
    )?;
    std::fs::create_dir_all(
        paths
            .config
            .parent()
            .context("Fcitx5 addon config path has no parent")?,
    )?;
    std::fs::copy(&output, &paths.library).context("install Fcitx5 bridge library")?;
    std::fs::write(&paths.config, expected_config).context("install Fcitx5 addon metadata")?;

    let refreshed = refresh_fcitx5();
    Ok(AddonInstallResult::Installed {
        loaded: refreshed && wait_for_bridge(),
    })
}

pub(crate) fn addon_installed() -> bool {
    addon_paths().is_ok_and(|paths| {
        paths.library.is_file()
            && addon_config(&paths.library).is_ok_and(|expected| {
                std::fs::read_to_string(paths.config).ok().as_deref() == Some(&expected)
            })
    })
}

pub(crate) fn bridge_is_loaded() -> bool {
    Command::new("busctl")
        .args([
            "--user",
            "--auto-start=no",
            "--timeout=500ms",
            "call",
            SERVICE,
            OBJECT,
            INTERFACE,
            "BuildId",
        ])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && build_id_matches(&String::from_utf8_lossy(&output.stdout), source_hash())
        })
}

fn build_id_matches(response: &str, expected: u64) -> bool {
    response.trim() == format!("s \"{expected:016x}\"")
}

fn wait_for_bridge() -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        if bridge_is_loaded() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn refresh_fcitx5() -> bool {
    Command::new("busctl")
        .args([
            "--user",
            "--auto-start=no",
            "--timeout=2s",
            "call",
            SERVICE,
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "Refresh",
        ])
        .status()
        .is_ok_and(|status| status.success())
}

struct AddonPaths {
    library: PathBuf,
    config: PathBuf,
}

fn addon_paths() -> Result<AddonPaths> {
    let data_home = match std::env::var_os("XDG_DATA_HOME") {
        Some(value) => PathBuf::from(value),
        None => home_dir()?.join(".local/share"),
    };
    let hash = source_hash();
    Ok(AddonPaths {
        library: data_home
            .join("snipexpand/fcitx5")
            .join(format!("libsnipexpand-fcitx5-{hash:016x}.so")),
        config: data_home.join("fcitx5/addon/snipexpand.conf"),
    })
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME environment variable is not set")
}

fn addon_config(library: &Path) -> Result<String> {
    let library = library
        .to_str()
        .context("Fcitx5 bridge path is not valid UTF-8")?
        .strip_suffix(".so")
        .context("Fcitx5 bridge library path has no .so suffix")?;
    if library.contains(['\n', '\r', ';']) {
        anyhow::bail!("Fcitx5 bridge path contains a character unsupported by addon metadata");
    }
    Ok(format!(
        "[Addon]\nName=SnipExpand direct text bridge\nComment=Flash-free Unicode commits for SnipExpand\nType=SharedLibrary\nLibrary={library}\nCategory=Module\nVersion={}\nOnDemand=False\nConfigurable=False\n\n[Addon/Dependencies]\n0=core\n1=dbus\n",
        env!("CARGO_PKG_VERSION")
    ))
}

fn source_hash() -> u64 {
    let mut hash = Fnv1a64::default();
    hash.write(env!("CARGO_PKG_VERSION").as_bytes());
    hash.write(ADDON_SOURCE.as_bytes());
    hash.write(SUFFIX_HEADER.as_bytes());
    hash.finish()
}

struct Fnv1a64(u64);

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut value = self.0;
        for byte in bytes {
            value ^= u64::from(*byte);
            value = value.wrapping_mul(0x100000001b3);
        }
        self.0 = value;
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(name).is_file())
    })
}

fn classify_output(output: &Output) -> BridgeCallResult {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    classify_response(output.status.success(), &stdout, &stderr)
}

fn classify_response(success: bool, stdout: &str, stderr: &str) -> BridgeCallResult {
    if success {
        let mut fields = stdout.split_whitespace();
        if fields.next() != Some("u") {
            return BridgeCallResult::Finished(DirectCommitResult::Indeterminate(format!(
                "Fcitx5 bridge returned an unexpected response: {}",
                stdout.trim()
            )));
        }
        return match fields.next().and_then(|value| value.parse::<u32>().ok()) {
            Some(0) if fields.next().is_none() => {
                BridgeCallResult::Finished(DirectCommitResult::Committed)
            }
            Some(3) if fields.next().is_none() => BridgeCallResult::Retryable {
                kind: RetryKind::NoSurroundingText,
                reason: status_reason(3).into(),
            },
            Some(5) if fields.next().is_none() => BridgeCallResult::Retryable {
                kind: RetryKind::TriggerMismatch,
                reason: status_reason(5).into(),
            },
            Some(status @ (1 | 2 | 4)) if fields.next().is_none() => BridgeCallResult::Finished(
                DirectCommitResult::NotCommitted(status_reason(status).into()),
            ),
            Some(status @ (6 | 7)) if fields.next().is_none() => BridgeCallResult::Finished(
                DirectCommitResult::Suppressed(status_reason(status).into()),
            ),
            _ => BridgeCallResult::Finished(DirectCommitResult::Indeterminate(format!(
                "Fcitx5 bridge returned an unknown status: {}",
                stdout.trim()
            ))),
        };
    }

    if is_unavailable_error(stderr) {
        BridgeCallResult::Finished(DirectCommitResult::NotCommitted(
            "Fcitx5 direct-commit bridge is unavailable".into(),
        ))
    } else {
        BridgeCallResult::Finished(DirectCommitResult::Indeterminate(format!(
            "Fcitx5 bridge call failed after dispatch: {}",
            stderr.trim()
        )))
    }
}

fn status_reason(status: u32) -> &'static str {
    match status {
        1 => "Fcitx5 bridge rejected an invalid request",
        2 => "Fcitx5 has no focused input context",
        3 => "the focused application did not provide surrounding text",
        4 => "the focused application has an active text selection",
        5 => "the focused application did not confirm the exact trigger suffix",
        6 => "Fcitx5 identified the focused field as a password field",
        7 => "the configured Fcitx sensitive-hint policy suppressed the expansion",
        _ => "Fcitx5 bridge returned an unknown refusal",
    }
}

fn is_unavailable_error(stderr: &str) -> bool {
    [
        "Unknown object",
        "Unknown interface",
        "Unknown method",
        "UnknownObject",
        "UnknownInterface",
        "UnknownMethod",
        "ServiceUnknown",
        "NameHasNoOwner",
        "Destination does not exist",
        "Failed to connect to bus",
        "is not activatable",
        "was not provided by any .service files",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_bridge_must_match_this_binary_including_source_revision() {
        assert!(build_id_matches("s \"000000000000002a\"\n", 42));
        for response in ["s \"old\"", "u 42", "s \"000000000000002a\" trailing"] {
            assert!(!build_id_matches(response, 42));
        }
        assert!(!build_id_matches("s \"000000000000002a\"", 43));
    }

    #[test]
    fn committed_status_is_accepted_only_in_the_expected_shape() {
        assert_eq!(
            classify_response(true, "u 0\n", ""),
            BridgeCallResult::Finished(DirectCommitResult::Committed)
        );
        assert!(matches!(
            classify_response(true, "u 0 trailing\n", ""),
            BridgeCallResult::Finished(DirectCommitResult::Indeterminate(_))
        ));
    }

    #[test]
    fn explicit_refusals_are_classified_by_retry_safety() {
        for status in [1, 2, 4] {
            assert!(matches!(
                classify_response(true, &format!("u {status}\n"), ""),
                BridgeCallResult::Finished(DirectCommitResult::NotCommitted(_))
            ));
        }
        for status in [3, 5] {
            assert!(matches!(
                classify_response(true, &format!("u {status}\n"), ""),
                BridgeCallResult::Retryable { .. }
            ));
        }
        for status in [6, 7] {
            assert!(matches!(
                classify_response(true, &format!("u {status}\n"), ""),
                BridgeCallResult::Finished(DirectCommitResult::Suppressed(_))
            ));
        }
    }

    #[test]
    fn missing_surrounding_text_uses_the_guarded_fallback_once() {
        let exact_calls = std::cell::Cell::new(0);
        let fallback_calls = std::cell::Cell::new(0);
        let result = replace_with(
            || {
                exact_calls.set(exact_calls.get() + 1);
                BridgeCallResult::Retryable {
                    kind: RetryKind::NoSurroundingText,
                    reason: "surrounding text is unavailable".into(),
                }
            },
            || {
                fallback_calls.set(fallback_calls.get() + 1);
                BridgeCallResult::Finished(DirectCommitResult::Committed)
            },
            std::time::Duration::ZERO,
        );
        assert_eq!(result, DirectCommitResult::Committed);
        assert_eq!(exact_calls.get(), 1);
        assert_eq!(fallback_calls.get(), 1);
    }

    #[test]
    fn suffix_lag_retries_exact_validation_instead_of_blind_deletion() {
        let exact_calls = std::cell::Cell::new(0);
        let fallback_calls = std::cell::Cell::new(0);
        let result = replace_with(
            || {
                let call = exact_calls.get() + 1;
                exact_calls.set(call);
                if call == 1 {
                    BridgeCallResult::Retryable {
                        kind: RetryKind::TriggerMismatch,
                        reason: "surrounding text is stale".into(),
                    }
                } else {
                    BridgeCallResult::Finished(DirectCommitResult::Committed)
                }
            },
            || {
                fallback_calls.set(fallback_calls.get() + 1);
                BridgeCallResult::Finished(DirectCommitResult::Committed)
            },
            std::time::Duration::ZERO,
        );
        assert_eq!(result, DirectCommitResult::Committed);
        assert_eq!(exact_calls.get(), 2);
        assert_eq!(fallback_calls.get(), 0);
    }

    #[test]
    fn a_second_retryable_refusal_stops_after_one_retry() {
        let exact_calls = std::cell::Cell::new(0);
        let fallback_calls = std::cell::Cell::new(0);
        let result = replace_with(
            || {
                exact_calls.set(exact_calls.get() + 1);
                BridgeCallResult::Retryable {
                    kind: RetryKind::NoSurroundingText,
                    reason: "surrounding text is unavailable".into(),
                }
            },
            || {
                fallback_calls.set(fallback_calls.get() + 1);
                BridgeCallResult::Retryable {
                    kind: RetryKind::TriggerMismatch,
                    reason: "guarded fallback was refused".into(),
                }
            },
            std::time::Duration::ZERO,
        );
        assert!(matches!(result, DirectCommitResult::NotCommitted(_)));
        assert_eq!(exact_calls.get(), 1);
        assert_eq!(fallback_calls.get(), 1);
    }

    #[test]
    fn definitive_and_indeterminate_results_are_never_retried() {
        for first in [
            DirectCommitResult::NotCommitted("definitive refusal".into()),
            DirectCommitResult::Suppressed("sensitive field".into()),
            DirectCommitResult::Indeterminate("transport failure".into()),
        ] {
            let mut calls = 0;
            let expected = first.clone();
            let result = replace_with(
                || {
                    calls += 1;
                    BridgeCallResult::Finished(first.clone())
                },
                || panic!("a definitive result must not call the fallback"),
                std::time::Duration::ZERO,
            );
            assert_eq!(result, expected);
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn missing_bridge_is_safe_but_transport_failures_are_indeterminate() {
        assert!(matches!(
            classify_response(
                false,
                "",
                "Call failed: Unknown object '/io/github/silouanwright/SnipExpand'"
            ),
            BridgeCallResult::Finished(DirectCommitResult::NotCommitted(_))
        ));
        assert!(matches!(
            classify_response(false, "", "Call failed: Destination does not exist"),
            BridgeCallResult::Finished(DirectCommitResult::NotCommitted(_))
        ));
        assert!(matches!(
            classify_response(false, "", "Failed to connect to bus: No medium found"),
            BridgeCallResult::Finished(DirectCommitResult::NotCommitted(_))
        ));
        assert!(matches!(
            classify_response(false, "", "Call timed out"),
            BridgeCallResult::Finished(DirectCommitResult::Indeterminate(_))
        ));
    }

    #[test]
    fn unknown_success_status_is_never_retried() {
        assert!(matches!(
            classify_response(true, "u 99\n", ""),
            BridgeCallResult::Finished(DirectCommitResult::Indeterminate(_))
        ));
        assert!(matches!(
            classify_response(true, "garbled\n", ""),
            BridgeCallResult::Finished(DirectCommitResult::Indeterminate(_))
        ));
    }

    #[test]
    fn addon_metadata_uses_an_absolute_library_without_the_loader_suffix() {
        let config = addon_config(Path::new("/tmp/snipexpand/libsnipexpand.so")).unwrap();
        assert!(config.contains("Library=/tmp/snipexpand/libsnipexpand\n"));
        assert!(!config.contains("Library=/tmp/snipexpand/libsnipexpand.so"));
        assert!(config.contains("OnDemand=False"));
        assert!(config.contains("1=dbus"));
    }

    #[test]
    fn embedded_addon_hash_is_stable_and_content_sensitive() {
        assert_eq!(source_hash(), source_hash());
        let mut different = Fnv1a64::default();
        different.write(ADDON_SOURCE.as_bytes());
        different.write(b"different");
        assert_ne!(source_hash(), different.finish());
    }
}
