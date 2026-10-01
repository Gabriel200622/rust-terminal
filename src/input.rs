//! egui event normalization and routing into the framework-independent core.

use eframe::egui;
use terminal_core::{
    Mode,
    input::{self as protocol, Key, Modifiers},
};

pub use protocol::{InputAction, InputEvent, RoutedInput, RoutingContext, route_events};
pub use protocol::{PointerButton, PointerEvent, PointerPosition, WheelUnit, route_pointer_events};

pub fn normalize_pointer_events(
    events: &[egui::Event],
    frame_modifiers: egui::Modifiers,
) -> Vec<PointerEvent> {
    events
        .iter()
        .filter_map(|event| {
            Some(match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: modifier,
                } => PointerEvent::Button {
                    position: PointerPosition { x: pos.x, y: pos.y },
                    button: match button {
                        egui::PointerButton::Primary => PointerButton::Primary,
                        egui::PointerButton::Middle => PointerButton::Middle,
                        egui::PointerButton::Secondary => PointerButton::Secondary,
                        _ => return None,
                    },
                    pressed: *pressed,
                    modifiers: modifiers(*modifier),
                },
                egui::Event::PointerMoved(pos) => PointerEvent::Moved {
                    position: PointerPosition { x: pos.x, y: pos.y },
                    modifiers: modifiers(frame_modifiers),
                },
                egui::Event::MouseWheel {
                    delta,
                    unit,
                    modifiers: modifier,
                    ..
                } => PointerEvent::Wheel {
                    delta: PointerPosition {
                        x: delta.x,
                        y: delta.y,
                    },
                    unit: match unit {
                        egui::MouseWheelUnit::Line => WheelUnit::Line,
                        egui::MouseWheelUnit::Page => WheelUnit::Page,
                        egui::MouseWheelUnit::Point => WheelUnit::Point,
                    },
                    modifiers: modifiers(*modifier),
                },
                _ => return None,
            })
        })
        .collect()
}

/// Removes pre-edit updates that repeat an already empty composition.
///
/// Some input methods answer every cursor-area update with another empty
/// pre-edit. The toolkit repaints for any event and then updates the cursor
/// area again, so the window would never go idle. A change to or from an
/// active composition is always kept.
pub fn drop_redundant_preedits(events: &mut Vec<egui::Event>, composing: &mut bool) {
    events.retain(|event| match event {
        egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
            let redundant = text.is_empty() && !*composing;
            *composing = !text.is_empty();
            !redundant
        }
        egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
            *composing = false;
            true
        }
        _ => true,
    });
}

/// Normalizes a frame without interpreting shortcuts or touching a session.
pub fn normalize_events(
    events: &[egui::Event],
    frame_modifiers: egui::Modifiers,
) -> Vec<InputEvent> {
    let frame_modifiers = modifiers(frame_modifiers);
    events
        .iter()
        .filter_map(|event| {
            Some(match event {
                egui::Event::Key {
                    key: value,
                    physical_key,
                    modifiers: value_modifiers,
                    pressed,
                    repeat,
                } => InputEvent::Key {
                    key: key(*value),
                    physical_key: physical_key.map(key),
                    modifiers: modifiers(*value_modifiers),
                    pressed: *pressed,
                    repeat: *repeat,
                },
                egui::Event::Text(text) => InputEvent::Text {
                    text: text.clone(),
                    modifiers: frame_modifiers,
                },
                egui::Event::Paste(text) => InputEvent::Paste {
                    text: text.clone(),
                    modifiers: frame_modifiers,
                },
                egui::Event::Copy => InputEvent::Copy(frame_modifiers),
                egui::Event::Cut => InputEvent::Cut(frame_modifiers),
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    InputEvent::Preedit(text.clone())
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => InputEvent::Commit {
                    text: text.clone(),
                    modifiers: frame_modifiers,
                },
                egui::Event::WindowFocused(focused) => InputEvent::Focus(*focused),
                _ => return None,
            })
        })
        .collect()
}

pub fn key(key: egui::Key) -> Key {
    match key {
        egui::Key::ArrowDown => Key::ArrowDown,
        egui::Key::ArrowLeft => Key::ArrowLeft,
        egui::Key::ArrowRight => Key::ArrowRight,
        egui::Key::ArrowUp => Key::ArrowUp,
        egui::Key::Escape => Key::Escape,
        egui::Key::Tab => Key::Tab,
        egui::Key::Backspace => Key::Backspace,
        egui::Key::Enter => Key::Enter,
        egui::Key::Space => Key::Space,
        egui::Key::Insert => Key::Insert,
        egui::Key::Delete => Key::Delete,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::PageUp => Key::PageUp,
        egui::Key::PageDown => Key::PageDown,
        egui::Key::Copy => Key::Copy,
        egui::Key::Cut => Key::Cut,
        egui::Key::Paste => Key::Paste,
        egui::Key::Colon => Key::Colon,
        egui::Key::Comma => Key::Comma,
        egui::Key::Backslash => Key::Backslash,
        egui::Key::Slash => Key::Slash,
        egui::Key::Pipe => Key::Pipe,
        egui::Key::Questionmark => Key::Questionmark,
        egui::Key::Exclamationmark => Key::Exclamationmark,
        egui::Key::OpenBracket => Key::OpenBracket,
        egui::Key::CloseBracket => Key::CloseBracket,
        egui::Key::OpenCurlyBracket => Key::OpenCurlyBracket,
        egui::Key::CloseCurlyBracket => Key::CloseCurlyBracket,
        egui::Key::Backtick => Key::Backtick,
        egui::Key::Minus => Key::Minus,
        egui::Key::Period => Key::Period,
        egui::Key::Plus => Key::Plus,
        egui::Key::Equals => Key::Equals,
        egui::Key::Semicolon => Key::Semicolon,
        egui::Key::Quote => Key::Quote,
        egui::Key::Num0 => Key::Num0,
        egui::Key::Num1 => Key::Num1,
        egui::Key::Num2 => Key::Num2,
        egui::Key::Num3 => Key::Num3,
        egui::Key::Num4 => Key::Num4,
        egui::Key::Num5 => Key::Num5,
        egui::Key::Num6 => Key::Num6,
        egui::Key::Num7 => Key::Num7,
        egui::Key::Num8 => Key::Num8,
        egui::Key::Num9 => Key::Num9,
        egui::Key::A => Key::A,
        egui::Key::B => Key::B,
        egui::Key::C => Key::C,
        egui::Key::D => Key::D,
        egui::Key::E => Key::E,
        egui::Key::F => Key::F,
        egui::Key::G => Key::G,
        egui::Key::H => Key::H,
        egui::Key::I => Key::I,
        egui::Key::J => Key::J,
        egui::Key::K => Key::K,
        egui::Key::L => Key::L,
        egui::Key::M => Key::M,
        egui::Key::N => Key::N,
        egui::Key::O => Key::O,
        egui::Key::P => Key::P,
        egui::Key::Q => Key::Q,
        egui::Key::R => Key::R,
        egui::Key::S => Key::S,
        egui::Key::T => Key::T,
        egui::Key::U => Key::U,
        egui::Key::V => Key::V,
        egui::Key::W => Key::W,
        egui::Key::X => Key::X,
        egui::Key::Y => Key::Y,
        egui::Key::Z => Key::Z,
        egui::Key::F1 => Key::F1,
        egui::Key::F2 => Key::F2,
        egui::Key::F3 => Key::F3,
        egui::Key::F4 => Key::F4,
        egui::Key::F5 => Key::F5,
        egui::Key::F6 => Key::F6,
        egui::Key::F7 => Key::F7,
        egui::Key::F8 => Key::F8,
        egui::Key::F9 => Key::F9,
        egui::Key::F10 => Key::F10,
        egui::Key::F11 => Key::F11,
        egui::Key::F12 => Key::F12,
        egui::Key::F13 => Key::F13,
        egui::Key::F14 => Key::F14,
        egui::Key::F15 => Key::F15,
        egui::Key::F16 => Key::F16,
        egui::Key::F17 => Key::F17,
        egui::Key::F18 => Key::F18,
        egui::Key::F19 => Key::F19,
        egui::Key::F20 => Key::F20,
        egui::Key::F21 => Key::F21,
        egui::Key::F22 => Key::F22,
        egui::Key::F23 => Key::F23,
        egui::Key::F24 => Key::F24,
        egui::Key::F25 => Key::F25,
        egui::Key::F26 => Key::F26,
        egui::Key::F27 => Key::F27,
        egui::Key::F28 => Key::F28,
        egui::Key::F29 => Key::F29,
        egui::Key::F30 => Key::F30,
        egui::Key::F31 => Key::F31,
        egui::Key::F32 => Key::F32,
        egui::Key::F33 => Key::F33,
        egui::Key::F34 => Key::F34,
        egui::Key::F35 => Key::F35,
        egui::Key::BrowserBack => Key::BrowserBack,
        egui::Key::ShiftLeft => Key::ShiftLeft,
        egui::Key::ShiftRight => Key::ShiftRight,
        egui::Key::ControlLeft => Key::ControlLeft,
        egui::Key::ControlRight => Key::ControlRight,
        egui::Key::AltLeft => Key::AltLeft,
        egui::Key::AltRight => Key::AltRight,
        egui::Key::SuperLeft => Key::SuperLeft,
        egui::Key::SuperRight => Key::SuperRight,
        egui::Key::IntlBackslash => Key::IntlBackslash,
    }
}

pub fn modifiers(value: egui::Modifiers) -> Modifiers {
    Modifiers {
        alt: value.alt,
        ctrl: value.ctrl,
        shift: value.shift,
        mac_cmd: value.mac_cmd,
        command: value.command,
    }
}

pub fn encode_key(value: egui::Key, modifier: egui::Modifiers, mode: Mode) -> Option<Vec<u8>> {
    protocol::encode_key(key(value), modifiers(modifier), mode)
}
pub fn encode_key_event(
    value: egui::Key,
    modifier: egui::Modifiers,
    mode: Mode,
    pressed: bool,
    repeat: bool,
) -> Option<Vec<u8>> {
    protocol::encode_key_event(key(value), modifiers(modifier), mode, pressed, repeat)
}
pub fn encode_key_event_with_text(
    value: egui::Key,
    modifier: egui::Modifiers,
    mode: Mode,
    pressed: bool,
    repeat: bool,
    text: Option<&str>,
) -> Option<Vec<u8>> {
    protocol::encode_key_event_with_text(
        key(value),
        modifiers(modifier),
        mode,
        pressed,
        repeat,
        text,
    )
}
pub fn encode_text(text: &str, modifier: egui::Modifiers, mode: Mode) -> Vec<u8> {
    protocol::encode_text(text, modifiers(modifier), mode)
}
pub fn mouse(
    button: u8,
    column: u16,
    row: u16,
    pressed: bool,
    modifier: egui::Modifiers,
    mode: Mode,
) -> Option<Vec<u8>> {
    protocol::mouse(button, column, row, pressed, modifiers(modifier), mode)
}
pub fn encode_mouse_motion(
    button: Option<u8>,
    column: u16,
    row: u16,
    modifier: egui::Modifiers,
    mode: Mode,
) -> Option<Vec<u8>> {
    protocol::encode_mouse_motion(button, column, row, modifiers(modifier), mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_empty_preedits_are_dropped_but_composition_changes_are_kept() {
        let preedit = |text: &str| {
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: text.into(),
                active_range_chars: None,
            })
        };
        let mut composing = false;

        // An idle input method repeating itself must not look like activity.
        let mut events = vec![preedit(""), egui::Event::Text("a".into()), preedit("")];
        drop_redundant_preedits(&mut events, &mut composing);
        assert_eq!(events, [egui::Event::Text("a".into())]);
        assert!(!composing);

        // Starting, updating and clearing a composition all reach the terminal.
        let mut events = vec![preedit("に"), preedit("にほ"), preedit(""), preedit("")];
        drop_redundant_preedits(&mut events, &mut composing);
        assert_eq!(events, [preedit("に"), preedit("にほ"), preedit("")]);
        assert!(!composing);

        // A commit ends the composition; the empty pre-edit after it is noise.
        let commit = egui::Event::Ime(egui::ImeEvent::Commit("日本".into()));
        let mut events = vec![preedit("にほん"), commit.clone(), preedit("")];
        drop_redundant_preedits(&mut events, &mut composing);
        assert_eq!(events, [preedit("にほん"), commit]);
        assert!(!composing);
    }

    #[test]
    fn normalization_preserves_layout_text_and_physical_key() {
        let events = [
            egui::Event::Key {
                key: egui::Key::E,
                physical_key: Some(egui::Key::Q),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Text("é".into()),
        ];
        let normalized = normalize_events(&events, egui::Modifiers::NONE);
        assert!(matches!(
            normalized[0],
            InputEvent::Key {
                key: Key::E,
                physical_key: Some(Key::Q),
                ..
            }
        ));
        let routed = route_events(
            RoutingContext::TerminalPane(42),
            &normalized,
            Mode::REPORT_ALL_KEYS_AS_ESC | Mode::REPORT_ASSOCIATED_TEXT,
        );
        assert_eq!(routed.len(), 1);
        assert_eq!(
            routed[0].action,
            InputAction::Write(b"\x1b[101;1;233u".to_vec())
        );
    }

    #[test]
    fn pointer_normalization_keeps_event_modifiers_and_scroll_units() {
        let events = [
            egui::Event::PointerButton {
                pos: egui::pos2(12.0, 34.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::SHIFT,
            },
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -2.0),
                unit: egui::MouseWheelUnit::Line,
                modifiers: egui::Modifiers::ALT,
            },
        ];
        let normalized = normalize_pointer_events(&events, egui::Modifiers::CTRL);
        assert_eq!(
            normalized,
            vec![
                PointerEvent::Button {
                    position: PointerPosition { x: 12.0, y: 34.0 },
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::SHIFT
                },
                PointerEvent::Wheel {
                    delta: PointerPosition { x: 0.0, y: -2.0 },
                    unit: WheelUnit::Line,
                    modifiers: Modifiers::ALT
                }
            ]
        );
    }
}
