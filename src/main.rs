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
    Add { trigger: String, expansion: String },
    /// Remove an expansion
    Remove { trigger: String },
    /// List all expansions
    List,
    /// Validate all configuration files without starting the daemon
    Check,
    /// Diagnose configuration, session, permissions, and runtime dependencies
    Doctor,
    /// Show the active application's title, class, and executable
    Detect,
    /// Signal running daemon to reload config
    Reload,
    /// Show daemon status
    Status {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Initialize configuration and install the systemd user service
    Install,
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
        Cmd::List => {
            let cfg = config::Config::load_default()?;
            if cfg.matches.is_empty() {
                println!("No expansions configured.");
                println!("Add one with: snipexpand add /trigger \"expansion text\"");
            } else {
                let mut rows: Vec<_> = cfg
                    .matches
                    .iter()
                    .flat_map(|item| item.triggers.iter().map(move |trigger| (trigger, item)))
                    .collect();
                rows.sort_by_key(|(trigger, _)| trigger.as_str());
                for (trigger, item) in rows {
                    println!(
                        "{:<20} => {:<36} [{}]",
                        trigger,
                        item.replace.replace('\n', "\\n"),
                        item.source.display()
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

        Cmd::Doctor => doctor(),

        Cmd::Detect => {
            let info = app::detect()?;
            println!("title: {}", info.title.as_deref().unwrap_or("<unknown>"));
            println!("class: {}", info.class.as_deref().unwrap_or("<unknown>"));
            println!("exec: {}", info.exec.as_deref().unwrap_or("<unknown>"));
        }

        Cmd::Add { trigger, expansion } => {
            let expansion = expansion.replace("\\n", "\n");
            config::Config::add_generated(&trigger, &expansion)?;
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

        Cmd::Status { json } => {
            let status = daemon_status()
                .map_err(|error| anyhow::anyhow!("SnipExpand daemon is not reachable: {error}"))?;
            if json {
                println!("{}", serde_json::to_string(&status)?);
            } else {
                println!("SnipExpand daemon is running");
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
    }
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

fn doctor() {
    let mut failed = false;
    check_doctor(
        "Wayland session",
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        &mut failed,
    );
    check_doctor("input group", in_group("input"), &mut failed);
    check_doctor("/dev/uinput writable", can_open_uinput(), &mut failed);
    check_doctor(
        "wtype Unicode fallback",
        command_exists("wtype"),
        &mut failed,
    );
    check_doctor(
        "configuration",
        config::Config::load_default().is_ok(),
        &mut failed,
    );
    let running = daemon_status().is_ok();
    check_doctor("daemon", running, &mut failed);
    if failed {
        std::process::exit(1);
    }
}

fn check_doctor(label: &str, ok: bool, failed: &mut bool) {
    println!("{} {label}", if ok { "✓" } else { "✗" });
    *failed |= !ok;
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
    // Find the binary path
    let binary = std::env::current_exe()?;

    // Build service file content
    let service = format!(
        r#"[Unit]
Description=SnipExpand Wayland text expander
Documentation=https://github.com/silouanwright/snipexpand
After=graphical-session.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=2s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
"#,
        binary.display()
    );

    // Write service file
    let service_dir = {
        let config_home = match std::env::var("XDG_CONFIG_HOME") {
            Ok(v) => std::path::PathBuf::from(v),
            Err(_) => {
                let home = std::env::var("HOME")
                    .map_err(|_| anyhow::anyhow!("HOME environment variable is not set"))?;
                std::path::PathBuf::from(home).join(".config")
            }
        };
        config_home.join("systemd").join("user")
    };
    std::fs::create_dir_all(&service_dir)?;
    let service_path = service_dir.join("snipexpand.service");
    std::fs::write(&service_path, &service)?;
    println!("Wrote: {:?}", service_path);

    // Enable and start the service
    let status = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "snipexpand"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now snipexpand failed");
    }

    println!("SnipExpand service installed and started.");
    if !in_group("input") {
        println!();
        println!("Input-device access still needs permission:");
        println!("  sudo usermod -a -G input $USER");
        println!("  # Then log out and back in");
    }
    Ok(())
}
