use anyhow::Result;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::expander::Expander;
use crate::injector::Injector;
use crate::ipc::{IpcCmd, IpcServer};
use crate::keyboard::{KeyboardEvent, KeyboardStream};

// evdev KEY codes (Linux input-event-codes.h)
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_ENTER: u16 = 28;
const MODIFIER_KEYS: &[u16] = &[
    29,  // KEY_LEFTCTRL
    42,  // KEY_LEFTSHIFT
    54,  // KEY_RIGHTSHIFT
    56,  // KEY_LEFTALT
    97,  // KEY_RIGHTCTRL
    100, // KEY_RIGHTALT / AltGr
    125, // KEY_LEFTMETA
    126, // KEY_RIGHTMETA
];
const SHORTCUT_MODIFIERS: &[u16] = &[
    29,  // KEY_LEFTCTRL
    56,  // KEY_LEFTALT
    97,  // KEY_RIGHTCTRL
    125, // KEY_LEFTMETA
    126, // KEY_RIGHTMETA
];
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
    held_modifiers: HashSet<(std::path::PathBuf, u16)>,
    undo: Option<Undo>,
    pending_undo: Option<Undo>,
    pending_expansion: Option<PendingExpansion>,
}

impl InputState {
    fn update_modifier(&mut self, device: &std::path::Path, code: u16, value: i32) {
        match value {
            0 => {
                self.held_modifiers.remove(&(device.to_path_buf(), code));
            }
            1 => {
                self.held_modifiers.insert((device.to_path_buf(), code));
            }
            _ => {}
        }
    }

    fn shift_held(&self) -> bool {
        self.held_modifiers
            .iter()
            .any(|(_, code)| matches!(code, 42 | 54))
    }

    fn altgr_held(&self) -> bool {
        self.held_modifiers.iter().any(|(_, code)| *code == 100)
    }

    fn shortcut_held(&self) -> bool {
        self.held_modifiers
            .iter()
            .any(|(_, code)| SHORTCUT_MODIFIERS.contains(code))
    }

    fn disconnect_device(&mut self, device: &std::path::Path) {
        self.held_modifiers
            .retain(|(held_device, _)| held_device != device);
    }
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
                    Some(KeyboardEvent::Key(ev)) => {
                        if MODIFIER_KEYS.contains(&ev.code) {
                            input.update_modifier(&ev.device, ev.code, ev.value);
                            if SHORTCUT_MODIFIERS.contains(&ev.code) {
                                cancel_input_context(&mut expander, &mut input);
                            } else if ev.value == 0 {
                                complete_pending_expansion(&injector, &config, &mut input, ev.code);
                            }
                            continue;
                        }
                        match ev.code {
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
                    Some(KeyboardEvent::Disconnected(device)) => {
                        input.disconnect_device(&device);
                        cancel_input_context(&mut expander, &mut input);
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
    if input.shortcut_held() {
        cancel_input_context(expander, input);
        return;
    }

    if RESET_KEYS.contains(&ev.code) {
        cancel_input_context(expander, input);
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
    if let Some(ch) =
        injector
            .keymap()
            .decode(ev.code as u32, input.shift_held(), input.altgr_held())
    {
        input.undo = None;
        input.pending_undo = None;
        tracing::debug!(
            "key {} (shift={} altgr={}) -> {:?}",
            ev.code,
            input.shift_held(),
            input.altgr_held(),
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
    } else {
        cancel_input_context(expander, input);
    }
}

fn cancel_input_context(expander: &mut Expander, input: &mut InputState) {
    input.undo = None;
    input.pending_undo = None;
    input.pending_expansion = None;
    expander.reset();
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
    if !pending.key_released || input.shift_held() || input.altgr_held() {
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
    if let Err(error) = injector.flush() {
        tracing::error!("Could not finish expansion injection: {}", error);
    }
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
            let (backend_changed, text_characters_changed) = {
                let current = config.lock().unwrap();
                (
                    new_cfg.settings.injection_backend != current.settings.injection_backend,
                    wayland_text_characters(&new_cfg) != wayland_text_characters(&current),
                )
            };
            if backend_changed {
                tracing::warn!("injection_backend changes require a daemon restart");
            }
            expander.update_configured(
                new_cfg.matches.clone(),
                new_cfg.settings.trigger_mode,
                new_cfg.settings.terminator_chars(),
            );
            injector.set_delay_ms(new_cfg.settings.injection_delay_for(injector.backend()));
            injector.set_settle_ms(new_cfg.settings.injection_settle_ms);
            if text_characters_changed {
                if let Err(error) =
                    injector.refresh_wayland_text_keymap(wayland_text_characters(&new_cfg))
                {
                    tracing::warn!("Could not refresh the Wayland Unicode keymap: {}", error);
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TriggerMode;

    fn expander(trigger: &str) -> Expander {
        Expander::new(
            vec![(trigger.to_string(), "expanded".to_string())],
            TriggerMode::Immediate,
        )
    }

    #[test]
    fn left_and_right_shift_are_tracked_independently() {
        let mut input = InputState::default();
        let keyboard = std::path::Path::new("/dev/input/event1");
        input.update_modifier(keyboard, 42, 1);
        input.update_modifier(keyboard, 54, 1);
        input.update_modifier(keyboard, 42, 0);
        assert!(input.shift_held());
        input.update_modifier(keyboard, 54, 0);
        assert!(!input.shift_held());
    }

    #[test]
    fn modifiers_are_tracked_per_keyboard_and_cleared_on_disconnect() {
        let mut input = InputState::default();
        let first = std::path::Path::new("/dev/input/event1");
        let second = std::path::Path::new("/dev/input/event2");
        input.update_modifier(first, 42, 1);
        input.update_modifier(second, 42, 1);
        input.update_modifier(first, 42, 0);
        assert!(input.shift_held());
        input.disconnect_device(second);
        assert!(!input.shift_held());
    }

    #[test]
    fn altgr_is_text_input_not_a_shortcut_modifier() {
        let mut input = InputState::default();
        input.update_modifier(std::path::Path::new("/dev/input/event1"), 100, 1);
        assert!(input.altgr_held());
        assert!(!input.shortcut_held());
    }

    #[test]
    fn shortcut_cancels_a_partial_trigger() {
        let mut expander = expander("ac");
        let mut input = InputState::default();
        assert!(expander.push_char('a').is_none());

        let keyboard = std::path::Path::new("/dev/input/event1");
        input.update_modifier(keyboard, 29, 1);
        cancel_input_context(&mut expander, &mut input);
        input.update_modifier(keyboard, 29, 0);

        assert!(expander.push_char('c').is_none());
    }

    #[test]
    fn cancel_clears_undo_and_pending_expansion_state() {
        let mut expander = expander("x");
        let mut input = InputState {
            undo: Some(Undo {
                replacement_len: 8,
                original: ";example".to_string(),
            }),
            pending_undo: Some(Undo {
                replacement_len: 8,
                original: ";example".to_string(),
            }),
            pending_expansion: Some(PendingExpansion {
                release_code: 45,
                key_released: false,
                expansion: crate::expander::Expansion {
                    delete_count: 1,
                    text: "expanded".to_string(),
                    cursor_back: 0,
                    undo_text: "x".to_string(),
                },
            }),
            ..InputState::default()
        };

        cancel_input_context(&mut expander, &mut input);

        assert!(input.undo.is_none());
        assert!(input.pending_undo.is_none());
        assert!(input.pending_expansion.is_none());
    }
}
