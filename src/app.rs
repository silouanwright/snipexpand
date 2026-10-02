use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppInfo {
    pub title: Option<String>,
    pub class: Option<String>,
    pub exec: Option<String>,
}

impl AppInfo {
    pub fn uses_chromium_text_input(&self) -> bool {
        self.class.as_deref().is_some_and(chromium_identifier)
            || self.exec.as_deref().is_some_and(chromium_identifier)
            || self.exec.as_deref().is_some_and(electron_executable)
    }
}

fn chromium_identifier(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value == "chromium"
        || value.contains("/chromium")
        || value.contains("google-chrome")
        || value
            .rsplit('/')
            .next()
            .is_some_and(|name| name == "chrome" || name.starts_with("chrome-"))
}

fn electron_executable(value: &str) -> bool {
    let Some(parent) = std::path::Path::new(value).parent() else {
        return false;
    };
    [
        "resources/app.asar",
        "resources/default_app.asar",
        "resources/electron.asar",
        "resources/app",
        "app.asar",
    ]
    .iter()
    .any(|relative| parent.join(relative).exists())
}

#[derive(Deserialize)]
struct HyprlandWindow {
    #[serde(default)]
    title: String,
    #[serde(default)]
    class: String,
    pid: Option<u32>,
}

pub fn detect() -> Result<AppInfo> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return detect_hyprland();
    }

    detect_wlroots().context(
        "active-application detection is unavailable; Hyprland or wlrctl is required on Wayland",
    )
}

fn detect_hyprland() -> Result<AppInfo> {
    let output = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .context("run 'hyprctl activewindow -j'")?;
    if !output.status.success() {
        anyhow::bail!(
            "hyprctl activewindow failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let window: HyprlandWindow =
        serde_json::from_slice(&output.stdout).context("parse Hyprland active-window response")?;
    Ok(AppInfo {
        title: nonempty(window.title),
        class: nonempty(window.class),
        exec: window.pid.and_then(executable_for_pid),
    })
}

fn detect_wlroots() -> Result<AppInfo> {
    let output = Command::new("wlrctl")
        .args(["toplevel", "list", "state:active"])
        .output()
        .context("run 'wlrctl toplevel list state:active'")?;
    if !output.status.success() {
        anyhow::bail!(
            "wlrctl active-window query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let line = String::from_utf8(output.stdout).context("decode wlrctl output")?;
    let (class, title) = line
        .trim()
        .split_once(": ")
        .context("wlrctl returned no active toplevel")?;
    Ok(AppInfo {
        title: nonempty(title.to_string()),
        class: nonempty(class.to_string()),
        exec: None,
    })
}

fn executable_for_pid(pid: u32) -> Option<String> {
    std::fs::read_link(PathBuf::from("/proc").join(pid.to_string()).join("exe"))
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_strings_become_missing_properties() {
        assert_eq!(nonempty(String::new()), None);
        assert_eq!(nonempty("foot".into()), Some("foot".into()));
    }

    #[test]
    fn chromium_apps_are_identified_without_matching_unrelated_names() {
        assert!(AppInfo {
            class: Some("chrome-__home_user_target.html-Default".into()),
            ..Default::default()
        }
        .uses_chromium_text_input());
        assert!(AppInfo {
            exec: Some("/usr/lib/chromium/chromium".into()),
            ..Default::default()
        }
        .uses_chromium_text_input());
        assert!(!AppInfo {
            class: Some("chromium-notes".into()),
            ..Default::default()
        }
        .uses_chromium_text_input());
    }

    #[test]
    fn electron_apps_are_identified_from_their_runtime_artifacts() {
        let dir = tempfile::TempDir::new().unwrap();
        let resources = dir.path().join("resources");
        std::fs::create_dir(&resources).unwrap();
        std::fs::write(resources.join("app.asar"), []).unwrap();

        assert!(AppInfo {
            exec: Some(dir.path().join("signal-desktop").display().to_string()),
            ..Default::default()
        }
        .uses_chromium_text_input());
        assert!(!AppInfo {
            exec: Some(dir.path().join("bin/editor").display().to_string()),
            ..Default::default()
        }
        .uses_chromium_text_input());
    }
}
