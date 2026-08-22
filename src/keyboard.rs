use std::collections::HashSet;
use std::path::PathBuf;

use evdev::{Device, EventType, Key};

/// A single key event forwarded from the kernel via evdev.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub device: PathBuf,
    pub code: u16,  // Linux evdev keycode
    pub value: i32, // 0=release, 1=press, 2=repeat
}

#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    Key(KeyEvent),
    Disconnected(PathBuf),
}

/// Merges key events from all physical keyboards into a single async stream.
pub struct KeyboardStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<KeyboardEvent>,
    actor: tokio::task::JoinHandle<()>,
}

impl KeyboardStream {
    /// Discover keyboards, start per-device reader tasks, and start the hotplug watcher.
    pub async fn new() -> anyhow::Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<KeyboardEvent>();
        let (done_tx, done_rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

        // One actor owns the active-device set and periodically reconciles it.
        // This is reliable across udev event variants and makes shutdown immediate.
        let actor = tokio::spawn(hotplug_actor(tx, done_rx, done_tx));

        Ok(Self { rx, actor })
    }

    /// Returns the next key event, or `None` if all senders have been dropped.
    pub async fn next_event(&mut self) -> Option<KeyboardEvent> {
        self.rx.recv().await
    }
}

impl Drop for KeyboardStream {
    fn drop(&mut self) {
        self.actor.abort();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Scan `/dev/input/event*` and return paths + already-opened devices for confirmed keyboards.
fn discover_keyboards() -> anyhow::Result<Vec<(PathBuf, evdev::Device)>> {
    let mut keyboards = Vec::new();
    let dir = std::fs::read_dir("/dev/input")?;
    for entry in dir.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.starts_with("event") {
            continue;
        }
        if let Ok(device) = evdev::Device::open(&path) {
            if is_keyboard(&device) && !is_virtual(&device) {
                keyboards.push((path, device));
            }
        }
    }
    Ok(keyboards)
}

/// Returns `true` if `device` supports EV_KEY and has KEY_A, KEY_Z, and KEY_SPACE.
fn is_keyboard(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    device.supported_events().contains(EventType::KEY)
        && keys.contains(Key::KEY_A)
        && keys.contains(Key::KEY_Z)
        && keys.contains(Key::KEY_SPACE)
}

/// Returns `true` if the device name contains "virtual" (case-insensitive).
fn is_virtual(device: &Device) -> bool {
    device
        .name()
        .map(|n| n.to_ascii_lowercase().contains("virtual"))
        .unwrap_or(false)
}

/// Runs the event loop for a single keyboard device.
///
/// Accepts an already-opened device to avoid double-open.
/// Forwards key events to `tx`, and signals `done_tx` when it exits.
fn run_device_reader(
    mut device: evdev::Device,
    path: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<KeyboardEvent>,
    done_tx: tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    let result = read_device_loop(&mut device, &path, &tx);
    if let Err(e) = result {
        if e.downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::raw_os_error)
            == Some(19)
        {
            tracing::info!("Keyboard disconnected: {:?}", path);
        } else {
            tracing::warn!("Device reader exited for {:?}: {}", path, e);
        }
    }
    let _ = tx.send(KeyboardEvent::Disconnected(path.clone()));
    // Notify hotplug actor that this device is no longer active.
    let _ = done_tx.send(path);
}

fn read_device_loop(
    device: &mut evdev::Device,
    path: &std::path::Path,
    tx: &tokio::sync::mpsc::UnboundedSender<KeyboardEvent>,
) -> anyhow::Result<()> {
    loop {
        for event in device.fetch_events()? {
            if event.event_type() == evdev::EventType::KEY {
                tracing::debug!(code = event.code(), value = event.value(), "Keyboard event");
                if tx
                    .send(KeyboardEvent::Key(KeyEvent {
                        device: path.to_path_buf(),
                        code: event.code(),
                        value: event.value(),
                    }))
                    .is_err()
                {
                    return Ok(()); // receiver dropped
                }
            }
        }
    }
}

/// Reconciles `/dev/input` and spawns readers for new keyboard devices.
/// Also tracks active devices via `done_rx` so replug works correctly.
async fn hotplug_actor(
    tx: tokio::sync::mpsc::UnboundedSender<KeyboardEvent>,
    mut done_rx: tokio::sync::mpsc::UnboundedReceiver<PathBuf>,
    done_tx: tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    let mut active: HashSet<std::path::PathBuf> = HashSet::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));

    loop {
        tokio::select! {
            path = done_rx.recv() => {
                match path {
                    Some(p) => { active.remove(&p); tracing::info!("Device removed from active set: {:?}", p); }
                    None => break,
                }
            }
            _ = interval.tick() => {
                reconcile_keyboards(&mut active, &tx, &done_tx);
            }
        }
    }
}

fn reconcile_keyboards(
    active: &mut HashSet<PathBuf>,
    tx: &tokio::sync::mpsc::UnboundedSender<KeyboardEvent>,
    done_tx: &tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    let keyboards = match discover_keyboards() {
        Ok(keyboards) => keyboards,
        Err(error) => {
            tracing::warn!("Failed to scan keyboard devices: {}", error);
            return;
        }
    };

    for (path, device) in keyboards {
        if !active.insert(path.clone()) {
            continue;
        }
        tracing::info!(
            "Reading keyboard: {:?} ({})",
            path,
            device.name().unwrap_or("unknown")
        );
        let tx = tx.clone();
        let done_tx = done_tx.clone();
        std::thread::spawn(move || run_device_reader(device, path, tx, done_tx));
    }
}
