use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent, Key,
};
use std::time::Duration;

fn emit_key(device: &mut VirtualDevice, code: u16) -> anyhow::Result<()> {
    device.emit(&[
        InputEvent::new(EventType::KEY, code, 1),
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
    ])?;
    std::thread::sleep(Duration::from_millis(30));
    device.emit(&[
        InputEvent::new(EventType::KEY, code, 0),
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
    ])?;
    std::thread::sleep(Duration::from_millis(30));
    Ok(())
}

fn emit_state(device: &mut VirtualDevice, code: u16, value: i32) -> anyhow::Result<()> {
    device.emit(&[InputEvent::new(EventType::KEY, code, value)])?;
    std::thread::sleep(Duration::from_millis(30));
    Ok(())
}

fn keycode(character: char) -> anyhow::Result<u16> {
    let normalized = character.to_ascii_lowercase();
    let code = match normalized {
        ';' => 39,
        'a'..='z' => [
            30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47,
            17, 45, 21, 44,
        ][(normalized as u8 - b'a') as usize],
        ' ' => 57,
        _ => anyhow::bail!("unsupported test character {character:?}"),
    };
    Ok(code)
}

fn emit_character(device: &mut VirtualDevice, character: char) -> anyhow::Result<()> {
    if character.is_ascii_uppercase() {
        emit_state(device, 42, 1)?; // Left Shift down
    }
    emit_key(device, keycode(character)?)?;
    if character.is_ascii_uppercase() {
        emit_state(device, 42, 0)?;
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let trigger = args.next().unwrap_or_else(|| ";snipexpand".to_string());
    let suffix = args.next().unwrap_or_default();
    let options: Vec<_> = args.collect();
    let undo = options.iter().any(|arg| arg == "--undo");
    let no_terminator = options.iter().any(|arg| arg == "--no-terminator");
    let mut keys = AttributeSet::<Key>::new();
    for code in 1u16..=248 {
        keys.insert(Key::new(code));
    }
    let mut device = VirtualDeviceBuilder::new()?
        .name("snipexpand e2e source")
        .with_keys(&keys)?
        .build()?;

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
