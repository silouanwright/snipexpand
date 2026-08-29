mod app;
mod config;
mod daemon;
mod expander;
mod injector;
mod ipc;
mod keyboard;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "snipexpand", about = "Wayland-native text expander", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create starter configuration without overwriting existing files
    Init,
    /// Add or overwrite an expansion (use \\n for newlines)
    Add {
        /// Human-readable name shown by snippet browsers
        #[arg(long)]
        label: Option<String>,
        /// Additional search term (repeatable)
        #[arg(long = "search-term")]
        search_terms: Vec<String>,
        trigger: String,
        expansion: String,
    },
    /// Remove an expansion
    Remove { trigger: String },
    /// List all expansions
    List {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Validate all configuration files without starting the daemon
    Check,
    /// Diagnose configuration, session, permissions, and runtime dependencies
    Doctor {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the active application's title, class, and executable
    Detect,
    /// Signal running daemon to reload config
    Reload,
    /// Enable automatic expansion
    Enable,
    /// Pause automatic expansion
    Disable,
    /// Toggle automatic expansion
    Toggle,
    /// Insert a configured expansion into the focused application
    Paste {
        /// Wait for a launcher or panel to close before inserting
        #[arg(long, default_value_t = 150)]
        delay_ms: u64,
        trigger: String,
    },
    /// Show daemon status
    Status {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Initialize configuration and install the systemd user service
    Install,
    /// Stop and remove the user service while preserving configuration
    Uninstall,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    ensure_config()?;
    match cli.command {
        None => {
            // Daemon mode: run the tokio event loop
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let config = config::Config::load_default()?;
                    daemon::run(config).await
                })
        }
        Some(cmd) => handle_cmd(cmd),
    }
}

fn handle_cmd(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Init => init_config()?,
        Cmd::List { json } => {
            let cfg = config::Config::load_default()?;
            let rows = list_entries(&cfg);
            if json {
                println!("{}", serde_json::to_string(&rows)?);
            } else if rows.is_empty() {
                println!("No expansions configured.");
                println!("Add one with: snipexpand add /trigger \"expansion text\"");
            } else {
                for row in rows {
                    println!(
                        "{:<20} => {:<36} [{}]",
                        row.trigger,
                        row.replacement.replace('\n', "\\n"),
                        row.source
                    );
                }
            }
        }

        Cmd::Check => {
            let cfg = config::Config::load_default()?;
            println!(
                "OK: {} match group(s), {} trigger(s), {} file(s)",
                cfg.matches.len(),
                cfg.matches
                    .iter()
                    .map(|item| item.triggers.len())
                    .sum::<usize>(),
                cfg.loaded_files.len()
            );
            print_config_warnings(&cfg);
        }

        Cmd::Doctor { json } => doctor(json),

        Cmd::Detect => {
            let info = app::detect()?;
            println!("title: {}", info.title.as_deref().unwrap_or("<unknown>"));
            println!("class: {}", info.class.as_deref().unwrap_or("<unknown>"));
            println!("exec: {}", info.exec.as_deref().unwrap_or("<unknown>"));
        }

        Cmd::Add {
            label,
            search_terms,
            trigger,
            expansion,
        } => {
            let expansion = expansion.replace("\\n", "\n");
            config::Config::add_generated(&trigger, &expansion, label.as_deref(), &search_terms)?;
            config::Config::load_default()?;
            println!("Added: {} => {}", trigger, expansion.replace('\n', "\\n"));
            // Signal daemon to reload if running
            let _ = signal_daemon_reload();
        }

        Cmd::Remove { trigger } => {
            if config::Config::remove_generated(&trigger)? {
                config::Config::load_default()?;
                println!("Removed: {}", trigger);
                let _ = signal_daemon_reload();
            } else {
                anyhow::bail!("Trigger '{}' not found", trigger);
            }
        }

        Cmd::Reload => {
            let sock = ipc::socket_path()?;
            if !sock.exists() {
                anyhow::bail!("SnipExpand daemon is not running");
            }
            signal_daemon_reload().map_err(|e| anyhow::anyhow!("Could not reach daemon: {}", e))?;
            println!("Reloaded");
        }

        Cmd::Enable => set_daemon_enabled("enable")?,
        Cmd::Disable => set_daemon_enabled("disable")?,
        Cmd::Toggle => set_daemon_enabled("toggle")?,

        Cmd::Paste { delay_ms, trigger } => {
            if delay_ms > 2000 {
                anyhow::bail!("delay-ms must be between 0 and 2000");
            }
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            let response =
                send_daemon_command(&format!("paste\t{}", serde_json::to_string(&trigger)?))
                    .map_err(|error| anyhow::anyhow!("Could not reach daemon: {error}"))?;
            if response != "ok" {
                anyhow::bail!("{}", response.strip_prefix("error: ").unwrap_or(&response));
            }
        }

        Cmd::Status { json } => {
            let status = daemon_status()
                .map_err(|error| anyhow::anyhow!("SnipExpand daemon is not reachable: {error}"))?;
            if json {
                println!("{}", serde_json::to_string(&status)?);
            } else {
                println!("SnipExpand daemon is running");
                println!(
                    "expansion: {}",
                    if status.enabled { "enabled" } else { "paused" }
                );
                println!("version: {}", status.version);
                println!("pid: {}", status.pid);
                println!("injection backend: {}", status.injection_backend);
                println!(
                    "matches: {} group(s), {} trigger(s), {} file(s)",
                    status.match_groups, status.triggers, status.files
                );
                println!(
                    "configuration: {}",
                    if status.config_valid {
                        "valid"
                    } else {
                        "INVALID (last valid configuration remains active)"
                    }
                );
            }
        }

        Cmd::Install => {
            install_service()?;
        }
        Cmd::Uninstall => {
            uninstall_service()?;
        }
    }
    Ok(())
}

fn set_daemon_enabled(command: &str) -> anyhow::Result<()> {
    let response = send_daemon_command(command)
        .map_err(|error| anyhow::anyhow!("Could not reach daemon: {error}"))?;
    println!("SnipExpand {response}");
    Ok(())
}

fn print_config_warnings(config: &config::Config) {
    for warning in config.unreachable_triggers() {
        println!(
            "WARNING: trigger '{}' in {} is unreachable in immediate mode because '{}' in {} expands first",
            warning.trigger,
            warning.source.display(),
            warning.blocking_trigger,
            warning.blocking_source.display()
        );
    }
}

fn init_config() -> anyhow::Result<()> {
    let dir = config::Config::dir();
    let match_dir = config::Config::match_dir();
    std::fs::create_dir_all(&match_dir)?;
    let settings = dir.join("config.yml");
    let matches = match_dir.join("base.yml");
    write_new(
        &settings,
        "trigger_mode: space\nterminators: [space]\ninjection_backend: auto\ninjection_delay_ms: 1\nwayland_injection_delay_ms: 0\nuinput_injection_delay_ms: 1\ninjection_settle_ms: 10\n",
    )?;
    write_new(
        &matches,
        "matches:\n  - trigger: \";hello\"\n    replace: \"Hello from SnipExpand!\"\n",
    )?;
    config::Config::load_default()?;
    println!("Configuration ready at {}", dir.display());
    Ok(())
}

fn ensure_config() -> anyhow::Result<()> {
    let dir = config::Config::dir();
    let fresh = !dir.exists();
    let match_dir = config::Config::match_dir();
    std::fs::create_dir_all(&match_dir)?;

    let settings = dir.join("config.yml");
    if !settings.exists() {
        write_new(
            &settings,
            "trigger_mode: space\nterminators: [space]\ninjection_backend: auto\ninjection_delay_ms: 1\nwayland_injection_delay_ms: 0\nuinput_injection_delay_ms: 1\ninjection_settle_ms: 10\n",
        )?;
    }

    if fresh {
        let matches = match_dir.join("base.yml");
        write_new(
            &matches,
            "matches:\n  - trigger: \";hello\"\n    replace: \"Hello from SnipExpand!\"\n",
        )?;
    }

    std::fs::write(
        dir.join("SKILL.md"),
        include_str!("../skills/snipexpand-shortcuts.md"),
    )?;

    Ok(())
}

fn write_new(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(contents.as_bytes())?;
            println!("Created: {}", path.display());
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("Kept existing: {}", path.display());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct ListEntry {
    trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    search_terms: Vec<String>,
    replacement: String,
    source: String,
    generated: bool,
    editable: bool,
}

fn list_entries(config: &config::Config) -> Vec<ListEntry> {
    let generated_path = config::Config::generated_path();
    let mut rows = config
        .matches
        .iter()
        .flat_map(|item| {
            let generated = item.source == generated_path;
            item.triggers.iter().map(move |trigger| ListEntry {
                trigger: trigger.clone(),
                label: item.label.clone(),
                search_terms: item.search_terms.clone(),
                replacement: item.replace.clone(),
                source: item.source.display().to_string(),
                generated,
                editable: generated,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.trigger.cmp(&right.trigger));
    rows
}

#[derive(Debug, serde::Serialize)]
struct DoctorCheck {
    id: &'static str,
    label: &'static str,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<&'static str>,
}

#[derive(Debug, serde::Serialize)]
struct DoctorReport {
    ok: bool,
    checks: Vec<DoctorCheck>,
}

fn doctor(json: bool) {
    let config_result = config::Config::load_default();
    let daemon_result = daemon_status();
    let checks = vec![
        doctor_check(
            "wayland_session",
            "Wayland session",
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            None,
            "Log in to a Wayland session",
        ),
        doctor_check(
            "input_group",
            "input group",
            in_group("input"),
            None,
            "Add your user to the input group, then log out and back in",
        ),
        doctor_check(
            "uinput_writable",
            "/dev/uinput writable",
            can_open_uinput(),
            None,
            "Grant your user write access to /dev/uinput",
        ),
        doctor_check(
            "wtype",
            "wtype Unicode fallback",
            command_exists("wtype"),
            None,
            "Install wtype",
        ),
        doctor_check(
            "configuration",
            "configuration",
            config_result.is_ok(),
            config_result.err().map(|error| error.to_string()),
            "Run snipexpand check and correct the reported configuration error",
        ),
        doctor_check(
            "daemon",
            "daemon",
            daemon_result.is_ok(),
            daemon_result.err().map(|error| error.to_string()),
            "Run snipexpand install or restart the snipexpand user service",
        ),
    ];
    let report = DoctorReport {
        ok: checks.iter().all(|check| check.ok),
        checks,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string(&report).expect("doctor report is serializable")
        );
    } else {
        for check in &report.checks {
            println!("{} {}", if check.ok { "✓" } else { "✗" }, check.label);
        }
    }
    if !report.ok {
        std::process::exit(1);
    }
}

fn doctor_check(
    id: &'static str,
    label: &'static str,
    ok: bool,
    detail: Option<String>,
    fix: &'static str,
) -> DoctorCheck {
    DoctorCheck {
        id,
        label,
        ok,
        detail: (!ok).then_some(detail).flatten(),
        fix: (!ok).then_some(fix),
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

fn in_group(name: &str) -> bool {
    std::process::Command::new("id")
        .arg("-nG")
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .split_whitespace()
                    .any(|group| group == name)
        })
}

fn can_open_uinput() -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

fn signal_daemon_reload() -> anyhow::Result<()> {
    let sock = ipc::socket_path()?;
    if !sock.exists() {
        return Ok(()); // Daemon not running, config will be read on next start
    }
    use std::io::Write;
    let mut stream = std::os::unix::net::UnixStream::connect(&sock)?;
    stream.write_all(b"reload\n")?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

fn send_daemon_command(command: &str) -> anyhow::Result<String> {
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    let mut stream = std::os::unix::net::UnixStream::connect(ipc::socket_path()?)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    writeln!(stream, "{command}")?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    Ok(response.trim_end().to_string())
}

fn daemon_status() -> anyhow::Result<ipc::DaemonStatus> {
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    let sock = ipc::socket_path()?;
    let mut stream = std::os::unix::net::UnixStream::connect(&sock)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(b"status\n")?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    if response.is_empty() {
        anyhow::bail!("daemon returned no status response");
    }
    serde_json::from_str(response.trim_end()).map_err(Into::into)
}

fn install_service() -> anyhow::Result<()> {
    let binary = std::env::current_exe()?;
    let service = service_definition(&binary);
    let service_path = service_path()?;
    let service_dir = service_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("systemd user service path has no parent"))?;
    std::fs::create_dir_all(service_dir)?;
    std::fs::write(&service_path, &service)?;
    println!("Wrote: {}", service_path.display());

    run_systemctl(&["daemon-reload"])?;
    run_systemctl(&["enable", "snipexpand.service"])?;
    // `enable --now` does not restart an already active service. An explicit
    // restart guarantees that upgrades run the invoking binary.
    run_systemctl(&["restart", "snipexpand.service"])?;

    println!("SnipExpand service installed and started.");
    if !in_group("input") {
        println!();
        println!("Input-device access still needs permission:");
        println!("  sudo usermod -a -G input $USER");
        println!("  # Then log out and back in");
    }
    Ok(())
}

fn service_definition(binary: &std::path::Path) -> String {
    let binary = binary
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!(
        r#"[Unit]
Description=SnipExpand Wayland text expander
Documentation=https://github.com/silouanwright/snipexpand
After=graphical-session.target

[Service]
Type=simple
ExecStart="{}"
Restart=on-failure
RestartSec=2s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
"#,
        binary
    )
}

fn service_path() -> anyhow::Result<std::path::PathBuf> {
    let config_home = match std::env::var("XDG_CONFIG_HOME") {
        Ok(value) => std::path::PathBuf::from(value),
        Err(_) => {
            let home = std::env::var("HOME")
                .map_err(|_| anyhow::anyhow!("HOME environment variable is not set"))?;
            std::path::PathBuf::from(home).join(".config")
        }
    };
    Ok(config_home
        .join("systemd")
        .join("user")
        .join("snipexpand.service"))
}

fn run_systemctl(arguments: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl --user {} failed", arguments.join(" "));
    }
    Ok(())
}

fn uninstall_service() -> anyhow::Result<()> {
    let service_path = service_path()?;
    if service_path.exists() {
        run_systemctl(&["disable", "--now", "snipexpand.service"])?;
    }
    if service_path.exists() {
        std::fs::remove_file(&service_path)?;
        println!("Removed: {}", service_path.display());
    } else {
        println!("SnipExpand user service was already removed.");
    }
    run_systemctl(&["daemon-reload"])?;
    println!("SnipExpand service removed. Configuration was preserved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_uses_the_invoking_binary_and_restarts_on_failure() {
        let definition = service_definition(std::path::Path::new("/tmp/Snip Expand/%i/bin"));
        assert!(definition.contains("ExecStart=\"/tmp/Snip Expand/%%i/bin\""));
        assert!(definition.contains("Restart=on-failure"));
        assert!(definition.contains("WantedBy=graphical-session.target"));
    }

    #[test]
    fn doctor_check_only_exposes_failure_guidance_when_needed() {
        let passing = doctor_check("daemon", "daemon", true, None, "Restart it");
        assert!(passing.fix.is_none());

        let failing = doctor_check(
            "daemon",
            "daemon",
            false,
            Some("not reachable".to_string()),
            "Restart it",
        );
        assert_eq!(failing.detail.as_deref(), Some("not reachable"));
        assert_eq!(failing.fix, Some("Restart it"));
    }

    #[test]
    fn list_entries_are_sorted_and_mark_only_generated_matches_editable() {
        let generated = config::Config::generated_path();
        let handwritten = generated.parent().unwrap().join("personal.yml");
        let make_match = |trigger: &str, source: std::path::PathBuf| config::Match {
            triggers: vec![trigger.to_string()],
            label: Some(format!("Label for {trigger}")),
            search_terms: vec!["example".into()],
            replace: format!("{trigger} replacement"),
            vars: Vec::new(),
            word: false,
            left_word: false,
            right_word: false,
            propagate_case: false,
            uppercase_style: config::UppercaseStyle::Uppercase,
            source,
        };
        let config = config::Config {
            matches: vec![make_match(";z", handwritten), make_match(";a", generated)],
            ..Default::default()
        };

        let entries = list_entries(&config);

        assert_eq!(entries[0].trigger, ";a");
        assert_eq!(entries[0].label.as_deref(), Some("Label for ;a"));
        assert_eq!(entries[0].search_terms, ["example"]);
        assert!(entries[0].editable);
        assert!(!entries[1].editable);
    }
}
