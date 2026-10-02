use anyhow::Result;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::{Config, FcitxSensitiveHint, NonBmpInput};
use crate::expander::Expander;
use crate::fcitx5::DirectCommitResult;
use crate::injector::{ComposeTiming, Injector, InjectorOptions};
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
    active_profile: Option<usize>,
    profile_initialized: bool,
    last_profile_check: Option<std::time::Instant>,
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
    let injector = Injector::spawn(InjectorOptions {
        backend: config.settings.injection_backend,
        enable_input_method: config.settings.requests_input_method(),
        delay_ms: config.settings.injection_delay_ms,
        wayland_delay_ms: config.settings.wayland_injection_delay_ms,
        uinput_delay_ms: config.settings.uinput_injection_delay_ms,
        settle_ms: config.settings.injection_settle_ms,
        compose_timing: ComposeTiming {
            delay_ms: config.settings.compose_delay_ms,
            settle_ms: config.settings.compose_settle_ms,
        },
        wayland_text_chars: wayland_text_characters(&config),
    })?;
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
            cfg.matches_for_profile(None),
            cfg.settings.trigger_mode,
            cfg.settings.terminator_chars(),
            cfg.settings.word_separator_chars(),
            cfg.settings.regex_max_buffer,
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
        let event_tx = tx.clone();
        let mut watcher = match RecommendedWatcher::new(
            move |result: notify::Result<notify::Event>| match result {
                Ok(event) if is_config_change(&event.kind) => {
                    let _ = event_tx.send(());
                }
                Ok(_) => {}
                Err(error) => tracing::warn!("Config watcher error: {}", error),
            },
            NConfig::default(),
        ) {
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
            // Editors often save through several create, rename, and modify
            // operations. Wait for that burst to settle and reload once.
            while rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_ok()
            {}
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
    let mut enabled = true;

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
                        if !enabled {
                            cancel_input_context(&mut expander, &mut input);
                            continue;
                        }
                        refresh_app_profile(&config, &mut expander, &injector, &mut input);
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
                if let Err(error) = reload_config(&config, &mut expander, &injector, &mut input) {
                    tracing::warn!("Failed to reload config: {error:#}");
                }
            }

            cmd = ipc_server.accept() => {
                match cmd {
                    Ok((IpcCmd::Reload, mut stream)) => {
                        tracing::info!("Reload requested via IPC");
                        let response = match reload_config(&config, &mut expander, &injector, &mut input) {
                            Ok(()) => "ok\n".to_string(),
                            Err(error) => format!("error: {}\n", format!("{error:#}").replace(['\r', '\n'], " ")),
                        };
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                    Ok((IpcCmd::Group(request), mut stream)) => {
                        let response = {
                            let mut current = config.lock().unwrap();
                            match handle_group_request(&Config::dir(), &request, &mut current, &mut expander, &mut input) {
                                Ok((response, effects)) => {
                                    if let Some(effects) = effects { apply_config_effects(&current, &injector, effects); }
                                    response
                                }
                                Err(error) => crate::groups::Response::Error { error: format!("{error:#}") },
                            }
                        };
                        if let Ok(mut wire) = serde_json::to_vec(&response) {
                            wire.push(b'\n');
                            let _ = stream.write_all(&wire).await;
                        }
                    }
                    Ok((IpcCmd::Status, mut stream)) => {
                        tracing::info!("Status requested via IPC");
                        let status = {
                            let cfg = config.lock().unwrap();
                            crate::ipc::DaemonStatus {
                                running: true,
                                enabled,
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
                    Ok((command @ (IpcCmd::Enable | IpcCmd::Disable | IpcCmd::Toggle), mut stream)) => {
                        enabled = match command {
                            IpcCmd::Enable => true,
                            IpcCmd::Disable => false,
                            IpcCmd::Toggle => !enabled,
                            _ => unreachable!(),
                        };
                        cancel_input_context(&mut expander, &mut input);
                        let response: &[u8] = if enabled { b"enabled\n" } else { b"disabled\n" };
                        let _ = stream.write_all(response).await;
                    }
                    Ok((IpcCmd::Paste { trigger, source }, mut stream)) => {
                        cancel_input_context(&mut expander, &mut input);
                        input.last_profile_check = None;
                        refresh_app_profile(&config, &mut expander, &injector, &mut input);
                        if !enabled {
                            let _ = stream.write_all(b"error: expansion is disabled\n").await;
                        } else if source.is_none() && expander.trigger_is_ambiguous(&trigger) {
                            let _ = stream.write_all(b"error: trigger is ambiguous; provide --source\n").await;
                        } else {
                            match expander.expansion_for_trigger(&trigger, source.as_deref()) {
                                Ok(Some(expansion)) => {
                                    input.undo = inject_expansion(&injector, &config, expansion);
                                    let _ = stream.write_all(b"ok\n").await;
                                }
                                Ok(None) => { let _ = stream.write_all(b"error: trigger not found\n").await; }
                                Err(error) => {
                                    tracing::warn!("Snippet rendering failed: {error:#}");
                                    let _ = stream.write_all(b"error: snippet rendering failed; see daemon log\n").await;
                                }
                            }
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
                if let Err(error) = reload_config(&config, &mut expander, &injector, &mut input) {
                    tracing::warn!("Failed to reload config: {error:#}");
                }
            }
        }
    }

    drop(kb_stream);
    tracing::info!("SnipExpand daemon stopped");
    Ok(())
}

fn is_config_change(kind: &notify::EventKind) -> bool {
    kind.is_create() || kind.is_modify() || kind.is_remove()
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
    let strategy = non_bmp_strategy(config, &expansion.text);
    let mut direct_commit_attempted = false;
    let compose_non_bmp = match strategy {
        NonBmpStrategy::Keymap => false,
        NonBmpStrategy::Compose => true,
        NonBmpStrategy::Fcitx5 {
            fallback_compose,
            allow_sensitive_hint,
        } => {
            direct_commit_attempted = true;
            match injector.replace_with_fcitx5(
                &expansion.undo_text,
                &expansion.text,
                allow_sensitive_hint,
            ) {
                DirectCommitResult::Committed => {
                    tracing::info!(
                        "Fcitx5 committed the expansion without a Unicode compose fallback"
                    );
                    injector.position_cursor(&expansion.text, expansion.cursor_back);
                    if let Err(error) = injector.flush() {
                        tracing::error!("Could not finish Fcitx5 expansion injection: {}", error);
                    }
                    let undo_enabled = config.lock().unwrap().settings.undo_enabled;
                    return (undo_enabled
                        && expansion.cursor_back == 0
                        && !expansion.text.contains('\n'))
                    .then(|| Undo {
                        replacement_len: expansion.text.chars().count(),
                        original: expansion.undo_text,
                    });
                }
                DirectCommitResult::NotCommitted(reason) => {
                    tracing::debug!(
                        "Fcitx5 direct commit unavailable ({reason}); using the keyboard fallback"
                    );
                }
                DirectCommitResult::Suppressed(reason) => {
                    tracing::info!("Expansion suppressed: {reason}");
                    return None;
                }
                DirectCommitResult::Indeterminate(reason) => {
                    tracing::error!(
                        "{reason}; refusing a keyboard retry because the target may already contain the replacement"
                    );
                    return None;
                }
            }
            fallback_compose
        }
        NonBmpStrategy::InputMethod { fallback_compose } => {
            direct_commit_attempted = true;
            match injector.replace_with_input_method(&expansion.undo_text, &expansion.text) {
                DirectCommitResult::Committed => {
                    injector.position_cursor(&expansion.text, expansion.cursor_back);
                    if let Err(error) = injector.flush() {
                        tracing::error!(
                            "Could not finish input-method-v2 expansion injection: {}",
                            error
                        );
                    }
                    let undo_enabled = config.lock().unwrap().settings.undo_enabled;
                    return (undo_enabled
                        && expansion.cursor_back == 0
                        && !expansion.text.contains('\n'))
                    .then(|| Undo {
                        replacement_len: expansion.text.chars().count(),
                        original: expansion.undo_text,
                    });
                }
                DirectCommitResult::NotCommitted(reason) => {
                    tracing::debug!(
                        "input-method-v2 direct commit unavailable ({reason}); using the keyboard fallback"
                    );
                }
                DirectCommitResult::Suppressed(reason) => {
                    tracing::info!("Expansion suppressed: {reason}");
                    return None;
                }
                DirectCommitResult::Indeterminate(reason) => {
                    tracing::error!(
                        "{reason}; refusing a keyboard retry because the target may already contain the replacement"
                    );
                    return None;
                }
            }
            fallback_compose
        }
    };
    if direct_commit_attempted {
        injector.backspace_without_settle(expansion.delete_count);
    } else if compose_non_bmp {
        injector.backspace_for_compose(expansion.delete_count);
    } else {
        injector.backspace(expansion.delete_count);
    }
    type_with_fallback(injector, &expansion.text, compose_non_bmp);
    injector.position_cursor(&expansion.text, expansion.cursor_back);
    if let Err(error) = injector.flush() {
        tracing::error!("Could not finish expansion injection: {}", error);
    }
    let undo_enabled = config.lock().unwrap().settings.undo_enabled;
    (undo_enabled && expansion.cursor_back == 0 && !expansion.text.contains('\n')).then(|| Undo {
        replacement_len: expansion.text.chars().count(),
        original: expansion.undo_text,
    })
}

fn type_with_fallback(injector: &Injector, text: &str, compose_non_bmp: bool) {
    if injector.backend() == "wayland" {
        match injector.type_wayland_text(text, compose_non_bmp) {
            Ok(()) => return,
            Err(error) if compose_non_bmp => {
                tracing::error!(
                    "Wayland text injection failed after entering compose mode; refusing an unsafe retry: {}",
                    error
                );
                return;
            }
            Err(error) => tracing::warn!("Persistent Wayland text unavailable: {}", error),
        }
    }
    if injector.can_type(text) {
        injector.type_text(text);
    } else if let Err(error) = injector.type_unicode(text) {
        tracing::error!("Unicode fallback failed: {}", error);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NonBmpStrategy {
    Keymap,
    Compose,
    Fcitx5 {
        fallback_compose: bool,
        allow_sensitive_hint: bool,
    },
    InputMethod {
        fallback_compose: bool,
    },
}

fn non_bmp_strategy(config: &Arc<Mutex<Config>>, text: &str) -> NonBmpStrategy {
    if !text.chars().any(|character| character as u32 > 0xffff) {
        return NonBmpStrategy::Keymap;
    }
    let app = crate::app::detect().ok();
    let cfg = config.lock().unwrap();
    let profile = app
        .as_ref()
        .and_then(|app| cfg.settings.profile_index(app))
        .and_then(|index| cfg.settings.app_profiles.get(index));
    let mode = profile
        .and_then(|profile| profile.non_bmp_input)
        .unwrap_or(cfg.settings.non_bmp_input);
    let sensitive_hint = profile
        .and_then(|profile| profile.fcitx_sensitive_hint)
        .unwrap_or(cfg.settings.fcitx_sensitive_hint);
    strategy_for_app(mode, sensitive_hint, app.as_ref())
}

fn strategy_for_app(
    mode: NonBmpInput,
    sensitive_hint: FcitxSensitiveHint,
    app: Option<&crate::app::AppInfo>,
) -> NonBmpStrategy {
    let fcitx5 = || NonBmpStrategy::Fcitx5 {
        fallback_compose: app.is_some_and(crate::app::AppInfo::uses_chromium_text_input),
        allow_sensitive_hint: sensitive_hint == FcitxSensitiveHint::Allow,
    };
    match mode {
        NonBmpInput::Keymap => NonBmpStrategy::Keymap,
        NonBmpInput::Compose => NonBmpStrategy::Compose,
        NonBmpInput::Fcitx5 => fcitx5(),
        NonBmpInput::InputMethod => NonBmpStrategy::InputMethod {
            fallback_compose: app.is_some_and(crate::app::AppInfo::uses_chromium_text_input),
        },
        NonBmpInput::Auto => {
            if app.is_some_and(crate::app::AppInfo::uses_chromium_text_input) {
                fcitx5()
            } else {
                NonBmpStrategy::Keymap
            }
        }
    }
}

fn wayland_text_characters(config: &Config) -> String {
    let mut text = (' '..='~').collect::<String>();
    text.push('\n');
    text.push('\t');
    for item in &config.matches {
        text.push_str(&item.replace);
        for variable in &item.vars {
            if let Some(echo) = &variable.params.echo {
                text.push_str(echo);
            }
        }
    }
    text
}

struct ConfigEffects {
    backend_changed: bool,
    input_method_changed: bool,
    text_characters: Option<String>,
}

/// Validate the entire candidate before changing any active matching state.
fn reload_matching_config(
    dir: &std::path::Path,
    current: &mut Config,
    expander: &mut Expander,
    input: &mut InputState,
) -> Result<ConfigEffects> {
    let new_cfg = Config::load_dir(dir)?;
    Ok(install_matching_config(new_cfg, current, expander, input))
}

fn install_matching_config(
    new_cfg: Config,
    current: &mut Config,
    expander: &mut Expander,
    input: &mut InputState,
) -> ConfigEffects {
    let backend_changed = new_cfg.settings.injection_backend != current.settings.injection_backend;
    let input_method_changed =
        new_cfg.settings.requests_input_method() != current.settings.requests_input_method();
    let characters = wayland_text_characters(&new_cfg);
    let changed_characters = (characters != wayland_text_characters(current)).then_some(characters);
    expander.update_configured(
        new_cfg.matches_for_profile(None),
        new_cfg.settings.trigger_mode,
        new_cfg.settings.terminator_chars(),
        new_cfg.settings.word_separator_chars(),
        new_cfg.settings.regex_max_buffer,
    );
    *current = new_cfg;
    cancel_input_context(expander, input);
    input.active_profile = None;
    input.profile_initialized = false;
    input.last_profile_check = None;
    ConfigEffects {
        backend_changed,
        input_method_changed,
        text_characters: changed_characters,
    }
}

fn reload_config(
    config: &Arc<Mutex<Config>>,
    expander: &mut Expander,
    injector: &Injector,
    input: &mut InputState,
) -> Result<()> {
    let mut current = config.lock().unwrap();
    let effects = reload_matching_config(&Config::dir(), &mut current, expander, input)?;
    apply_config_effects(&current, injector, effects);
    Ok(())
}

fn apply_config_effects(current: &Config, injector: &Injector, effects: ConfigEffects) {
    if effects.backend_changed {
        tracing::warn!("injection_backend changes require a daemon restart");
    }
    if effects.input_method_changed {
        tracing::warn!("input-method-v2 enablement changes require a daemon restart");
    }
    injector.set_compose_timing(
        current.settings.compose_delay_ms,
        current.settings.compose_settle_ms,
    );
    injector.set_delay_ms(current.settings.injection_delay_for(injector.backend()));
    injector.set_settle_ms(current.settings.injection_settle_ms);
    if let Some(characters) = effects.text_characters {
        if let Err(error) = injector.refresh_wayland_text_keymap(characters) {
            tracing::warn!("Could not refresh the Wayland Unicode keymap: {}", error);
        }
    }
    log_config_warnings(current);
    tracing::info!("Config reloaded");
}

fn handle_group_request(
    dir: &std::path::Path,
    request: &crate::groups::Request,
    current: &mut Config,
    expander: &mut Expander,
    input: &mut InputState,
) -> Result<(crate::groups::Response, Option<ConfigEffects>)> {
    let effects = if matches!(request, crate::groups::Request::List) {
        None
    } else {
        let candidate = crate::groups::change(dir, request)?;
        Some(install_matching_config(candidate, current, expander, input))
    };
    Ok((
        crate::groups::Response::Ok {
            groups: crate::groups::entries(current),
        },
        effects,
    ))
}

fn refresh_app_profile(
    config: &Arc<Mutex<Config>>,
    expander: &mut Expander,
    injector: &Injector,
    input: &mut InputState,
) {
    let has_profiles = !config.lock().unwrap().settings.app_profiles.is_empty();
    if !has_profiles {
        return;
    }
    if input
        .last_profile_check
        .is_some_and(|checked| checked.elapsed() < std::time::Duration::from_millis(250))
    {
        return;
    }
    input.last_profile_check = Some(std::time::Instant::now());
    let profile = match crate::app::detect() {
        Ok(app) => config.lock().unwrap().settings.profile_index(&app),
        Err(error) => {
            tracing::warn!("Could not evaluate app profiles: {}", error);
            None
        }
    };
    if input.profile_initialized && profile == input.active_profile {
        return;
    }

    let cfg = config.lock().unwrap();
    let selected = profile.and_then(|index| cfg.settings.app_profiles.get(index));
    let trigger_mode = selected
        .and_then(|profile| profile.trigger_mode)
        .unwrap_or(cfg.settings.trigger_mode);
    let terminators = selected
        .and_then(|profile| profile.terminators.as_ref())
        .map(|values| {
            values
                .iter()
                .map(|value| match value {
                    crate::config::Terminator::Space => ' ',
                    crate::config::Terminator::Enter => '\n',
                    crate::config::Terminator::Tab => '\t',
                })
                .collect()
        })
        .unwrap_or_else(|| cfg.settings.terminator_chars());
    let word_separators = selected
        .and_then(|profile| profile.word_separators.as_ref())
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .chars()
                        .next()
                        .expect("profile separators were validated")
                })
                .collect()
        })
        .or_else(|| cfg.settings.word_separator_chars());
    expander.update_configured(
        cfg.matches_for_profile(profile),
        trigger_mode,
        terminators,
        word_separators,
        cfg.settings.regex_max_buffer,
    );
    injector.set_delay_ms(
        selected
            .and_then(|profile| profile.injection_delay_ms)
            .unwrap_or_else(|| cfg.settings.injection_delay_for(injector.backend())),
    );
    injector.set_settle_ms(
        selected
            .and_then(|profile| profile.injection_settle_ms)
            .unwrap_or(cfg.settings.injection_settle_ms),
    );
    injector.set_compose_timing(
        selected
            .and_then(|profile| profile.compose_delay_ms)
            .unwrap_or(cfg.settings.compose_delay_ms),
        selected
            .and_then(|profile| profile.compose_settle_ms)
            .unwrap_or(cfg.settings.compose_settle_ms),
    );
    input.active_profile = profile;
    input.profile_initialized = true;
}

fn log_config_warnings(config: &Config) {
    for duplicate in config.duplicate_triggers() {
        tracing::warn!(
            trigger = duplicate.trigger,
            matches = duplicate.sources.len(),
            "Duplicate trigger requires a source selection or an app profile that leaves one match active"
        );
    }
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

    #[tokio::test]
    async fn group_commands_cross_the_real_socket_and_persist_without_desktop_io() {
        use crate::groups::{Request, Response};
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.yml"),
            "snippet_groups: [{name: work, match_files: [work]}]",
        )
        .unwrap();
        let socket = dir.path().join("daemon.sock");
        let server = IpcServer::new(&socket).await.unwrap();
        let mut current = Config::load_dir(dir.path()).unwrap();
        let mut engine = Expander::new(current.matches_for_profile(None), TriggerMode::Immediate);
        let mut input = InputState::default();
        for (request, expected) in [
            (Request::List, Some(true)),
            (
                Request::Disable {
                    name: "work".into(),
                },
                Some(false),
            ),
            (
                Request::Toggle {
                    name: "work".into(),
                },
                Some(true),
            ),
            (
                Request::Enable {
                    name: "missing".into(),
                },
                None,
            ),
        ] {
            let client_dir = dir.path().to_path_buf();
            let client_socket = socket.clone();
            let client = tokio::task::spawn_blocking(move || {
                crate::groups::command(&client_dir, Some(&client_socket), &request)
            });
            let (command, mut stream) = server.accept().await.unwrap();
            let IpcCmd::Group(request) = command else {
                panic!("expected group request")
            };
            let response = match handle_group_request(
                dir.path(),
                &request,
                &mut current,
                &mut engine,
                &mut input,
            ) {
                Ok((response, _)) => response,
                Err(error) => Response::Error {
                    error: format!("{error:#}"),
                },
            };
            let wire = format!("{}\n", serde_json::to_string(&response).unwrap());
            stream.write_all(wire.as_bytes()).await.unwrap();
            let result = client.await.unwrap();
            if let Some(enabled) = expected {
                assert_eq!(result.unwrap()[0].enabled, enabled);
                assert_eq!(crate::groups::entries(&current)[0].enabled, enabled);
                assert_eq!(
                    crate::groups::entries(&Config::load_dir(dir.path()).unwrap())[0].enabled,
                    enabled
                );
            } else {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("unknown snippet group"));
            }
        }
    }

    #[test]
    fn group_ipc_handler_commits_and_cancels_input_only_on_success() {
        use crate::groups::{Request, Response};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("match")).unwrap();
        std::fs::write(
            dir.path().join("config.yml"),
            "snippet_groups: [{name: work, match_files: [work.yml]}]",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("match/work.yml"),
            "matches: [{trigger: ab, replace: expanded}]",
        )
        .unwrap();
        let mut current = Config::load_dir(dir.path()).unwrap();
        let mut engine = Expander::new(current.matches_for_profile(None), TriggerMode::Immediate);
        let mut input = InputState::default();
        engine.push_char('a');
        queue_expansion(&mut input, 48, engine.push_char('b').unwrap());
        input.undo = Some(Undo {
            replacement_len: 8,
            original: "ab".into(),
        });
        engine.push_char('a');
        let (_, effects) = handle_group_request(
            dir.path(),
            &Request::List,
            &mut current,
            &mut engine,
            &mut input,
        )
        .unwrap();
        assert!(effects.is_none());
        assert!(input.pending_expansion.is_some());
        // A persistence failure must not change the running candidate or input.
        std::fs::create_dir(dir.path().join(".groups.lock")).unwrap();
        assert!(handle_group_request(
            dir.path(),
            &Request::Disable {
                name: "work".into()
            },
            &mut current,
            &mut engine,
            &mut input
        )
        .is_err());
        assert!(input.pending_expansion.is_some());
        assert_eq!(engine.push_char('b').unwrap().text, "expanded");
        assert_eq!(current.matches_for_profile(None).len(), 1);
        std::fs::rename(dir.path().join(".groups.lock"), dir.path().join("obstacle")).unwrap();
        let (response, effects) = handle_group_request(
            dir.path(),
            &Request::Disable {
                name: "work".into(),
            },
            &mut current,
            &mut engine,
            &mut input,
        )
        .unwrap();
        assert!(matches!(response, Response::Ok { groups } if !groups[0].enabled));
        assert!(effects.is_some());
        assert!(input.pending_expansion.is_none());
        assert!(input.undo.is_none());
        assert!(engine.expansion_for_trigger("ab", None).unwrap().is_none());
        assert!(current.matches_for_profile(None).is_empty());
        assert!(Config::load_dir(dir.path())
            .unwrap()
            .matches_for_profile(None)
            .is_empty());
        handle_group_request(
            dir.path(),
            &Request::Toggle {
                name: "work".into(),
            },
            &mut current,
            &mut engine,
            &mut input,
        )
        .unwrap();
        // Enabling does not complete a partial trigger from before the transition.
        assert!(engine.push_char('b').is_none());
        engine.push_char('a');
        assert_eq!(engine.push_char('b').unwrap().text, "expanded");
        assert!(engine.expansion_for_trigger("ab", None).unwrap().is_some());
    }

    #[test]
    fn reload_keeps_last_valid_config_and_cancels_stale_pending_input_on_success() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("match")).unwrap();
        let path = dir.path().join("match/test.yml");
        std::fs::write(&path, "matches: [{trigger: ab, replace: old}]").unwrap();
        let mut config = Config::load_dir(dir.path()).unwrap();
        let mut engine = Expander::new(config.matches.clone(), TriggerMode::Immediate);
        let mut input = InputState::default();
        engine.push_char('a');
        let expansion = engine.push_char('b').unwrap();
        queue_expansion(&mut input, 48, expansion);
        engine.push_char('a');
        std::fs::write(&path, "matches: [invalid YAML").unwrap();
        assert!(reload_matching_config(dir.path(), &mut config, &mut engine, &mut input).is_err());
        assert_eq!(config.matches[0].replace, "old");
        assert_eq!(engine.push_char('b').unwrap().text, "old");
        assert!(input.pending_expansion.is_some());
        // A read failure also preserves the last valid state.
        std::fs::rename(&path, dir.path().join("saved.yml")).unwrap();
        std::fs::create_dir(&path).unwrap();
        // Config loading traverses directories, so use a directory as config.yml.
        std::fs::create_dir(dir.path().join("config.yml")).unwrap();
        assert!(reload_matching_config(dir.path(), &mut config, &mut engine, &mut input).is_err());
        std::fs::rename(dir.path().join("config.yml"), dir.path().join("unused")).unwrap();
        std::fs::rename(&path, dir.path().join("unused-match-dir")).unwrap();
        std::fs::write(&path, "matches: [{trigger: ab, replace: new}]").unwrap();
        engine.push_char('a');
        reload_matching_config(dir.path(), &mut config, &mut engine, &mut input).unwrap();
        assert_eq!(config.matches[0].replace, "new");
        assert!(input.pending_expansion.is_none());
        assert!(engine.push_char('b').is_none());
    }

    #[test]
    fn echo_unicode_is_included_in_the_wayland_text_keymap() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("match")).unwrap();
        std::fs::write(dir.path().join("match/echo.yml"), "matches: [{trigger: ';echo', replace: '{{v}}', vars: [{name: v, type: echo, params: {echo: '猫🦀'}}]}]").unwrap();
        let config = Config::load_dir(dir.path()).unwrap();
        let characters = wayland_text_characters(&config);
        assert!(characters.contains('猫'));
        assert!(characters.contains('🦀'));
    }

    #[test]
    fn non_bmp_strategy_is_app_aware_and_can_be_overridden() {
        let chromium = crate::app::AppInfo {
            exec: Some("/usr/lib/chromium/chromium".into()),
            ..Default::default()
        };
        let terminal = crate::app::AppInfo {
            class: Some("foot".into()),
            ..Default::default()
        };
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Auto,
                FcitxSensitiveHint::Allow,
                Some(&chromium)
            ),
            NonBmpStrategy::Fcitx5 {
                fallback_compose: true,
                allow_sensitive_hint: true
            }
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Auto,
                FcitxSensitiveHint::Allow,
                Some(&terminal)
            ),
            NonBmpStrategy::Keymap
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Keymap,
                FcitxSensitiveHint::Allow,
                Some(&chromium)
            ),
            NonBmpStrategy::Keymap
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Compose,
                FcitxSensitiveHint::Allow,
                Some(&terminal)
            ),
            NonBmpStrategy::Compose
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Fcitx5,
                FcitxSensitiveHint::Allow,
                Some(&chromium)
            ),
            NonBmpStrategy::Fcitx5 {
                fallback_compose: true,
                allow_sensitive_hint: true
            }
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::Fcitx5,
                FcitxSensitiveHint::Suppress,
                Some(&terminal)
            ),
            NonBmpStrategy::Fcitx5 {
                fallback_compose: false,
                allow_sensitive_hint: false
            }
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::InputMethod,
                FcitxSensitiveHint::Allow,
                Some(&chromium)
            ),
            NonBmpStrategy::InputMethod {
                fallback_compose: true
            }
        );
        assert_eq!(
            strategy_for_app(
                NonBmpInput::InputMethod,
                FcitxSensitiveHint::Suppress,
                Some(&terminal)
            ),
            NonBmpStrategy::InputMethod {
                fallback_compose: false
            }
        );
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

    #[test]
    fn config_watcher_ignores_reads_but_accepts_writes() {
        use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};

        assert!(!is_config_change(&notify::EventKind::Access(
            AccessKind::Any
        )));
        assert!(!is_config_change(&notify::EventKind::Other));
        assert!(is_config_change(&notify::EventKind::Create(
            CreateKind::Any
        )));
        assert!(is_config_change(&notify::EventKind::Modify(
            ModifyKind::Any
        )));
        assert!(is_config_change(&notify::EventKind::Remove(
            RemoveKind::Any
        )));
    }
}
