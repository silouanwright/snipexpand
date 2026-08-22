use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent, Key,
};
use std::sync::OnceLock;
use std::time::Duration;

fn event_delay() -> Duration {
    static DELAY: OnceLock<Duration> = OnceLock::new();
    *DELAY.get_or_init(|| {
        let millis = std::env::var("SNIPEXPAND_E2E_EVENT_DELAY_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(30);
        Duration::from_millis(millis)
    })
}

fn label_event_delay() -> Duration {
    static DELAY: OnceLock<Duration> = OnceLock::new();
    *DELAY.get_or_init(|| {
        let millis = std::env::var("SNIPEXPAND_E2E_LABEL_EVENT_DELAY_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(20);
        Duration::from_millis(millis)
    })
}

fn navigation_event_delay() -> Duration {
    static DELAY: OnceLock<Duration> = OnceLock::new();
    *DELAY.get_or_init(|| {
        let millis = std::env::var("SNIPEXPAND_E2E_NAVIGATION_EVENT_DELAY_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(120);
        Duration::from_millis(millis)
    })
}

fn emit_key_with_delay(
    device: &mut VirtualDevice,
    code: u16,
    delay: Duration,
) -> anyhow::Result<()> {
    device.emit(&[
        InputEvent::new(EventType::KEY, code, 1),
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
    ])?;
    std::thread::sleep(delay);
    device.emit(&[
        InputEvent::new(EventType::KEY, code, 0),
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
    ])?;
    std::thread::sleep(delay);
    Ok(())
}

fn emit_key(device: &mut VirtualDevice, code: u16) -> anyhow::Result<()> {
    emit_key_with_delay(device, code, event_delay())
}

fn emit_state_with_delay(
    device: &mut VirtualDevice,
    code: u16,
    value: i32,
    delay: Duration,
) -> anyhow::Result<()> {
    device.emit(&[InputEvent::new(EventType::KEY, code, value)])?;
    std::thread::sleep(delay);
    Ok(())
}

fn keycode(character: char) -> anyhow::Result<(u16, bool)> {
    let normalized = character.to_ascii_lowercase();
    let value = match character {
        '!' => (2, true),
        '#' => (4, true),
        ':' => (39, true),
        '<' => (51, true),
        '>' => (52, true),
        '^' => (7, true),
        _ => match normalized {
            ';' => (39, false),
            '-' => (12, false),
            '.' => (52, false),
            '/' => (53, false),
            '0' => (11, false),
            '1'..='9' => ((normalized as u16 - '1' as u16) + 2, false),
            'a'..='z' => (
                [
                    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20,
                    22, 47, 17, 45, 21, 44,
                ][(normalized as u8 - b'a') as usize],
                character.is_ascii_uppercase(),
            ),
            ' ' => (57, false),
            _ => anyhow::bail!("unsupported test character {character:?}"),
        },
    };
    Ok(value)
}

fn emit_character_with_delay(
    device: &mut VirtualDevice,
    character: char,
    delay: Duration,
) -> anyhow::Result<()> {
    let (code, shifted) = keycode(character)?;
    if shifted {
        emit_state_with_delay(device, 42, 1, delay)?; // Left Shift down
    }
    emit_key_with_delay(device, code, delay)?;
    if shifted {
        emit_state_with_delay(device, 42, 0, delay)?;
    }
    Ok(())
}

fn emit_character(device: &mut VirtualDevice, character: char) -> anyhow::Result<()> {
    emit_character_with_delay(device, character, event_delay())
}

fn recenter_nvim_insert(device: &mut VirtualDevice) -> anyhow::Result<()> {
    emit_key_with_delay(device, 88, navigation_event_delay()) // F12
}

fn create_device() -> anyhow::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for code in 1u16..=248 {
        keys.insert(Key::new(code));
    }
    Ok(VirtualDeviceBuilder::new()?
        .name("snipexpand e2e source")
        .with_keys(&keys)?
        .build()?)
}

fn run_batch(
    path: &str,
    pause_ms: u64,
    no_final_enter: bool,
    separator_enters: usize,
    nvim_conclusion_path: Option<&str>,
) -> anyhow::Result<()> {
    let trigger_file = std::fs::read_to_string(path)?;
    let triggers: Vec<_> = trigger_file
        .lines()
        .filter(|line| !line.is_empty())
        .collect();
    let mut device = create_device()?;
    std::thread::sleep(Duration::from_millis(1500));
    for (index, instruction) in triggers.iter().enumerate() {
        let (label, trigger) = instruction
            .split_once(" => ")
            .map_or((None, *instruction), |(label, trigger)| {
                (Some(label), trigger)
            });
        if let Some(label) = label {
            for character in format!("## {label}").chars() {
                emit_character_with_delay(&mut device, character, label_event_delay())?;
            }
            emit_key_with_delay(&mut device, 28, label_event_delay())?;
            std::thread::sleep(Duration::from_millis(300));
        }
        for character in trigger.chars() {
            emit_character(&mut device, character)?;
        }
        std::thread::sleep(Duration::from_millis(200));
        if !(no_final_enter && index + 1 == triggers.len()) {
            for _ in 0..separator_enters {
                emit_key(&mut device, 28)?;
            }
            if nvim_conclusion_path.is_some() && index + 3 >= triggers.len() {
                recenter_nvim_insert(&mut device)?;
            }
            std::thread::sleep(Duration::from_millis(pause_ms));
        }
    }
    if let Some(path) = nvim_conclusion_path {
        std::thread::sleep(Duration::from_millis(pause_ms));
        let navigation_delay = navigation_event_delay();
        emit_key_with_delay(&mut device, 1, navigation_delay)?; // Escape
        emit_character_with_delay(&mut device, 'G', navigation_delay)?;
        emit_character_with_delay(&mut device, 'o', navigation_delay)?;
        emit_key_with_delay(&mut device, 28, navigation_delay)?;
        recenter_nvim_insert(&mut device)?;

        let conclusion = std::fs::read_to_string(path)?;
        let lines: Vec<_> = conclusion.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            for character in line.chars() {
                emit_character_with_delay(&mut device, character, label_event_delay())?;
            }
            if index + 1 < lines.len() {
                emit_key_with_delay(&mut device, 28, label_event_delay())?;
            }
        }
        std::thread::sleep(Duration::from_millis(pause_ms));
    }
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let trigger = args.next().unwrap_or_else(|| ";snipexpand".to_string());
    if trigger == "--batch-file" {
        let path = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing batch path"))?;
        let pause_ms = args.next().map_or(Ok(100), |value| value.parse())?;
        let options: Vec<_> = args.collect();
        let no_final_enter = options.iter().any(|arg| arg == "--no-final-enter");
        let separator_enters = if options.iter().any(|arg| arg == "--blank-lines") {
            2
        } else {
            1
        };
        let nvim_conclusion_path = options
            .windows(2)
            .find(|pair| pair[0] == "--nvim-conclusion-file")
            .map(|pair| pair[1].as_str());
        return run_batch(
            &path,
            pause_ms,
            no_final_enter,
            separator_enters,
            nvim_conclusion_path,
        );
    }
    let suffix = args.next().unwrap_or_default();
    let options: Vec<_> = args.collect();
    let undo = options.iter().any(|arg| arg == "--undo");
    let no_terminator = options.iter().any(|arg| arg == "--no-terminator");
    let mut device = create_device()?;

    // Allow SnipExpand's device reconciler and the compositor to discover the device.
    std::thread::sleep(Duration::from_millis(1500));
    for character in trigger.chars().chain((!no_terminator).then_some(' ')) {
        emit_character(&mut device, character)?;
    }
    std::thread::sleep(Duration::from_millis(1500));
    if undo {
        emit_key(&mut device, 14)?; // Backspace undoes the expansion.
        std::thread::sleep(Duration::from_millis(1500));
    }
    for character in suffix.chars() {
        emit_character(&mut device, character)?;
    }
    emit_key(&mut device, 28)?; // Enter submits the expanded line.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
