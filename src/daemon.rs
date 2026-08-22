use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::expander::Expander;
use crate::injector::Injector;
use crate::ipc::{IpcCmd, IpcServer};
use crate::keyboard::KeyboardStream;

// evdev KEY codes (Linux input-event-codes.h)
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_ENTER: u16 = 28;
// Keys that reset the expansion buffer (cursor movement)
const RESET_KEYS: &[u16] = &[
    105, // KEY_LEFT
    106, // KEY_RIGHT
    103, // KEY_UP
    108, // KEY_DOWN
    102, // KEY_HOME
    107, // KEY_END
    1,   // KEY_ESC
    110, // KEY_INSERT
    111, // KEY_DELETE
    104, // KEY_PAGEUP
    109, // KEY_PAGEDOWN
];

struct Undo {
    replacement_len: usize,
    original: String,
}

struct PendingExpansion {
    release_code: u16,
    key_released: bool,
    expansion: crate::expander::Expansion,
}

#[derive(Default)]
struct InputState {
    shift_held: bool,
    altgr_held: bool,
    undo: Option<Undo>,
    pending_undo: Option<Undo>,
    pending_expansion: Option<PendingExpansion>,
}

pub async fn run(config: Config) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("snipexpand=info".parse()?),
        )
        .init();
    tracing::info!("SnipExpand daemon starting");
    log_config_warnings(&config);

    // Spawn Wayland thread (blocks until keymap received)
    let injector = Injector::spawn(
        config.settings.injection_backend,
        config.settings.injection_delay_ms,
        config.settings.wayland_injection_delay_ms,
        config.settings.uinput_injection_delay_ms,
        config.settings.injection_settle_ms,
        wayland_text_characters(&config),
    )?;
    tracing::info!("Injection keyboard ready");

    // Open evdev keyboard stream
    let mut kb_stream = KeyboardStream::new().await?;
    tracing::info!("Keyboard event stream ready");

    // IPC server
    let ipc_path = crate::ipc::socket_path()?;
    let ipc_server = IpcServer::new(&ipc_path).await?;
    tracing::info!("IPC socket at {:?}", ipc_path);

    // Config + expander
    let config = Arc::new(Mutex::new(config));
    let mut expander = {
        let cfg = config.lock().unwrap();
        Expander::new_configured(
            cfg.matches.clone(),
            cfg.settings.trigger_mode,
            cfg.settings.terminator_chars(),
        )
    };

    // Config file watcher
    let (watch_tx, mut watch_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let config_path = Config::dir();
    let watch_tx2 = watch_tx.clone();
    // Use std::thread::spawn (not spawn_blocking) so the tokio runtime doesn't
    // wait for this thread on shutdown, enabling fast SIGTERM handling.
    std::thread::spawn(move || {
        use notify::{Config as NConfig, RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, NConfig::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Config watcher failed to start: {}", e);
                return;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&config_path) {
            tracing::error!("Failed to create config directory: {}", e);
            return;
        }
        if let Err(e) = watcher.watch(&config_path, RecursiveMode::Recursive) {
            tracing::error!("Failed to watch config directory: {}", e);
            return;
        }
        while rx.recv().is_ok() {
            let _ = watch_tx2.send(());
        }
    });

    // Signals
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;
    let mut sig_usr1 = signal(SignalKind::user_defined1())?;

    // Keep watch_tx alive so the channel stays open
    let _watch_tx = watch_tx;

    // Track physical modifier state for XKB-based input character decoding.
    let mut input = InputState::default();

    tracing::info!("SnipExpand daemon ready");

    loop {
        tokio::select! {
            event = kb_stream.next_event() => {
                match event {
                    Some(ev) => {
                        // Update modifier state on both press and release.
                        match ev.code {
                            42 | 54 => {
                                input.shift_held = ev.value == 1;
                                if ev.value == 0 {
                                    complete_pending_expansion(&injector, &config, &mut input, ev.code);
                                }
                            }   // L/R Shift
                            100 => {
                                input.altgr_held = ev.value == 1;
                                if ev.value == 0 {
                                    complete_pending_expansion(&injector, &config, &mut input, ev.code);
                                }
                            } // Right Alt / AltGr
                            _ if ev.value == 0 && ev.code == KEY_BACKSPACE => {
                                if let Some(previous) = input.pending_undo.take() {
                                    complete_undo(&injector, &mut expander, previous);
                                }
                            }
                            _ if ev.value == 2 && ev.code == KEY_BACKSPACE => {
                                // A held Backspace means continuous deletion, not expansion undo.
                                input.pending_undo = None;
                                expander.reset();
                            }
                            _ if ev.value == 0 => {
                                complete_pending_expansion(&injector, &config, &mut input, ev.code);
                            }
                            _ if ev.value == 1 => {
                                // Key press only. Repeat events flood the buffer.
                                handle_key_event(&ev, &mut expander, &injector, &mut input);
                            }
                            _ => {}
                        }
                    }
                    None => {
                        tracing::warn!("Keyboard stream ended");
                        break;
                    }
                }
            }

            Some(_) = watch_rx.recv() => {
                tracing::info!("Config changed, reloading");
                reload_config(&config, &mut expander, &injector);
            }

            cmd = ipc_server.accept() => {
                match cmd {
                    Ok((IpcCmd::Reload, mut stream)) => {
                        tracing::info!("Reload requested via IPC");
                        reload_config(&config, &mut expander, &injector);
                        let _ = stream.write_all(b"ok\n").await;
                    }
                    Ok((IpcCmd::Status, mut stream)) => {
                        tracing::info!("Status requested via IPC");
                        let status = {
                            let cfg = config.lock().unwrap();
                            crate::ipc::DaemonStatus {
                                running: true,
                                version: env!("CARGO_PKG_VERSION").to_string(),
                                pid: std::process::id(),
                                injection_backend: injector.backend().to_string(),
                                match_groups: cfg.matches.len(),
                                triggers: cfg.matches.iter().map(|item| item.triggers.len()).sum(),
                                files: cfg.loaded_files.len(),
                                config_valid: Config::load_default().is_ok(),
                            }
                        };
                        if let Ok(mut response) = serde_json::to_vec(&status) {
                            response.push(b'\n');
                            let _ = stream.write_all(&response).await;
                        }
                    }
                    Err(e) => tracing::warn!("IPC error: {}", e),
                }
            }

            _ = sig_term.recv() => {
                tracing::info!("SIGTERM received, shutting down");
                break;
            }
            _ = sig_int.recv() => {
                tracing::info!("SIGINT received, shutting down");
                break;
            }
            _ = sig_usr1.recv() => {
                tracing::info!("SIGUSR1 received, reloading config");
                reload_config(&config, &mut expander, &injector);
            }
        }
    }

    drop(kb_stream);
    tracing::info!("SnipExpand daemon stopped");
    Ok(())
}

fn handle_key_event(
    ev: &crate::keyboard::KeyEvent,
    expander: &mut Expander,
    injector: &Injector,
    input: &mut InputState,
) {
    if RESET_KEYS.contains(&ev.code) {
        input.undo = None;
        input.pending_undo = None;
        input.pending_expansion = None;
        expander.reset();
        return;
    }

    if ev.code == KEY_BACKSPACE {
        if let Some(previous) = input.undo.take() {
            input.pending_undo = Some(previous);
            return;
        }
        expander.pop_char();
        return;
    }

    if ev.code == KEY_ENTER || ev.code == KEY_TAB {
        input.undo = None;
        input.pending_undo = None;
        let character = if ev.code == KEY_ENTER { '\n' } else { '\t' };
        if let Some(expansion) = expander.push_char(character) {
            queue_expansion(input, ev.code, expansion);
        }
        return;
    }

    // Use the actual XKB keymap to decode the keypress for any keyboard layout.
    if let Some(ch) = injector
        .keymap()
        .decode(ev.code as u32, input.shift_held, input.altgr_held)
    {
        input.undo = None;
        input.pending_undo = None;
        tracing::debug!(
            "key {} (shift={} altgr={}) -> {:?}",
            ev.code,
            input.shift_held,
            input.altgr_held,
            ch
        );
        if let Some(expansion) = expander.push_char(ch) {
            tracing::info!(
                "Trigger matched; waiting for key release ({} backspaces + {} chars)",
                expansion.delete_count,
                expansion.text.len()
            );
            queue_expansion(input, ev.code, expansion);
        }
    }
}

fn queue_expansion(
    input: &mut InputState,
    release_code: u16,
    expansion: crate::expander::Expansion,
) {
    input.pending_expansion = Some(PendingExpansion {
        release_code,
        key_released: false,
        expansion,
    });
}

fn complete_pending_expansion(
    injector: &Injector,
    config: &Arc<Mutex<Config>>,
    input: &mut InputState,
    released_code: u16,
) {
    let Some(pending) = input.pending_expansion.as_mut() else {
        return;
    };
    if released_code == pending.release_code {
        pending.key_released = true;
    }
    if !pending.key_released || input.shift_held || input.altgr_held {
        return;
    }
    let Some(pending) = input.pending_expansion.take() else {
        return;
    };
    tracing::info!(
        "Trigger key released; expanding ({} backspaces + {} chars)",
        pending.expansion.delete_count,
        pending.expansion.text.len()
    );
    input.undo = inject_expansion(injector, config, pending.expansion);
}

fn complete_undo(injector: &Injector, expander: &mut Expander, previous: Undo) {
    if let Err(error) = injector.undo_text(
        previous.replacement_len.saturating_sub(1),
        &previous.original,
    ) {
        tracing::error!("Could not undo expansion: {}", error);
        return;
    }
    expander.reset();
    tracing::info!("Undid previous expansion");
}

fn inject_expansion(
    injector: &Injector,
    config: &Arc<Mutex<Config>>,
    expansion: crate::expander::Expansion,
) -> Option<Undo> {
    let has_exclusions = !config.lock().unwrap().settings.app_exclusions.is_empty();
    if has_exclusions {
        match crate::app::detect() {
            Ok(app) if config.lock().unwrap().excludes_app(&app) => {
                tracing::info!(
                    class = app.class.as_deref().unwrap_or("<unknown>"),
                    title = app.title.as_deref().unwrap_or("<unknown>"),
                    "Expansion suppressed by app exclusion"
                );
                return None;
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(
                "Could not evaluate app exclusions; allowing expansion: {}",
                error
            ),
        }
    }
    injector.backspace(expansion.delete_count);
    type_with_fallback(injector, &expansion.text);
    injector.cursor_left(expansion.cursor_back);
    let undo_enabled = config.lock().unwrap().settings.undo_enabled;
    (undo_enabled && expansion.cursor_back == 0 && !expansion.text.contains('\n')).then(|| Undo {
        replacement_len: expansion.text.chars().count(),
        original: expansion.undo_text,
    })
}

fn type_with_fallback(injector: &Injector, text: &str) {
    if injector.backend() == "wayland" {
        match injector.type_wayland_text(text) {
            Ok(()) => return,
            Err(error) => tracing::warn!("Persistent Wayland text unavailable: {}", error),
        }
    }
    if injector.can_type(text) {
        injector.type_text(text);
    } else if let Err(error) = injector.type_unicode(text) {
        tracing::error!("Unicode fallback failed: {}", error);
    }
}

fn wayland_text_characters(config: &Config) -> String {
    let mut text = (' '..='~').collect::<String>();
    text.push('\n');
    text.push('\t');
    for item in &config.matches {
        text.push_str(&item.replace);
    }
    text
}

fn reload_config(config: &Arc<Mutex<Config>>, expander: &mut Expander, injector: &Injector) {
    match Config::load_default() {
        Ok(new_cfg) => {
            if new_cfg.settings.injection_backend
                != config.lock().unwrap().settings.injection_backend
            {
                tracing::warn!("injection_backend changes require a daemon restart");
            }
            expander.update_configured(
                new_cfg.matches.clone(),
                new_cfg.settings.trigger_mode,
                new_cfg.settings.terminator_chars(),
            );
            injector.set_delay_ms(new_cfg.settings.injection_delay_for(injector.backend()));
            injector.set_settle_ms(new_cfg.settings.injection_settle_ms);
            *config.lock().unwrap() = new_cfg;
            log_config_warnings(&config.lock().unwrap());
            tracing::info!("Config reloaded");
        }
        Err(e) => tracing::warn!("Failed to reload config: {}", e),
    }
}

fn log_config_warnings(config: &Config) {
    for warning in config.unreachable_triggers() {
        tracing::warn!(
            trigger = warning.trigger,
            source = %warning.source.display(),
            blocking_trigger = warning.blocking_trigger,
            blocking_source = %warning.blocking_source.display(),
            "Trigger is unreachable in immediate mode because its prefix expands first"
        );
    }
}
