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
    Connection, Dispatch, QueueHandle, WEnum,
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
        done: mpsc::Sender<std::result::Result<(), String>>,
    },
    RefreshTextKeymap {
        characters: String,
        done: mpsc::Sender<std::result::Result<(), String>>,
    },
    Flush(mpsc::Sender<()>),
}

// ---------------------------------------------------------------------------
// Injector main-thread handle
// ---------------------------------------------------------------------------

pub struct Injector {
    tx: mpsc::SyncSender<InjectionCmd>,
    keymap: KeymapLookup,
    delay_ms: Arc<AtomicU64>,
    settle_ms: AtomicU64,
    backend: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
enum CursorMove {
    None,
    Left(usize),
    Line { up: usize, column: usize },
}

impl Injector {
    pub fn spawn(
        backend: InjectionBackend,
        delay_ms: u64,
        wayland_delay_ms: Option<u64>,
        uinput_delay_ms: Option<u64>,
        settle_ms: u64,
        wayland_text_chars: String,
    ) -> Result<Self> {
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

    pub fn backspace(&self, count: usize) {
        std::thread::sleep(std::time::Duration::from_millis(
            self.settle_ms.load(Ordering::Relaxed),
        ));
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

    pub fn type_wayland_text(&self, text: &str) -> Result<()> {
        let (done, result) = mpsc::channel();
        self.tx
            .send(InjectionCmd::Text {
                text: text.to_string(),
                done,
            })
            .context("Wayland injection thread stopped")?;
        result
            .recv_timeout(std::time::Duration::from_secs(2))
            .context("timed out waiting for Wayland text injection")?
            .map_err(anyhow::Error::msg)
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

    fn send_text(&mut self, _text: &str, _delay_ms: u64) -> Result<()> {
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

struct WaylandKeyboard {
    connection: Connection,
    queue_handle: QueueHandle<VirtualKeyboardState>,
    seat: wl_seat::WlSeat,
    manager: ZwpVirtualKeyboardManagerV1,
    keyboard: ZwpVirtualKeyboardV1,
    text_keyboards: Vec<ZwpVirtualKeyboardV1>,
    text_codes: HashMap<char, (usize, u16)>,
    started: std::time::Instant,
    depressed_modifiers: u32,
    shift_mask: u32,
    altgr_mask: u32,
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
        let seat = state.seat.as_ref().context("compositor has no wl_seat")?;
        let manager = state
            .manager
            .as_ref()
            .context("compositor has no virtual-keyboard-v1 support")?;
        let keyboard = manager.create_virtual_keyboard(seat, &qh, ());

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
            let text_keyboard = manager.create_virtual_keyboard(seat, &qh, ());
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
            seat: seat.clone(),
            manager: manager.clone(),
            keyboard,
            text_keyboards,
            text_codes,
            started: std::time::Instant::now(),
            depressed_modifiers: 0,
            shift_mask: modifier_mask(xkb::MOD_NAME_SHIFT),
            altgr_mask: modifier_mask(xkb::MOD_NAME_ISO_LEVEL3_SHIFT),
        })
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
                self.depressed_modifiers &= !modifier;
            } else {
                self.depressed_modifiers |= modifier;
            }
            self.keyboard.modifiers(self.depressed_modifiers, 0, 0, 0);
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

    fn send_text(&mut self, text: &str, delay_ms: u64) -> Result<()> {
        // Resolve the complete string before emitting anything. If one
        // character is unavailable, the caller can safely fall back without
        // duplicating the already emitted prefix.
        let sequence = resolve_text_codes(&self.text_codes, text)?;
        for (keyboard_index, code) in sequence {
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

fn resolve_text_codes(
    text_codes: &HashMap<char, (usize, u16)>,
    text: &str,
) -> Result<Vec<(usize, u16)>> {
    text.chars()
        .map(|character| {
            text_codes
                .get(&character)
                .copied()
                .with_context(|| format!("character {character:?} is not in the text keymap"))
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

fn injection_thread(
    cmd_rx: mpsc::Receiver<InjectionCmd>,
    delay_ms: Arc<AtomicU64>,
    requested: InjectionBackend,
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

    if name == "uinput" {
        // Let the hotplug watcher discover and exclude our virtual device.
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    while let Ok(command) = cmd_rx.recv() {
        match command {
            InjectionCmd::Text { text, done } => {
                let result = keyboard
                    .send_text(&text, delay_ms.load(Ordering::Relaxed))
                    .map_err(|error| error.to_string());
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
        let maps = build_text_keymaps("A€¯ツ");
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
        for character in ['A', '€', '¯', 'ツ'] {
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
            resolve_text_codes(&codes, "ab").unwrap(),
            [(0, 30), (0, 48)]
        );
        assert!(resolve_text_codes(&codes, "ab🦀").is_err());
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
