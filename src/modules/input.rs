use std::thread;
use std::time::Duration;

/// Simulate mouse movement to absolute pixel coordinates.
pub fn simulate_mouse_move(x: f64, y: f64) {
    let event = rdev::EventType::MouseMove { x, y };
    send_event(event);
}

/// Simulate a mouse button press or release.
pub fn simulate_mouse_button(button: u8, pressed: bool) {
    let btn = u8_to_button(button);
    let event = if pressed {
        rdev::EventType::ButtonPress(btn)
    } else {
        rdev::EventType::ButtonRelease(btn)
    };
    send_event(event);
}

/// Simulate a scroll wheel event.
pub fn simulate_scroll(delta_x: i32, delta_y: i32) {
    let event = rdev::EventType::Wheel {
        delta_x: delta_x as i64,
        delta_y: delta_y as i64,
    };
    send_event(event);
}

/// Simulate a keyboard event from a key name string.
pub fn simulate_key(key_name: &str, pressed: bool) {
    if let Some(key) = parse_key(key_name) {
        let event = if pressed {
            rdev::EventType::KeyPress(key)
        } else {
            rdev::EventType::KeyRelease(key)
        };
        send_event(event);
    }
}

/// Send an rdev event with a small delay for reliability.
fn send_event(event: rdev::EventType) {
    if let Err(e) = rdev::simulate(&event) {
        eprintln!("Input simulation failed: {:?}", e);
    }
    // Small delay to ensure OS processes the event
    thread::sleep(Duration::from_micros(20));
}

/// Convert u8 button ID back to rdev::Button.
fn u8_to_button(b: u8) -> rdev::Button {
    match b {
        0 => rdev::Button::Left,
        1 => rdev::Button::Right,
        2 => rdev::Button::Middle,
        n => rdev::Button::Unknown(n as u16),
    }
}

/// Parse a key name (from rdev::Key debug format) back to an rdev::Key.
/// This provides cross-platform key identity via the debug string representation.
fn parse_key(name: &str) -> Option<rdev::Key> {
    // Common keys — extend as needed
    match name {
        // Letters
        "KeyA" => Some(rdev::Key::KeyA),
        "KeyB" => Some(rdev::Key::KeyB),
        "KeyC" => Some(rdev::Key::KeyC),
        "KeyD" => Some(rdev::Key::KeyD),
        "KeyE" => Some(rdev::Key::KeyE),
        "KeyF" => Some(rdev::Key::KeyF),
        "KeyG" => Some(rdev::Key::KeyG),
        "KeyH" => Some(rdev::Key::KeyH),
        "KeyI" => Some(rdev::Key::KeyI),
        "KeyJ" => Some(rdev::Key::KeyJ),
        "KeyK" => Some(rdev::Key::KeyK),
        "KeyL" => Some(rdev::Key::KeyL),
        "KeyM" => Some(rdev::Key::KeyM),
        "KeyN" => Some(rdev::Key::KeyN),
        "KeyO" => Some(rdev::Key::KeyO),
        "KeyP" => Some(rdev::Key::KeyP),
        "KeyQ" => Some(rdev::Key::KeyQ),
        "KeyR" => Some(rdev::Key::KeyR),
        "KeyS" => Some(rdev::Key::KeyS),
        "KeyT" => Some(rdev::Key::KeyT),
        "KeyU" => Some(rdev::Key::KeyU),
        "KeyV" => Some(rdev::Key::KeyV),
        "KeyW" => Some(rdev::Key::KeyW),
        "KeyX" => Some(rdev::Key::KeyX),
        "KeyY" => Some(rdev::Key::KeyY),
        "KeyZ" => Some(rdev::Key::KeyZ),
        // Numbers
        "Num0" => Some(rdev::Key::Num0),
        "Num1" => Some(rdev::Key::Num1),
        "Num2" => Some(rdev::Key::Num2),
        "Num3" => Some(rdev::Key::Num3),
        "Num4" => Some(rdev::Key::Num4),
        "Num5" => Some(rdev::Key::Num5),
        "Num6" => Some(rdev::Key::Num6),
        "Num7" => Some(rdev::Key::Num7),
        "Num8" => Some(rdev::Key::Num8),
        "Num9" => Some(rdev::Key::Num9),
        // Function keys
        "F1" => Some(rdev::Key::F1),
        "F2" => Some(rdev::Key::F2),
        "F3" => Some(rdev::Key::F3),
        "F4" => Some(rdev::Key::F4),
        "F5" => Some(rdev::Key::F5),
        "F6" => Some(rdev::Key::F6),
        "F7" => Some(rdev::Key::F7),
        "F8" => Some(rdev::Key::F8),
        "F9" => Some(rdev::Key::F9),
        "F10" => Some(rdev::Key::F10),
        "F11" => Some(rdev::Key::F11),
        "F12" => Some(rdev::Key::F12),
        // Modifiers
        "ShiftLeft" => Some(rdev::Key::ShiftLeft),
        "ShiftRight" => Some(rdev::Key::ShiftRight),
        "ControlLeft" => Some(rdev::Key::ControlLeft),
        "ControlRight" => Some(rdev::Key::ControlRight),
        "Alt" => Some(rdev::Key::Alt),
        "AltGr" => Some(rdev::Key::AltGr),
        "MetaLeft" => Some(rdev::Key::MetaLeft),
        "MetaRight" => Some(rdev::Key::MetaRight),
        // Special keys
        "Return" => Some(rdev::Key::Return),
        "Escape" => Some(rdev::Key::Escape),
        "BackSpace" => Some(rdev::Key::BackSpace),
        "Tab" => Some(rdev::Key::Tab),
        "Space" => Some(rdev::Key::Space),
        "CapsLock" => Some(rdev::Key::CapsLock),
        "Delete" => Some(rdev::Key::Delete),
        "UpArrow" => Some(rdev::Key::UpArrow),
        "DownArrow" => Some(rdev::Key::DownArrow),
        "LeftArrow" => Some(rdev::Key::LeftArrow),
        "RightArrow" => Some(rdev::Key::RightArrow),
        "Home" => Some(rdev::Key::Home),
        "End" => Some(rdev::Key::End),
        "PageUp" => Some(rdev::Key::PageUp),
        "PageDown" => Some(rdev::Key::PageDown),
        // Punctuation
        "Minus" => Some(rdev::Key::Minus),
        "Equal" => Some(rdev::Key::Equal),
        "LeftBracket" => Some(rdev::Key::LeftBracket),
        "RightBracket" => Some(rdev::Key::RightBracket),
        "BackSlash" => Some(rdev::Key::BackSlash),
        "SemiColon" => Some(rdev::Key::SemiColon),
        "Quote" => Some(rdev::Key::Quote),
        "BackQuote" => Some(rdev::Key::BackQuote),
        "Comma" => Some(rdev::Key::Comma),
        "Dot" => Some(rdev::Key::Dot),
        "Slash" => Some(rdev::Key::Slash),
        // Unknown key with raw code
        other => {
            // Try to parse "Unknown(NNN)" format
            if other.starts_with("Unknown(") && other.ends_with(')') {
                let code_str = &other[8..other.len() - 1];
                if let Ok(code) = code_str.parse::<u32>() {
                    return Some(rdev::Key::Unknown(code));
                }
            }
            eprintln!("Input: unrecognized key name: {}", other);
            None
        }
    }
}
