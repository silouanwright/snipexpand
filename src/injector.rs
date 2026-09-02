use anyhow::{Context, Result};
use evdev::{uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::thread;
use wayland_client::{
    protocol::{wl_keyboard, wl_registry, wl_seat},
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2, zwp_input_method_v2::ZwpInputMethodV2,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

use crate::config::InjectionBackend;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KeyInfo {
    pub evdev_code: u32,
    /// XKB level: 0=no mod, 1=Shift, 2=AltGr, 3=AltGr+Shift
    pub level: u32,
}

#[derive(Clone)]
pub struct KeymapLookup {
    table: HashMap<char, KeyInfo>,
    /// Reverse lookup: (evdev_code, xkb_level) → char, for decoding physical keypresses.
    input_table: HashMap<(u32, u32), char>,
}

struct KeymapData {
    lookup: KeymapLookup,
    text: String,
}

impl KeymapLookup {
    /// Build from an XKB keymap string received from the compositor.
    pub fn build(keymap_str: &str) -> Self {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let Some(keymap) = xkb::Keymap::new_from_string(
            &ctx,
            keymap_str.to_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            tracing::error!("Failed to parse XKB keymap string");
            return Self {
                table: HashMap::new(),
                input_table: HashMap::new(),
            };
        };
        Self::build_from_xkb(&keymap)
    }

    fn build_default_data() -> KeymapData {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        if let Some(keymap) =
            xkb::Keymap::new_from_names(&ctx, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
        {
            tracing::info!("Loaded system default XKB keymap");
            KeymapData {
                lookup: Self::build_from_xkb(&keymap),
                text: keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1),
            }
        } else {
            tracing::error!("Failed to load default system keymap");
            KeymapData {
                lookup: Self {
                    table: HashMap::new(),
                    input_table: HashMap::new(),
                },
                text: String::new(),
            }
        }
    }

    fn build_from_xkb(keymap: &xkb::Keymap) -> Self {
        let mut table: HashMap<char, KeyInfo> = HashMap::new();
        let mut input_table: HashMap<(u32, u32), char> = HashMap::new();
        keymap.key_for_each(|km, kc| {
            let xkb_code = kc.raw();
            if xkb_code < 8 {
                return;
            }
            let evdev_code = xkb_code - 8;
            // Only build the injection table from layout 0 (the active layout).
            // Iterating all layouts causes collisions: e.g. on QWERTZ Czech + English,
            // 'y' from the English layout gets stored with the evdev_code of the Czech 'z'
            // key, so injecting 'y' produces 'z' in the compositor. Layout 0 is authoritative.
            let layout = 0;
            let num_levels = km.num_levels_for_key(kc, layout);
            for level in 0..num_levels {
                for sym in km.key_get_syms_by_level(kc, layout, level) {
                    if let Some(ch) = keysym_to_char(*sym) {
                        table.entry(ch).or_insert(KeyInfo { evdev_code, level });
                        // Also record (evdev_code, level) → char for input decoding.
                        input_table.entry((evdev_code, level)).or_insert(ch);
                    }
                }
            }
        });
        Self { table, input_table }
    }

    pub fn lookup(&self, ch: char) -> Option<&KeyInfo> {
        self.table.get(&ch)
    }

    /// Decode a physical keypress to a char using the actual XKB keymap.
    /// `shift` = left/right Shift held; `altgr` = AltGr (right Alt) held.
    pub fn decode(&self, evdev_code: u32, shift: bool, altgr: bool) -> Option<char> {
        let level: u32 = match (shift, altgr) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        };
        self.input_table.get(&(evdev_code, level)).copied()
    }
}

fn keysym_to_char(keysym: xkb::Keysym) -> Option<char> {
    let cp = xkb::keysym_to_utf32(keysym);
    if cp == 0 {
        return None;
    }
    char::from_u32(cp)
}

// ---------------------------------------------------------------------------
// Injection commands
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum InjectionCmd {
    Key {
        code: u16,
        value: i32,
    },
    Text {
        text: String,
        compose_non_bmp: bool,
        compose_timing: ComposeTiming,
        done: mpsc::Sender<std::result::Result<(), String>>,
    },
    ReplaceWithInputMethod {
        original: String,
        text: String,
        done: mpsc::Sender<InputMethodCommitResult>,
    },
    RefreshTextKeymap {
        characters: String,
        done: mpsc::Sender<std::result::Result<(), String>>,
    },
    Flush(mpsc::Sender<()>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputMethodCommitResult {
    Committed,
    NotCommitted(String),
    Indeterminate(String),
}

// ---------------------------------------------------------------------------
// Injector main-thread handle
// ---------------------------------------------------------------------------

pub struct Injector {
    tx: mpsc::SyncSender<InjectionCmd>,
    keymap: KeymapLookup,
    delay_ms: Arc<AtomicU64>,
    settle_ms: AtomicU64,
    compose_delay_ms: AtomicU64,
    compose_settle_ms: AtomicU64,
    backend: &'static str,
}

pub(crate) struct InjectorOptions {
    pub(crate) backend: InjectionBackend,
    pub(crate) enable_input_method: bool,
    pub(crate) delay_ms: u64,
    pub(crate) wayland_delay_ms: Option<u64>,
    pub(crate) uinput_delay_ms: Option<u64>,
    pub(crate) settle_ms: u64,
    pub(crate) compose_timing: ComposeTiming,
    pub(crate) wayland_text_chars: String,
}

#[derive(Debug, PartialEq, Eq)]
enum CursorMove {
    None,
    Left(usize),
    Line { up: usize, column: usize },
}

impl Injector {
    pub fn spawn(options: InjectorOptions) -> Result<Self> {
        let InjectorOptions {
            backend,
            enable_input_method,
            delay_ms,
            wayland_delay_ms,
            uinput_delay_ms,
            settle_ms,
            compose_timing,
            wayland_text_chars,
        } = options;
        let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapData>();
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<InjectionCmd>(512);

        // Thread 1: get XKB keymap from Wayland compositor, then exit.
        // Falls back to system default keymap if Wayland is unavailable.
        let injection_delay_ms = Arc::new(AtomicU64::new(delay_ms));
        thread::Builder::new()
            .name("snipexpand-keymap".into())
            .spawn(move || match wayland_keymap_thread(keymap_tx.clone()) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!(
                        "Wayland keymap unavailable ({}), falling back to system default",
                        e
                    );
                    let _ = keymap_tx.send(KeymapLookup::build_default_data());
                }
            })
            .context("Failed to spawn keymap thread")?;

        let keymap = keymap_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .context("Timed out waiting for keymap")?;

        // Thread 2: selected virtual keyboard transport for injection.
        let thread_delay_ms = Arc::clone(&injection_delay_ms);
        let injection_keymap = keymap.text.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("snipexpand-injector".into())
            .spawn(move || {
                if let Err(e) = injection_thread(
                    cmd_rx,
                    thread_delay_ms,
                    backend,
                    enable_input_method,
                    &injection_keymap,
                    &wayland_text_chars,
                    ready_tx,
                ) {
                    tracing::error!("injection thread error: {}", e);
                }
            })
            .context("Failed to spawn injection thread")?;

        let active_backend = ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .context("Timed out waiting for injection backend")?
            .map_err(anyhow::Error::msg)?;
        let selected_delay = match active_backend {
            "wayland" => wayland_delay_ms,
            "uinput" => uinput_delay_ms,
            _ => None,
        }
        .unwrap_or(delay_ms);
        injection_delay_ms.store(selected_delay, Ordering::Relaxed);
        tracing::info!(backend = active_backend, "Injection backend ready");

        tracing::info!(
            "Keymap loaded, {} chars in lookup table",
            keymap.lookup.table.len()
        );
        Ok(Self {
            tx: cmd_tx,
            keymap: keymap.lookup,
            delay_ms: injection_delay_ms,
            settle_ms: AtomicU64::new(settle_ms),
            compose_delay_ms: AtomicU64::new(compose_timing.delay_ms),
            compose_settle_ms: AtomicU64::new(compose_timing.settle_ms),
            backend: active_backend,
        })
    }

    pub fn keymap(&self) -> &KeymapLookup {
        &self.keymap
    }

    pub fn backend(&self) -> &'static str {
        self.backend
    }

    pub fn set_delay_ms(&self, delay_ms: u64) {
        self.delay_ms.store(delay_ms, Ordering::Relaxed);
    }

    pub fn set_settle_ms(&self, settle_ms: u64) {
        self.settle_ms.store(settle_ms, Ordering::Relaxed);
    }

    pub fn set_compose_timing(&self, delay_ms: u64, settle_ms: u64) {
        self.compose_delay_ms.store(delay_ms, Ordering::Relaxed);
        self.compose_settle_ms.store(settle_ms, Ordering::Relaxed);
    }

    pub fn backspace(&self, count: usize) {
        self.backspace_after(count, self.settle_ms.load(Ordering::Relaxed));
    }

    pub fn backspace_for_compose(&self, count: usize) {
        let settle_ms = compose_delete_settle_ms(
            self.settle_ms.load(Ordering::Relaxed),
            self.compose_settle_ms.load(Ordering::Relaxed),
        );
        self.backspace_after(count, settle_ms);
    }

    pub fn backspace_without_settle(&self, count: usize) {
        self.backspace_after(count, 0);
    }

    fn backspace_after(&self, count: usize, settle_ms: u64) {
        sleep_ms(settle_ms);
        for _ in 0..count {
            let _ = self.tx.send(InjectionCmd::Key { code: 14, value: 1 }); // press
            let _ = self.tx.send(InjectionCmd::Key { code: 14, value: 0 }); // release
        }
    }

    pub fn cursor_left(&self, count: usize) {
        self.press_key(105, count); // KEY_LEFT
    }

    pub fn position_cursor(&self, text: &str, chars_after: usize) {
        match cursor_move(text, chars_after) {
            CursorMove::None => {}
            CursorMove::Left(count) => self.cursor_left(count),
            CursorMove::Line { up, column } => {
                self.press_key(103, up); // KEY_UP
                self.press_key(102, 1); // KEY_HOME
                self.press_key(106, column); // KEY_RIGHT
            }
        }
    }

    fn press_key(&self, code: u16, count: usize) {
        for _ in 0..count {
            let _ = self.tx.send(InjectionCmd::Key { code, value: 1 });
            let _ = self.tx.send(InjectionCmd::Key { code, value: 0 });
        }
    }

    pub fn type_text(&self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                let _ = self.tx.send(InjectionCmd::Key { code: 28, value: 1 }); // KEY_ENTER
                let _ = self.tx.send(InjectionCmd::Key { code: 28, value: 0 });
                continue;
            }
            match self.keymap.lookup(ch) {
                Some(ki) => {
                    let code = ki.evdev_code as u16;
                    // KEY_LEFTSHIFT=42, KEY_RIGHTALT=100 (AltGr)
                    let need_shift = ki.level == 1 || ki.level == 3;
                    let need_altgr = ki.level == 2 || ki.level == 3;
                    if need_altgr {
                        let _ = self.tx.send(InjectionCmd::Key {
                            code: 100,
                            value: 1,
                        });
                    }
                    if need_shift {
                        let _ = self.tx.send(InjectionCmd::Key { code: 42, value: 1 });
                    }
                    let _ = self.tx.send(InjectionCmd::Key { code, value: 1 });
                    let _ = self.tx.send(InjectionCmd::Key { code, value: 0 });
                    if need_shift {
                        let _ = self.tx.send(InjectionCmd::Key { code: 42, value: 0 });
                    }
                    if need_altgr {
                        let _ = self.tx.send(InjectionCmd::Key {
                            code: 100,
                            value: 0,
                        });
                    }
                }
                None => tracing::warn!("No keycode for char {:?}, skipping", ch),
            }
        }
    }

    pub fn can_type(&self, text: &str) -> bool {
        text.chars()
            .all(|ch| ch == '\n' || ch == '\t' || self.keymap.lookup(ch).is_some())
    }

    pub fn type_wayland_text(&self, text: &str, compose_non_bmp: bool) -> Result<()> {
        let (done, result) = mpsc::channel();
        let compose_timing = ComposeTiming {
            delay_ms: self.compose_delay_ms.load(Ordering::Relaxed),
            settle_ms: self.compose_settle_ms.load(Ordering::Relaxed),
        };
        let timeout = wayland_text_timeout(text, compose_non_bmp, compose_timing);
        self.tx
            .send(InjectionCmd::Text {
                text: text.to_string(),
                compose_non_bmp,
                compose_timing,
                done,
            })
            .context("Wayland injection thread stopped")?;
        result
            .recv_timeout(timeout)
            .context("timed out waiting for Wayland text injection")?
            .map_err(anyhow::Error::msg)
    }

    pub fn replace_with_input_method(&self, original: &str, text: &str) -> InputMethodCommitResult {
        sleep_ms(compose_delete_settle_ms(
            self.settle_ms.load(Ordering::Relaxed),
            self.compose_settle_ms.load(Ordering::Relaxed),
        ));
        let (done, result) = mpsc::channel();
        if self
            .tx
            .send(InjectionCmd::ReplaceWithInputMethod {
                original: original.to_string(),
                text: text.to_string(),
                done,
            })
            .is_err()
        {
            return InputMethodCommitResult::NotCommitted(
                "Wayland injection thread stopped".into(),
            );
        }
        result
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap_or_else(|error| {
                InputMethodCommitResult::Indeterminate(format!(
                    "timed out waiting for input-method-v2: {error}"
                ))
            })
    }

    pub fn refresh_wayland_text_keymap(&self, characters: String) -> Result<()> {
        if self.backend != "wayland" {
            return Ok(());
        }
        let (done, result) = mpsc::channel();
        self.tx
            .send(InjectionCmd::RefreshTextKeymap { characters, done })
            .context("Wayland injection thread stopped")?;
        result
            .recv_timeout(std::time::Duration::from_secs(2))
            .context("timed out refreshing Wayland text keymap")?
            .map_err(anyhow::Error::msg)
    }

    /// Last-resort compositor injection when the persistent startup text
    /// keymaps do not contain a newly configured character.
    pub fn type_unicode(&self, text: &str) -> Result<()> {
        self.flush()?;
        let status = std::process::Command::new("wtype")
            .arg("--")
            .arg(text)
            .status()
            .context("run wtype Unicode fallback")?;
        if !status.success() {
            anyhow::bail!("wtype Unicode fallback exited with {status}");
        }
        Ok(())
    }

    pub fn undo_text(&self, delete_count: usize, original: &str) -> Result<()> {
        if self.can_type(original) {
            self.backspace(delete_count);
            self.type_text(original);
            return self.flush();
        }

        self.backspace(delete_count);
        self.flush()?;
        self.type_unicode(original)
    }

    pub fn flush(&self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(InjectionCmd::Flush(tx))
            .context("uinput injection thread stopped")?;
        rx.recv_timeout(std::time::Duration::from_secs(2))
            .context("timed out waiting for injected keys")
    }
}

fn compose_delete_settle_ms(injection_settle_ms: u64, compose_settle_ms: u64) -> u64 {
    injection_settle_ms.max(compose_settle_ms)
}

fn cursor_move(text: &str, chars_after: usize) -> CursorMove {
    if chars_after == 0 {
        return CursorMove::None;
    }
    let marker = text.chars().count().saturating_sub(chars_after);
    let before = text.chars().take(marker).collect::<String>();
    let after = text.chars().skip(marker).collect::<String>();
    let up = after.chars().filter(|character| *character == '\n').count();
    if up == 0 {
        CursorMove::Left(chars_after)
    } else {
        CursorMove::Line {
            up,
            column: before
                .rsplit_once('\n')
                .map_or_else(|| before.chars().count(), |(_, line)| line.chars().count()),
        }
    }
}

// ---------------------------------------------------------------------------
// Injection transports
// ---------------------------------------------------------------------------

trait KeyboardTransport {
    fn send_key(&mut self, code: u16, value: i32) -> Result<()>;

    fn send_text(
        &mut self,
        _text: &str,
        _delay_ms: u64,
        _compose_non_bmp: bool,
        _compose_timing: ComposeTiming,
    ) -> Result<()> {
        anyhow::bail!("text injection is unsupported by this backend")
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn refresh_text_keymap(&mut self, _characters: &str) -> Result<()> {
        Ok(())
    }
}

struct UinputKeyboard {
    device: evdev::uinput::VirtualDevice,
}

impl UinputKeyboard {
    fn new() -> Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 1u16..=248 {
            keys.insert(KeyCode::new(code));
        }

        let device = VirtualDevice::builder()
            .context("Failed to open /dev/uinput; is the 'input' group set?")?
            .name("snipexpand virtual keyboard")
            .with_keys(&keys)
            .context("UI_SET_KEYBIT failed")?
            .build()
            .context("UI_DEV_CREATE failed")?;
        Ok(Self { device })
    }
}

impl KeyboardTransport for UinputKeyboard {
    fn send_key(&mut self, code: u16, value: i32) -> Result<()> {
        let events = [
            InputEvent::new(EventType::KEY.0, code, value),
            InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
        ];
        self.device.emit(&events).context("uinput emit")
    }
}

struct VirtualKeyboardState {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
}

struct InputMethodState {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpInputMethodManagerV2>,
    input_method: InputMethodLifecycle,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct InputMethodLifecycle {
    active: bool,
    pending_active: Option<bool>,
    surrounding: Option<SurroundingText>,
    pending_surrounding: Option<SurroundingText>,
    unavailable: bool,
    serial: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurroundingText {
    text: String,
    cursor: usize,
    anchor: usize,
}

impl InputMethodLifecycle {
    fn activate(&mut self) {
        self.pending_active = Some(true);
        self.pending_surrounding = None;
    }

    fn deactivate(&mut self) {
        self.pending_active = Some(false);
    }

    fn done(&mut self) {
        if let Some(active) = self.pending_active.take() {
            self.active = active;
            if !active {
                self.surrounding = None;
            } else {
                self.surrounding = self.pending_surrounding.take();
            }
        } else if let Some(surrounding) = self.pending_surrounding.take() {
            self.surrounding = Some(surrounding);
        }
        self.serial = self.serial.wrapping_add(1);
    }

    fn set_surrounding(&mut self, text: String, cursor: u32, anchor: u32) {
        self.pending_surrounding = Some(SurroundingText {
            text,
            cursor: cursor as usize,
            anchor: anchor as usize,
        });
    }

    fn make_unavailable(&mut self) {
        self.active = false;
        self.pending_active = None;
        self.surrounding = None;
        self.pending_surrounding = None;
        self.unavailable = true;
    }

    fn can_commit(&self) -> bool {
        self.active
            && self.pending_active.is_none()
            && self.pending_surrounding.is_none()
            && !self.unavailable
    }

    fn can_replace(&self, original: &str) -> bool {
        let Some(surrounding) = self.surrounding.as_ref() else {
            return false;
        };
        if surrounding.cursor != surrounding.anchor || surrounding.cursor > surrounding.text.len() {
            return false;
        }
        let start = surrounding.cursor.saturating_sub(original.len());
        surrounding.cursor >= original.len()
            && surrounding.text.as_bytes().get(start..surrounding.cursor)
                == Some(original.as_bytes())
    }
}

struct WaylandKeyboard {
    connection: Connection,
    queue_handle: QueueHandle<VirtualKeyboardState>,
    seat: wl_seat::WlSeat,
    manager: ZwpVirtualKeyboardManagerV1,
    keyboard: ZwpVirtualKeyboardV1,
    text_keyboards: Vec<ZwpVirtualKeyboardV1>,
    text_codes: HashMap<char, (usize, u16)>,
    keymap_lookup: KeymapLookup,
    started: std::time::Instant,
    depressed_modifiers: u32,
    shift_mask: u32,
    control_mask: u32,
    altgr_mask: u32,
}

struct InputMethodClient {
    event_queue: EventQueue<InputMethodState>,
    state: InputMethodState,
    input_method: ZwpInputMethodV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ComposeTiming {
    pub(crate) delay_ms: u64,
    pub(crate) settle_ms: u64,
}

fn wayland_text_timeout(
    text: &str,
    compose_non_bmp: bool,
    timing: ComposeTiming,
) -> std::time::Duration {
    let compose_ms = text
        .chars()
        .filter(|character| compose_non_bmp && *character as u32 > 0xffff)
        .fold(0u64, |total, character| {
            let keys = format!("{:x}", character as u32).len() as u64 + 2;
            total.saturating_add(
                keys.saturating_mul(timing.delay_ms)
                    .saturating_add(timing.settle_ms.saturating_mul(2)),
            )
        });
    std::time::Duration::from_secs(2).saturating_add(std::time::Duration::from_millis(compose_ms))
}

#[derive(Debug, PartialEq, Eq)]
enum WaylandTextStroke {
    Keymap(usize, u16),
    Unicode(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComposeKey {
    code: u16,
    control: bool,
    shift: bool,
    altgr: bool,
}

fn resolve_unicode_compose(keymap: &KeymapLookup, character: char) -> Result<Vec<ComposeKey>> {
    let resolve = |value: char| -> Result<ComposeKey> {
        let key = keymap
            .lookup(value)
            .with_context(|| format!("active keymap cannot type Unicode compose key {value:?}"))?;
        let (shift, altgr) = match key.level {
            0 => (false, false),
            1 => (true, false),
            2 => (false, true),
            3 => (true, true),
            level => anyhow::bail!(
                "active keymap requires unsupported level {level} for Unicode compose key {value:?}"
            ),
        };
        Ok(ComposeKey {
            code: u16::try_from(key.evdev_code)
                .context("Unicode compose keycode does not fit Linux input range")?,
            control: false,
            shift,
            altgr,
        })
    };

    let mut start = resolve('u')?;
    start.control = true;
    start.shift = true;
    let mut sequence = vec![start];
    sequence.extend(
        format!("{:x}", character as u32)
            .chars()
            .map(resolve)
            .collect::<Result<Vec<_>>>()?,
    );
    sequence.push(ComposeKey {
        code: 28,
        control: false,
        shift: false,
        altgr: false,
    });
    Ok(sequence)
}

impl WaylandKeyboard {
    fn new(keymap: &str, text_chars: &str) -> Result<Self> {
        if keymap.is_empty() {
            anyhow::bail!("cannot create Wayland virtual keyboard without a keymap");
        }
        let connection = Connection::connect_to_env().context("connect to Wayland display")?;
        let display = connection.display();
        let mut queue = connection.new_event_queue::<VirtualKeyboardState>();
        let qh = queue.handle();
        let mut state = VirtualKeyboardState {
            seat: None,
            manager: None,
        };
        display.get_registry(&qh, ());
        queue.roundtrip(&mut state)?;
        let seat = state
            .seat
            .as_ref()
            .context("compositor has no wl_seat")?
            .clone();
        let manager = state
            .manager
            .as_ref()
            .context("compositor has no virtual-keyboard-v1 support")?
            .clone();
        let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());

        let name = std::ffi::CString::new("snipexpand-keymap")?;
        let fd =
            nix::sys::memfd::memfd_create(name.as_c_str(), nix::sys::memfd::MFdFlags::MFD_CLOEXEC)?;
        let mut file = std::fs::File::from(fd);
        file.write_all(keymap.as_bytes())?;
        file.write_all(b"\0")?;
        keyboard.keymap(1, file.as_fd(), keymap.len() as u32 + 1);
        let mut text_keyboards = Vec::new();
        let mut text_codes = HashMap::new();
        for (keyboard_index, (text_keymap, codes)) in
            build_text_keymaps(text_chars).into_iter().enumerate()
        {
            let text_keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
            upload_keymap(&text_keyboard, &text_keymap)?;
            text_keyboards.push(text_keyboard);
            text_codes.extend(
                codes
                    .into_iter()
                    .map(|(character, code)| (character, (keyboard_index, code))),
            );
        }
        connection.roundtrip()?;

        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let parsed = xkb::Keymap::new_from_string(
            &context,
            keymap.to_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .context("parse virtual keyboard keymap")?;
        let modifier_mask = |name: &str| {
            let index = parsed.mod_get_index(name);
            if index != xkb::MOD_INVALID {
                1u32 << index
            } else {
                0
            }
        };

        Ok(Self {
            connection,
            queue_handle: qh,
            seat,
            manager,
            keyboard,
            text_keyboards,
            text_codes,
            keymap_lookup: KeymapLookup::build_from_xkb(&parsed),
            started: std::time::Instant::now(),
            depressed_modifiers: 0,
            shift_mask: modifier_mask(xkb::MOD_NAME_SHIFT),
            control_mask: modifier_mask(xkb::MOD_NAME_CTRL),
            altgr_mask: modifier_mask(xkb::MOD_NAME_ISO_LEVEL3_SHIFT),
        })
    }

    fn set_modifiers(&mut self, modifiers: u32) {
        self.depressed_modifiers = modifiers;
        self.keyboard.modifiers(modifiers, 0, 0, 0);
    }

    fn send_keyboard_key(&self, code: u16, state: wl_keyboard::KeyState) {
        let elapsed = self.started.elapsed().as_millis() as u32;
        self.keyboard.key(elapsed, code as u32, state.into());
    }

    fn send_unicode_compose(
        &mut self,
        sequence: &[ComposeKey],
        timing: ComposeTiming,
    ) -> Result<()> {
        if self.control_mask == 0 || self.shift_mask == 0 {
            anyhow::bail!("active keymap has no Ctrl or Shift modifier for Unicode compose");
        }
        if self.altgr_mask == 0 && sequence.iter().any(|key| key.altgr) {
            anyhow::bail!("active keymap has no AltGr modifier required by Unicode compose");
        }
        self.connection
            .roundtrip()
            .context("finish preceding Wayland input before Unicode compose")?;
        sleep_ms(timing.settle_ms);

        let result = (|| -> Result<()> {
            for key in sequence {
                let mut modifiers = 0;
                if key.control {
                    modifiers |= self.control_mask;
                }
                if key.shift {
                    modifiers |= self.shift_mask;
                }
                if key.altgr {
                    modifiers |= self.altgr_mask;
                }
                self.set_modifiers(modifiers);
                self.send_keyboard_key(key.code, wl_keyboard::KeyState::Pressed);
                self.send_keyboard_key(key.code, wl_keyboard::KeyState::Released);
                self.set_modifiers(0);
                self.connection
                    .roundtrip()
                    .context("dispatch Unicode compose key")?;
                sleep_ms(timing.delay_ms);
            }
            sleep_ms(timing.settle_ms);
            Ok(())
        })();

        // Clear synthetic modifiers even if dispatch failed mid-sequence. The
        // cleanup roundtrip is intentionally attempted before returning the
        // original error so Ctrl or Shift cannot leak into the next input.
        self.set_modifiers(0);
        let cleanup = self
            .connection
            .roundtrip()
            .context("release Unicode compose modifiers")
            .map(|_| ());
        result?;
        cleanup
    }
}

impl InputMethodClient {
    fn new() -> Result<Self> {
        let connection = Connection::connect_to_env().context("connect to Wayland display")?;
        let display = connection.display();
        let mut event_queue = connection.new_event_queue::<InputMethodState>();
        let qh = event_queue.handle();
        let mut state = InputMethodState {
            seat: None,
            manager: None,
            input_method: InputMethodLifecycle::default(),
        };
        display.get_registry(&qh, ());
        event_queue.roundtrip(&mut state)?;
        let seat = state.seat.as_ref().context("compositor has no wl_seat")?;
        let manager = state
            .manager
            .as_ref()
            .context("compositor has no input-method-v2 support")?;
        let input_method = manager.get_input_method(seat, &qh, ());
        event_queue
            .roundtrip(&mut state)
            .context("initialize input-method-v2")?;
        if state.input_method.unavailable {
            input_method.destroy();
            anyhow::bail!("input-method-v2 is already owned by another input method");
        }
        Ok(Self {
            event_queue,
            state,
            input_method,
        })
    }

    fn commit_replacement(&mut self, original: &str, text: &str) -> InputMethodCommitResult {
        let Ok(before_bytes) = u32::try_from(original.len()) else {
            return InputMethodCommitResult::NotCommitted(
                "matched trigger is too large for input-method-v2".into(),
            );
        };
        if text.len() > 4_000 {
            return InputMethodCommitResult::NotCommitted(
                "replacement exceeds the input-method-v2 4000-byte limit".into(),
            );
        }
        if let Err(error) = self.event_queue.roundtrip(&mut self.state) {
            return InputMethodCommitResult::NotCommitted(format!(
                "could not refresh input-method-v2 state: {error}"
            ));
        }
        if self.state.input_method.unavailable {
            return InputMethodCommitResult::NotCommitted(
                "input-method-v2 became unavailable".into(),
            );
        }
        if !self.state.input_method.can_commit() {
            return InputMethodCommitResult::NotCommitted(
                "the focused application has no active text-input-v3 context".into(),
            );
        }
        if !self.state.input_method.can_replace(original) {
            return InputMethodCommitResult::NotCommitted(
                "the focused client did not confirm the matched trigger as surrounding text".into(),
            );
        }

        self.input_method.delete_surrounding_text(before_bytes, 0);
        self.input_method.commit_string(text.to_string());
        self.input_method.commit(self.state.input_method.serial);
        match self.event_queue.roundtrip(&mut self.state) {
            Ok(_) if self.state.input_method.unavailable => InputMethodCommitResult::Indeterminate(
                "input-method-v2 became unavailable while committing the replacement".into(),
            ),
            Ok(_) => InputMethodCommitResult::Committed,
            Err(error) => InputMethodCommitResult::Indeterminate(format!(
                "input-method-v2 dispatch failed after queuing the replacement: {error}"
            )),
        }
    }

    fn is_unavailable(&self) -> bool {
        self.state.input_method.unavailable
    }
}

impl Drop for InputMethodClient {
    fn drop(&mut self) {
        self.input_method.destroy();
    }
}

impl KeyboardTransport for WaylandKeyboard {
    fn send_key(&mut self, code: u16, value: i32) -> Result<()> {
        let modifier = match code {
            42 | 54 => self.shift_mask,
            100 => self.altgr_mask,
            _ => 0,
        };
        if modifier != 0 {
            if value == 0 {
                self.set_modifiers(self.depressed_modifiers & !modifier);
            } else {
                self.set_modifiers(self.depressed_modifiers | modifier);
            }
            return self.connection.flush().context("flush Wayland modifiers");
        }
        let state = if value == 0 {
            wl_keyboard::KeyState::Released
        } else {
            wl_keyboard::KeyState::Pressed
        };
        let elapsed = self.started.elapsed().as_millis() as u32;
        self.keyboard.key(elapsed, code as u32, state.into());
        self.connection.flush().context("flush Wayland key event")
    }

    fn send_text(
        &mut self,
        text: &str,
        delay_ms: u64,
        compose_non_bmp: bool,
        compose_timing: ComposeTiming,
    ) -> Result<()> {
        // Resolve the complete string before emitting anything. Runtime errors
        // are not retried by the caller because the target may have accepted a
        // prefix before reporting a later dispatch failure.
        let sequence = resolve_wayland_text(&self.text_codes, text, compose_non_bmp)?;
        let compose_sequences = sequence
            .iter()
            .filter_map(|stroke| match stroke {
                WaylandTextStroke::Unicode(character) => {
                    Some(resolve_unicode_compose(&self.keymap_lookup, *character))
                }
                WaylandTextStroke::Keymap(_, _) => None,
            })
            .collect::<Result<Vec<_>>>()?;
        let mut compose_sequences = compose_sequences.iter();
        for stroke in sequence {
            match stroke {
                WaylandTextStroke::Keymap(keyboard_index, code) => {
                    let elapsed = self.started.elapsed().as_millis() as u32;
                    self.text_keyboards[keyboard_index].key(
                        elapsed,
                        code as u32,
                        wl_keyboard::KeyState::Pressed.into(),
                    );
                    self.text_keyboards[keyboard_index].key(
                        elapsed,
                        code as u32,
                        wl_keyboard::KeyState::Released.into(),
                    );
                }
                WaylandTextStroke::Unicode(_) => {
                    let compose = compose_sequences
                        .next()
                        .context("missing pre-resolved Unicode compose sequence")?;
                    self.send_unicode_compose(compose, compose_timing)?;
                }
            }
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        }
        self.connection
            .roundtrip()
            .context("wait for Wayland text dispatch")?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.connection
            .roundtrip()
            .context("wait for Wayland key dispatch")?;
        Ok(())
    }

    fn refresh_text_keymap(&mut self, characters: &str) -> Result<()> {
        let mut keyboards = Vec::new();
        let mut codes = HashMap::new();
        for (keyboard_index, (text_keymap, text_codes)) in
            build_text_keymaps(characters).into_iter().enumerate()
        {
            let keyboard = self
                .manager
                .create_virtual_keyboard(&self.seat, &self.queue_handle, ());
            upload_keymap(&keyboard, &text_keymap)?;
            keyboards.push(keyboard);
            codes.extend(
                text_codes
                    .into_iter()
                    .map(|(character, code)| (character, (keyboard_index, code))),
            );
        }
        self.connection
            .roundtrip()
            .context("activate refreshed Wayland text keymaps")?;
        for keyboard in self.text_keyboards.drain(..) {
            keyboard.destroy();
        }
        self.text_keyboards = keyboards;
        self.text_codes = codes;
        self.connection
            .flush()
            .context("retire previous Wayland text keymaps")
    }
}

fn sleep_ms(delay_ms: u64) {
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
}

fn resolve_wayland_text(
    text_codes: &HashMap<char, (usize, u16)>,
    text: &str,
    compose_non_bmp: bool,
) -> Result<Vec<WaylandTextStroke>> {
    text.chars()
        .map(|character| {
            if compose_non_bmp && character as u32 > 0xffff {
                Ok(WaylandTextStroke::Unicode(character))
            } else {
                text_codes
                    .get(&character)
                    .copied()
                    .map(|(keyboard, code)| WaylandTextStroke::Keymap(keyboard, code))
                    .with_context(|| format!("character {character:?} is not in the text keymap"))
            }
        })
        .collect()
}

fn upload_keymap(keyboard: &ZwpVirtualKeyboardV1, keymap: &str) -> Result<()> {
    let name = std::ffi::CString::new("snipexpand-text-keymap")?;
    let fd =
        nix::sys::memfd::memfd_create(name.as_c_str(), nix::sys::memfd::MFdFlags::MFD_CLOEXEC)?;
    let mut file = std::fs::File::from(fd);
    file.write_all(keymap.as_bytes())?;
    file.write_all(b"\0")?;
    keyboard.keymap(1, file.as_fd(), keymap.len() as u32 + 1);
    Ok(())
}

const SAFE_TEXT_CODES: &[u16] = &[
    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45,
    21, 44,
];

fn build_text_keymaps(text: &str) -> Vec<(String, HashMap<char, u16>)> {
    let mut chars = text.chars().collect::<Vec<_>>();
    chars.sort_unstable();
    chars.dedup();
    chars
        .chunks(SAFE_TEXT_CODES.len())
        .map(|chunk| build_text_keymap(chunk, SAFE_TEXT_CODES))
        .collect()
}

fn build_text_keymap(chars: &[char], safe_codes: &[u16]) -> (String, HashMap<char, u16>) {
    let mut codes = HashMap::new();
    let mut keycodes = String::new();
    let mut symbols = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        let code = safe_codes[index];
        let name = index + 1;
        codes.insert(ch, code);
        keycodes.push_str(&format!("<K{name}> = {};\n", code + 8));
        let keysym = match ch {
            '\n' => "Return".to_string(),
            '\t' => "Tab".to_string(),
            '\u{1b}' => "Escape".to_string(),
            _ => xkb::keysym_get_name(xkb::utf32_to_keysym(ch as u32)),
        };
        symbols.push_str(&format!("key <K{name}> {{[{keysym}]}};\n"));
    }
    let maximum = safe_codes.iter().max().copied().unwrap_or(1) + 8;
    (
        format!(
            "xkb_keymap {{\n\
             xkb_keycodes \"snipexpand\" {{ minimum = 8; maximum = {maximum}; {keycodes} }};\n\
             xkb_types \"snipexpand\" {{ include \"complete\" }};\n\
             xkb_compatibility \"snipexpand\" {{ include \"complete\" }};\n\
             xkb_symbols \"snipexpand\" {{ {symbols} }};\n\
             }};\n"
        ),
        codes,
    )
}

impl Dispatch<wl_registry::WlRegistry, ()> for VirtualKeyboardState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
            "zwp_virtual_keyboard_manager_v1" => {
                state.manager = Some(registry.bind(name, 1, qh, ()))
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        _: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        _: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for InputMethodState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
            "zwp_input_method_manager_v2" => state.manager = Some(registry.bind(name, 1, qh, ())),
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for InputMethodState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpInputMethodManagerV2, ()> for InputMethodState {
    fn event(
        _: &mut Self,
        _: &ZwpInputMethodManagerV2,
        _: <ZwpInputMethodManagerV2 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for InputMethodState {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodV2,
        event: <ZwpInputMethodV2 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::Event;

        match event {
            Event::Activate => state.input_method.activate(),
            Event::Deactivate => state.input_method.deactivate(),
            Event::SurroundingText {
                text,
                cursor,
                anchor,
            } => state.input_method.set_surrounding(text, cursor, anchor),
            Event::Done => state.input_method.done(),
            Event::Unavailable => state.input_method.make_unavailable(),
            _ => {}
        }
    }
}

fn injection_thread(
    cmd_rx: mpsc::Receiver<InjectionCmd>,
    delay_ms: Arc<AtomicU64>,
    requested: InjectionBackend,
    enable_input_method: bool,
    keymap: &str,
    wayland_text_chars: &str,
    ready: mpsc::SyncSender<std::result::Result<&'static str, String>>,
) -> Result<()> {
    let selected: Result<(Box<dyn KeyboardTransport>, &'static str)> = match requested {
        InjectionBackend::Wayland => WaylandKeyboard::new(keymap, wayland_text_chars)
            .map(|value| (Box::new(value) as _, "wayland")),
        InjectionBackend::Uinput => {
            UinputKeyboard::new().map(|value| (Box::new(value) as _, "uinput"))
        }
        InjectionBackend::Auto => match WaylandKeyboard::new(keymap, wayland_text_chars) {
            Ok(value) => Ok((Box::new(value) as _, "wayland")),
            Err(error) => {
                tracing::warn!("Wayland injection unavailable ({error}); falling back to uinput");
                UinputKeyboard::new().map(|value| (Box::new(value) as _, "uinput"))
            }
        },
    };
    let (mut keyboard, name) = match selected {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            let _ = ready.send(Err(message.clone()));
            anyhow::bail!(message);
        }
    };
    let _ = ready.send(Ok(name));

    let mut input_method = if enable_input_method {
        match InputMethodClient::new() {
            Ok(client) => {
                tracing::info!(
                    "input-method-v2 direct commits enabled; this process owns the seat input method"
                );
                Some(client)
            }
            Err(error) => {
                tracing::warn!(
                    "input-method-v2 direct commits unavailable ({error}); using keyboard fallback"
                );
                None
            }
        }
    } else {
        None
    };

    if name == "uinput" {
        // Let the hotplug watcher discover and exclude our virtual device.
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    while let Ok(command) = cmd_rx.recv() {
        match command {
            InjectionCmd::Text {
                text,
                compose_non_bmp,
                compose_timing,
                done,
            } => {
                let result = keyboard
                    .send_text(
                        &text,
                        delay_ms.load(Ordering::Relaxed),
                        compose_non_bmp,
                        compose_timing,
                    )
                    .map_err(|error| error.to_string());
                let _ = done.send(result);
            }
            InjectionCmd::ReplaceWithInputMethod {
                original,
                text,
                done,
            } => {
                let (result, retire) = input_method.as_mut().map_or_else(
                    || {
                        (
                            InputMethodCommitResult::NotCommitted(
                                "input-method-v2 was not enabled or is unavailable".into(),
                            ),
                            false,
                        )
                    },
                    |client| {
                        let result = client.commit_replacement(&original, &text);
                        (result, client.is_unavailable())
                    },
                );
                if retire {
                    input_method = None;
                }
                let _ = done.send(result);
            }
            InjectionCmd::RefreshTextKeymap { characters, done } => {
                let result = keyboard
                    .refresh_text_keymap(&characters)
                    .map_err(|error| error.to_string());
                let _ = done.send(result);
            }
            InjectionCmd::Flush(done) => {
                if let Err(error) = keyboard.flush() {
                    tracing::error!("{name} flush error: {error}");
                }
                let _ = done.send(());
            }
            InjectionCmd::Key { code, value } => {
                if let Err(e) = keyboard.send_key(code, value) {
                    tracing::error!("{name} injection error: {e}");
                }
                if value == 0 {
                    let base_delay = delay_ms.load(Ordering::Relaxed);
                    let delay = if matches!(code, 42 | 54 | 100) && base_delay > 0 {
                        base_delay + 1
                    } else {
                        base_delay
                    };
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Wayland keymap thread. It reads only the keymap; no virtual keyboard protocol is needed.
// ---------------------------------------------------------------------------

struct WaylandKeymapState {
    seat: Option<wl_seat::WlSeat>,
    keymap_tx: Option<mpsc::Sender<KeymapData>>,
    keymap_sent: bool,
}

fn read_wayland_keymap(fd: std::os::fd::OwnedFd, size: u32) -> Result<String> {
    const MAX_KEYMAP_SIZE: u32 = 16 * 1024 * 1024;
    if size == 0 || size > MAX_KEYMAP_SIZE {
        anyhow::bail!("invalid Wayland keymap size {size}");
    }
    let mut file = std::fs::File::from(fd);
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = vec![0; size as usize];
    file.read_exact(&mut bytes)?;
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    String::from_utf8(bytes).context("Wayland keymap was not UTF-8")
}

fn wayland_keymap_thread(keymap_tx: mpsc::Sender<KeymapData>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("Failed to connect to Wayland display (WAYLAND_DISPLAY not set?)")?;
    let display = conn.display();
    let mut event_queue = conn.new_event_queue::<WaylandKeymapState>();
    let qh = event_queue.handle();

    let mut state = WaylandKeymapState {
        seat: None,
        keymap_tx: Some(keymap_tx),
        keymap_sent: false,
    };

    display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state)?;
    event_queue.roundtrip(&mut state)?;

    let Some(seat) = state.seat.clone() else {
        anyhow::bail!("No wl_seat in compositor globals");
    };

    // Requesting a keyboard normally causes the compositor to immediately send
    // wl_keyboard.keymap. Retry transient empty or malformed startup maps.
    for _ in 0..3 {
        seat.get_keyboard(&qh, ());
        event_queue.roundtrip(&mut state)?;
        if state.keymap_sent {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    if !state.keymap_sent {
        anyhow::bail!("Compositor did not send a keyboard keymap");
    }
    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandKeymapState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        else {
            return;
        };
        if interface == "wl_seat" {
            let seat: wl_seat::WlSeat = registry.bind(name, 1, qh, ());
            state.seat = Some(seat);
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandKeymapState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandKeymapState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            if state.keymap_sent {
                return;
            }
            if format != WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                tracing::warn!(?format, "Ignoring unsupported Wayland keymap format");
                return;
            }
            let keymap_str = match read_wayland_keymap(fd, size) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!("Failed to read Wayland keymap: {}", error);
                    return;
                }
            };
            let data = KeymapData {
                lookup: KeymapLookup::build(&keymap_str),
                text: keymap_str,
            };
            if data.lookup.table.is_empty() {
                tracing::warn!(
                    size,
                    "Ignoring unusable Wayland keymap and waiting for another"
                );
                return;
            }
            if let Some(tx) = state.keymap_tx.take() {
                let _ = tx.send(data);
            }
            state.keymap_sent = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::OwnedFd;

    #[test]
    fn persistent_text_keymap_has_level_zero_unicode() {
        let maps = build_text_keymaps("A€¯ツ🧐");
        assert_eq!(maps.len(), 1);
        let (keymap, codes) = &maps[0];
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let parsed = xkb::Keymap::new_from_string(
            &context,
            keymap.clone(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("generated keymap should parse");
        let lookup = KeymapLookup::build_from_xkb(&parsed);
        for character in ['A', '€', '¯', 'ツ', '🧐'] {
            assert!(codes.contains_key(&character));
            assert!(SAFE_TEXT_CODES.contains(&codes[&character]));
            assert_eq!(lookup.lookup(character).unwrap().level, 0);
        }
    }

    #[test]
    fn wayland_keymap_reader_ignores_the_inherited_fd_offset() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"test keymap\0").unwrap();
        let size = file.metadata().unwrap().len() as u32;
        let fd: OwnedFd = file.into();

        assert_eq!(read_wayland_keymap(fd, size).unwrap(), "test keymap");
    }

    #[test]
    fn wayland_text_is_fully_resolved_before_injection() {
        let codes = HashMap::from([('a', (0, 30)), ('b', (0, 48))]);
        assert_eq!(
            resolve_wayland_text(&codes, "ab🦀", true).unwrap(),
            [
                WaylandTextStroke::Keymap(0, 30),
                WaylandTextStroke::Keymap(0, 48),
                WaylandTextStroke::Unicode('🦀')
            ]
        );
        let direct_codes = HashMap::from([('🦀', (1, 46))]);
        assert_eq!(
            resolve_wayland_text(&direct_codes, "🦀", false).unwrap(),
            [WaylandTextStroke::Keymap(1, 46)]
        );
    }

    #[test]
    fn unicode_compose_resolves_a_complete_modifier_safe_sequence() {
        let keymap = KeymapLookup {
            table: HashMap::from([
                (
                    'u',
                    KeyInfo {
                        evdev_code: 22,
                        level: 0,
                    },
                ),
                (
                    '1',
                    KeyInfo {
                        evdev_code: 2,
                        level: 1,
                    },
                ),
                (
                    'f',
                    KeyInfo {
                        evdev_code: 33,
                        level: 2,
                    },
                ),
                (
                    '9',
                    KeyInfo {
                        evdev_code: 10,
                        level: 0,
                    },
                ),
                (
                    'd',
                    KeyInfo {
                        evdev_code: 32,
                        level: 3,
                    },
                ),
                (
                    '0',
                    KeyInfo {
                        evdev_code: 11,
                        level: 0,
                    },
                ),
            ]),
            input_table: HashMap::new(),
        };
        assert_eq!(
            resolve_unicode_compose(&keymap, '🧐').unwrap(),
            [
                ComposeKey {
                    code: 22,
                    control: true,
                    shift: true,
                    altgr: false,
                },
                ComposeKey {
                    code: 2,
                    control: false,
                    shift: true,
                    altgr: false,
                },
                ComposeKey {
                    code: 33,
                    control: false,
                    shift: false,
                    altgr: true,
                },
                ComposeKey {
                    code: 10,
                    control: false,
                    shift: false,
                    altgr: false,
                },
                ComposeKey {
                    code: 32,
                    control: false,
                    shift: true,
                    altgr: true,
                },
                ComposeKey {
                    code: 11,
                    control: false,
                    shift: false,
                    altgr: false,
                },
                ComposeKey {
                    code: 28,
                    control: false,
                    shift: false,
                    altgr: false,
                },
            ]
        );
    }

    #[test]
    fn unicode_compose_fails_before_injection_when_a_hex_key_is_missing() {
        let keymap = KeymapLookup {
            table: HashMap::from([(
                'u',
                KeyInfo {
                    evdev_code: 22,
                    level: 0,
                },
            )]),
            input_table: HashMap::new(),
        };
        let error = resolve_unicode_compose(&keymap, '🙂').unwrap_err();
        assert!(error.to_string().contains("compose key '1'"));
    }

    #[test]
    fn wayland_text_timeout_includes_each_compose_sequence() {
        let timing = ComposeTiming {
            delay_ms: 5,
            settle_ms: 10,
        };
        assert_eq!(
            wayland_text_timeout("plain", true, timing),
            std::time::Duration::from_millis(2_000)
        );
        assert_eq!(
            wayland_text_timeout("🙂", true, timing),
            std::time::Duration::from_millis(2_055)
        );
        assert_eq!(
            wayland_text_timeout("🙂🧐", true, timing),
            std::time::Duration::from_millis(2_110)
        );
        assert_eq!(
            wayland_text_timeout("🙂", false, timing),
            std::time::Duration::from_millis(2_000)
        );
    }

    #[test]
    fn compose_deletion_keeps_the_safer_settle_interval() {
        assert_eq!(compose_delete_settle_ms(0, 10), 10);
        assert_eq!(compose_delete_settle_ms(20, 10), 20);
    }

    #[test]
    fn input_method_activation_is_applied_only_on_done() {
        let mut lifecycle = InputMethodLifecycle::default();
        assert!(!lifecycle.can_commit());

        lifecycle.activate();
        lifecycle.set_surrounding("before ;sm".into(), 10, 10);
        assert!(!lifecycle.can_commit());
        assert_eq!(lifecycle.serial, 0);

        lifecycle.done();
        assert!(lifecycle.can_commit());
        assert!(lifecycle.can_replace(";sm"));
        assert_eq!(lifecycle.serial, 1);
    }

    #[test]
    fn input_method_replacement_requires_an_exact_unselected_suffix() {
        let mut lifecycle = InputMethodLifecycle::default();
        lifecycle.activate();
        lifecycle.set_surrounding("mail ;🙂".into(), 10, 10);
        lifecycle.done();
        assert!(lifecycle.can_replace(";🙂"));
        assert!(!lifecycle.can_replace(";sm"));

        lifecycle.set_surrounding("mail ;🙂".into(), 10, 5);
        lifecycle.done();
        assert!(!lifecycle.can_replace(";🙂"));
    }

    #[test]
    fn input_method_deactivation_and_unavailable_are_fail_closed() {
        let mut lifecycle = InputMethodLifecycle::default();
        lifecycle.activate();
        lifecycle.set_surrounding(";sm".into(), 3, 3);
        lifecycle.done();
        assert!(lifecycle.can_commit());

        lifecycle.deactivate();
        assert!(!lifecycle.can_commit());
        lifecycle.done();
        assert!(!lifecycle.can_commit());
        assert_eq!(lifecycle.serial, 2);

        lifecycle.activate();
        lifecycle.make_unavailable();
        lifecycle.done();
        assert!(!lifecycle.can_commit());
    }

    #[test]
    fn multiline_cursor_moves_by_line_and_column() {
        assert_eq!(
            cursor_move("<details>\n<summary></summary>\n\n</details>", 11),
            CursorMove::Line { up: 1, column: 0 }
        );
        assert_eq!(
            cursor_move("foo\nbarbaz\nqux", 7),
            CursorMove::Line { up: 1, column: 3 }
        );
        assert_eq!(cursor_move("****", 2), CursorMove::Left(2));
    }
}
