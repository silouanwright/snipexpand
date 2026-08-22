use anyhow::{Context, Result};
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key};
use std::collections::HashMap;
use std::io::Read;
use std::os::fd::IntoRawFd;
use std::os::unix::io::FromRawFd;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::thread;
use wayland_client::{
    protocol::{wl_keyboard, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};
use xkbcommon::xkb;

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

    /// Fallback: load the system default keymap via xkbcommon (no Wayland needed).
    pub fn build_default() -> Self {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        if let Some(keymap) =
            xkb::Keymap::new_from_names(&ctx, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
        {
            tracing::info!("Loaded system default XKB keymap");
            Self::build_from_xkb(&keymap)
        } else {
            tracing::error!("Failed to load default system keymap");
            Self {
                table: HashMap::new(),
                input_table: HashMap::new(),
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
    Key { code: u16, value: i32 },
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
}

impl Injector {
    pub fn spawn(delay_ms: u64, settle_ms: u64) -> Result<Self> {
        let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapLookup>();
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<InjectionCmd>(512);

        // Thread 1: get XKB keymap from Wayland compositor, then exit.
        // Falls back to system default keymap if Wayland is unavailable.
        let injection_delay_ms = Arc::new(AtomicU64::new(delay_ms));
        let thread_delay_ms = Arc::clone(&injection_delay_ms);
        thread::Builder::new()
            .name("snipexpand-keymap".into())
            .spawn(move || match wayland_keymap_thread(keymap_tx.clone()) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!(
                        "Wayland keymap unavailable ({}), falling back to system default",
                        e
                    );
                    let _ = keymap_tx.send(KeymapLookup::build_default());
                }
            })
            .context("Failed to spawn keymap thread")?;

        // Thread 2: uinput virtual keyboard for injection.
        thread::Builder::new()
            .name("snipexpand-uinput".into())
            .spawn(move || {
                if let Err(e) = uinput_thread(cmd_rx, thread_delay_ms) {
                    tracing::error!("uinput thread error: {}", e);
                }
            })
            .context("Failed to spawn uinput thread")?;

        let keymap = keymap_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .context("Timed out waiting for keymap")?;

        tracing::info!(
            "Keymap loaded, {} chars in lookup table",
            keymap.table.len()
        );
        Ok(Self {
            tx: cmd_tx,
            keymap,
            delay_ms: injection_delay_ms,
            settle_ms: AtomicU64::new(settle_ms),
        })
    }

    pub fn keymap(&self) -> &KeymapLookup {
        &self.keymap
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
        for _ in 0..count {
            let _ = self.tx.send(InjectionCmd::Key {
                code: 105,
                value: 1,
            }); // KEY_LEFT press
            let _ = self.tx.send(InjectionCmd::Key {
                code: 105,
                value: 0,
            }); // KEY_LEFT release
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

    /// Type through the compositor when the active XKB layout cannot represent
    /// every character. `wtype` creates a temporary keymap, so Unicode is not
    /// constrained by the physical keyboard layout.
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
        self.flush()?;
        let mut command = std::process::Command::new("wtype");
        for _ in 0..delete_count {
            command.args(["-k", "BackSpace"]);
        }
        let status = command
            .arg("--")
            .arg(original)
            .status()
            .context("run wtype undo sequence")?;
        if !status.success() {
            anyhow::bail!("wtype undo sequence exited with {status}");
        }
        Ok(())
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

// ---------------------------------------------------------------------------
// uinput injection thread
// ---------------------------------------------------------------------------

fn uinput_thread(cmd_rx: mpsc::Receiver<InjectionCmd>, delay_ms: Arc<AtomicU64>) -> Result<()> {
    // Register all common key codes (1–248 covers every standard key).
    let mut keys = AttributeSet::<Key>::new();
    for code in 1u16..=248 {
        keys.insert(Key::new(code));
    }

    let mut device = VirtualDeviceBuilder::new()
        .context("Failed to open /dev/uinput; is the 'input' group set?")?
        .name("snipexpand virtual keyboard")
        .with_keys(&keys)
        .context("UI_SET_KEYBIT failed")?
        .build()
        .context("UI_DEV_CREATE failed")?;

    tracing::info!("uinput virtual keyboard created");

    // Brief pause so keyboard.rs hotplug watcher sees and filters the new device
    // before we start injecting (prevents self-triggering).
    std::thread::sleep(std::time::Duration::from_millis(200));

    while let Ok(command) = cmd_rx.recv() {
        let InjectionCmd::Key { code, value } = command else {
            if let InjectionCmd::Flush(done) = command {
                let _ = done.send(());
            }
            continue;
        };
        let events = [
            InputEvent::new(EventType::KEY, code, value),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ];
        if let Err(e) = device.emit(&events) {
            tracing::error!("uinput emit error: {}", e);
        }
        // Each emit ends in SYN_REPORT. A 2 ms release gap gives the compositor
        // time to consume each transition without producing a visible typing effect.
        // Modifier releases get one extra millisecond to avoid state leakage.
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Wayland keymap thread. It reads only the keymap; no virtual keyboard protocol is needed.
// ---------------------------------------------------------------------------

struct WaylandKeymapState {
    seat: Option<wl_seat::WlSeat>,
    keymap_tx: Option<mpsc::Sender<KeymapLookup>>,
    keymap_sent: bool,
}

fn wayland_keymap_thread(keymap_tx: mpsc::Sender<KeymapLookup>) -> Result<()> {
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

    // Requesting keyboard causes the compositor to immediately send wl_keyboard.keymap.
    seat.get_keyboard(&qh, ());
    event_queue.roundtrip(&mut state)?;

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
        if let wl_keyboard::Event::Keymap {
            format: _,
            fd,
            size,
        } = event
        {
            if state.keymap_sent {
                return;
            }
            let keymap_str = {
                let mut file = unsafe { std::fs::File::from_raw_fd(fd.into_raw_fd()) };
                let mut s = String::with_capacity(size as usize);
                if let Err(error) = file.by_ref().take(size as u64).read_to_string(&mut s) {
                    tracing::warn!("Failed to read Wayland keymap: {}", error);
                }
                // Wayland sends the keymap with a null terminator; strip it before
                // passing to xkbcommon which uses CString internally.
                s.trim_end_matches('\0').to_string()
            };
            let mut lookup = KeymapLookup::build(&keymap_str);
            if lookup.table.is_empty() {
                tracing::warn!("Wayland keymap was unusable; falling back to system default");
                lookup = KeymapLookup::build_default();
            }
            if let Some(tx) = state.keymap_tx.take() {
                let _ = tx.send(lookup);
            }
            state.keymap_sent = true;
        }
    }
}
