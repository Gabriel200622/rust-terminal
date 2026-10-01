//! Framework-independent terminal keyboard and mouse protocols.
//!
//! Layout-produced text is authoritative. The desktop adapter supplies logical
//! keys and optional physical keys; missing keypad/lock information is never guessed.

use crate::Mode as TermMode;

/// Ownership is chosen before bytes are encoded or terminal effects are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingContext {
    Overlay,
    TextField,
    TerminalSearch,
    TerminalPane(u64),
}

/// Recorded keyboard/composition events, with framework-independent values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key {
        key: Key,
        physical_key: Option<Key>,
        modifiers: Modifiers,
        pressed: bool,
        repeat: bool,
    },
    Text {
        text: String,
        modifiers: Modifiers,
    },
    Paste {
        text: String,
        modifiers: Modifiers,
    },
    Copy(Modifiers),
    Cut(Modifiers),
    Preedit(String),
    Commit {
        text: String,
        modifiers: Modifiers,
    },
    Focus(bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Middle,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelUnit {
    Line,
    Page,
    Point,
}

/// Pointer events retain logical positions until the pane adapter maps cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEvent {
    Button {
        position: PointerPosition,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    Moved {
        position: PointerPosition,
        modifiers: Modifiers,
    },
    Wheel {
        delta: PointerPosition,
        unit: WheelUnit,
        modifiers: Modifiers,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutedPointer {
    pub target: u64,
    pub event: PointerEvent,
}

pub fn route_pointer_events(
    context: RoutingContext,
    events: &[PointerEvent],
) -> Vec<RoutedPointer> {
    let RoutingContext::TerminalPane(target) = context else {
        return Vec::new();
    };
    events
        .iter()
        .map(|event| RoutedPointer {
            target,
            event: *event,
        })
        .collect()
}

/// Effects carry their pane target at routing time, so focus changes cannot retarget them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedInput {
    pub target: u64,
    pub action: InputAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    Write(Vec<u8>),
    Paste(String),
    Copy,
    Preedit(String),
    ScrollPage { reverse: bool },
    Focus(bool),
}

/// Routes a batch exactly once, preferring layout text for AltGr and composition.
/// Pointer geometry belongs to the desktop adapter and uses the same selected owner.
pub fn route_events(
    context: RoutingContext,
    events: &[InputEvent],
    mode: TermMode,
) -> Vec<RoutedInput> {
    let RoutingContext::TerminalPane(target) = context else {
        return Vec::new();
    };
    let mut routed = Vec::new();
    let mut suppressed_text = None;
    let mut mirrored_clipboard = None;
    for (index, event) in events.iter().enumerate() {
        if suppressed_text == Some(index) {
            continue;
        }
        let action = match event {
            InputEvent::Key {
                key,
                modifiers,
                pressed,
                repeat,
                ..
            } => {
                mirrored_clipboard = None;
                if modifiers.mac_cmd {
                    continue;
                }
                if *pressed && modifiers.shift && matches!(key, Key::PageUp | Key::PageDown) {
                    Some(InputAction::ScrollPage {
                        reverse: *key == Key::PageUp,
                    })
                } else {
                    let text = if *pressed {
                        match events.get(index + 1) {
                            Some(InputEvent::Text { text, .. }) => Some(text.as_str()),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if modifiers.ctrl && modifiers.alt && text.is_some() {
                        continue;
                    }
                    encode_key_event_with_text(*key, *modifiers, mode, *pressed, *repeat, text).map(
                        |bytes| {
                            if text.is_some() {
                                suppressed_text = Some(index + 1);
                            }
                            if *pressed
                                && modifiers.ctrl
                                && !modifiers.shift
                                && matches!(key, Key::C | Key::V | Key::X)
                            {
                                mirrored_clipboard = Some(*key);
                            }
                            InputAction::Write(bytes)
                        },
                    )
                }
            }
            InputEvent::Text { text, modifiers } => {
                let bytes = encode_text(text, *modifiers, mode);
                (!bytes.is_empty()).then_some(InputAction::Write(bytes))
            }
            InputEvent::Commit { text, modifiers } => {
                if matches!(events.get(index+1),Some(InputEvent::Text {text:paired,..}) if paired==text)
                {
                    suppressed_text = Some(index + 1);
                }
                let bytes = encode_text(text, *modifiers, mode);
                routed.push(RoutedInput {
                    target,
                    action: InputAction::Preedit(String::new()),
                });
                (!bytes.is_empty()).then_some(InputAction::Write(bytes))
            }
            InputEvent::Preedit(text) => Some(InputAction::Preedit(text.clone())),
            InputEvent::Copy(modifiers) => {
                if mirrored_clipboard.take() == Some(Key::C) {
                    continue;
                }
                if modifiers.ctrl && !modifiers.shift && !modifiers.mac_cmd {
                    encode_key(Key::C, *modifiers, mode).map(InputAction::Write)
                } else {
                    Some(InputAction::Copy)
                }
            }
            InputEvent::Cut(modifiers) => {
                if mirrored_clipboard.take() == Some(Key::X) {
                    continue;
                }
                if modifiers.ctrl && !modifiers.mac_cmd {
                    encode_key(Key::X, *modifiers, mode).map(InputAction::Write)
                } else {
                    None
                }
            }
            InputEvent::Paste { text, modifiers } => {
                if mirrored_clipboard.take() == Some(Key::V) {
                    continue;
                }
                if modifiers.ctrl && !modifiers.shift && !modifiers.mac_cmd {
                    encode_key(Key::V, *modifiers, mode).map(InputAction::Write)
                } else {
                    Some(InputAction::Paste(text.clone()))
                }
            }
            InputEvent::Focus(focused) => Some(InputAction::Focus(*focused)),
        };
        if let Some(action) = action {
            routed.push(RoutedInput { target, action });
        }
    }
    routed
}

/// A logical key; physical location may be supplied separately by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Escape,
    Tab,
    Backspace,
    Enter,
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Copy,
    Cut,
    Paste,
    Colon,
    Comma,
    Backslash,
    Slash,
    Pipe,
    Questionmark,
    Exclamationmark,
    OpenBracket,
    CloseBracket,
    OpenCurlyBracket,
    CloseCurlyBracket,
    Backtick,
    Minus,
    Period,
    Plus,
    Equals,
    Semicolon,
    Quote,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
    BrowserBack,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    SuperLeft,
    SuperRight,
    IntlBackslash,
}

/// Modifiers at the time of an event. `mac_cmd` represents the Super/Command bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub mac_cmd: bool,
    pub command: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        alt: false,
        ctrl: false,
        shift: false,
        mac_cmd: false,
        command: false,
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        command: true,
        ..Self::NONE
    };
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
}

/// Encode a press. Returns `None` for printable keys owned by a Text event.
pub fn encode_key(key: Key, modifiers: Modifiers, mode: TermMode) -> Option<Vec<u8>> {
    encode_key_event(key, modifiers, mode, true, false)
}

/// Encode a press/repeat/release. Legacy modes never report releases.
pub fn encode_key_event(
    key: Key,
    modifiers: Modifiers,
    mode: TermMode,
    pressed: bool,
    repeat: bool,
) -> Option<Vec<u8>> {
    encode_key_event_with_text(key, modifiers, mode, pressed, repeat, None)
}

/// Attach actual layout-produced text to a Kitty key event when requested.
/// `modifiers` must reflect the state after the current modifier-key event.
pub fn encode_key_event_with_text(
    key: Key,
    modifiers: Modifiers,
    mode: TermMode,
    pressed: bool,
    repeat: bool,
    text: Option<&str>,
) -> Option<Vec<u8>> {
    let all = mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC);
    let events = mode.contains(TermMode::REPORT_EVENT_TYPES);
    if !pressed && !events {
        return None;
    }

    let printable = printable_key(key);
    let modifier_key = modifier_key_code(key);
    if modifier_key.is_some() && !all {
        return None;
    }
    let basic = matches!(key, Key::Enter | Key::Tab | Key::Backspace);
    if basic && !all {
        return pressed.then(|| legacy_basic(key, modifiers, mode));
    }

    let nontext_modifiers = modifiers.ctrl || modifiers.alt || modifiers.mac_cmd;
    let disambiguate = mode.contains(TermMode::DISAMBIGUATE_ESC_CODES);
    let enhanced = all
        || (disambiguate && (printable.is_none() || nontext_modifiers))
        || (events && (printable.is_none() || nontext_modifiers));

    if enhanced {
        let code = if let Some(code) = modifier_key {
            Code::Unicode(code)
        } else if let Some(character) = printable {
            Code::Unicode(character as u32)
        } else {
            functional_code(key)?
        };
        let event = if !pressed {
            3
        } else if repeat {
            2
        } else {
            1
        };
        let mut parameter = modifier_parameter(modifiers);
        // A physical Super press carries its own bit even when the adapter
        // cannot provide aggregate Super state on this platform.
        if pressed && matches!(key, Key::SuperLeft | Key::SuperRight) {
            parameter = 1 + ((parameter - 1) | 8);
        }
        let actual_text = if pressed {
            text.filter(|value| !value.is_empty())
                .map(str::to_owned)
                .or_else(|| {
                    printable
                        .filter(|_| !modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd)
                        .map(|character| shifted_character(character, modifiers.shift).to_string())
                })
        } else {
            None
        };
        let alternate = if mode.contains(TermMode::REPORT_ALTERNATE_KEYS) && modifiers.shift {
            printable.and_then(|character| {
                let shifted = actual_text
                    .as_deref()
                    .and_then(single_character)
                    .unwrap_or_else(|| shifted_character(character, true));
                (shifted != character).then_some(shifted as u32)
            })
        } else {
            None
        };
        return Some(kitty_sequence(
            code,
            parameter,
            events.then_some(event),
            alternate,
            if all && mode.contains(TermMode::REPORT_ASSOCIATED_TEXT) {
                actual_text.as_deref()
            } else {
                None
            },
        ));
    }

    if !pressed {
        return None;
    }
    if let Some(character) = printable {
        if modifiers.ctrl && !modifiers.mac_cmd {
            let character = shifted_character(character, modifiers.shift);
            let mut bytes = Vec::with_capacity(5);
            if modifiers.alt {
                bytes.push(0x1b);
            }
            if let Some(control) = control_character(character) {
                bytes.push(control);
            } else {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            return Some(bytes);
        }
        return None;
    }
    legacy_functional(key, modifiers, mode)
}

/// Encode text that was not already handled by an encoded printable Key event.
/// In legacy mode Ctrl text is ignored; Ctrl+Alt text is preserved as AltGr.
pub fn encode_text(text: &str, mut modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }
    if modifiers.ctrl && modifiers.alt {
        modifiers.ctrl = false;
        modifiers.alt = false;
        modifiers.command = false;
    } else if modifiers.ctrl || modifiers.mac_cmd {
        return Vec::new();
    }
    if mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC) {
        if mode.contains(TermMode::REPORT_ASSOCIATED_TEXT) {
            // A composition/text-only event has no known physical key.
            return kitty_sequence(Code::Unicode(0), 1, None, None, Some(text));
        }
        let mut bytes = Vec::new();
        for character in text.chars().filter(|character| !character.is_control()) {
            bytes.extend(kitty_sequence(
                Code::Unicode(character as u32),
                modifier_parameter(modifiers),
                None,
                None,
                None,
            ));
        }
        return bytes;
    }
    if modifiers.alt && mode.contains(TermMode::DISAMBIGUATE_ESC_CODES) {
        let mut bytes = Vec::new();
        for character in text.chars().filter(|character| !character.is_control()) {
            let code = character.to_lowercase().next().unwrap_or(character) as u32;
            bytes.extend(kitty_sequence(
                Code::Unicode(code),
                modifier_parameter(modifiers),
                None,
                None,
                None,
            ));
        }
        return bytes;
    }
    let mut bytes = Vec::with_capacity(text.len() + usize::from(modifiers.alt));
    if modifiers.alt {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

#[derive(Clone, Copy)]
enum Code {
    Unicode(u32),
    Csi(u16, char),
}

fn modifier_parameter(modifiers: Modifiers) -> u8 {
    1 + u8::from(modifiers.shift)
        + u8::from(modifiers.alt) * 2
        + u8::from(modifiers.ctrl) * 4
        + u8::from(modifiers.mac_cmd) * 8
}

fn single_character(text: &str) -> Option<char> {
    let mut characters = text.chars();
    let first = characters.next()?;
    characters.next().is_none().then_some(first)
}

fn kitty_sequence(
    code: Code,
    modifiers: u8,
    event: Option<u8>,
    alternate: Option<u32>,
    text: Option<&str>,
) -> Vec<u8> {
    use std::fmt::Write;
    let (number, suffix) = match code {
        Code::Unicode(number) => (number, 'u'),
        Code::Csi(number, suffix) => (u32::from(number), suffix),
    };
    let mut sequence = format!("\x1b[{number}");
    if let Some(alternate) = alternate {
        let _ = write!(sequence, ":{alternate}");
    }
    let _ = write!(sequence, ";{modifiers}");
    if let Some(event) = event.filter(|event| *event != 1) {
        let _ = write!(sequence, ":{event}");
    }
    let codepoints: Vec<_> = text
        .into_iter()
        .flat_map(str::chars)
        .filter(|character| !character.is_control())
        .map(|character| character as u32)
        .collect();
    if !codepoints.is_empty() {
        sequence.push(';');
        for (index, codepoint) in codepoints.into_iter().enumerate() {
            if index != 0 {
                sequence.push(':');
            }
            let _ = write!(sequence, "{codepoint}");
        }
    }
    sequence.push(suffix);
    sequence.into_bytes()
}

fn printable_key(key: Key) -> Option<char> {
    use Key::*;
    Some(match key {
        A => 'a',
        B => 'b',
        C => 'c',
        D => 'd',
        E => 'e',
        F => 'f',
        G => 'g',
        H => 'h',
        I => 'i',
        J => 'j',
        K => 'k',
        L => 'l',
        M => 'm',
        N => 'n',
        O => 'o',
        P => 'p',
        Q => 'q',
        R => 'r',
        S => 's',
        T => 't',
        U => 'u',
        V => 'v',
        W => 'w',
        X => 'x',
        Y => 'y',
        Z => 'z',
        Num0 => '0',
        Num1 => '1',
        Num2 => '2',
        Num3 => '3',
        Num4 => '4',
        Num5 => '5',
        Num6 => '6',
        Num7 => '7',
        Num8 => '8',
        Num9 => '9',
        Space => ' ',
        Colon => ':',
        Comma => ',',
        Backslash => '\\',
        Slash => '/',
        Pipe => '|',
        Questionmark => '?',
        Exclamationmark => '!',
        OpenBracket => '[',
        CloseBracket => ']',
        OpenCurlyBracket => '{',
        CloseCurlyBracket => '}',
        Backtick => '`',
        Minus => '-',
        Period => '.',
        Plus => '+',
        Equals => '=',
        Semicolon => ';',
        Quote => '\'',
        _ => return None,
    })
}

fn shifted_character(character: char, shift: bool) -> char {
    if shift && character.is_ascii_lowercase() {
        character.to_ascii_uppercase()
    } else if shift {
        match character {
            '0' => ')',
            '1' => '!',
            '2' => '@',
            '3' => '#',
            '4' => '$',
            '5' => '%',
            '6' => '^',
            '7' => '&',
            '8' => '*',
            '9' => '(',
            '[' => '{',
            ']' => '}',
            '\\' => '|',
            '/' => '?',
            '`' => '~',
            '-' => '_',
            '=' => '+',
            ';' => ':',
            '\'' => '"',
            ',' => '<',
            '.' => '>',
            _ => character,
        }
    } else {
        character
    }
}

fn control_character(character: char) -> Option<u8> {
    Some(match character {
        'a'..='z' => character as u8 - b'a' + 1,
        'A'..='Z' => character as u8 - b'A' + 1,
        ' ' | '@' | '`' | '2' => 0,
        '[' | '{' | '3' => 27,
        '\\' | '|' | '4' => 28,
        ']' | '}' | '5' => 29,
        '^' | '~' | '6' => 30,
        '_' | '-' | '/' | '7' => 31,
        '?' | '8' => 127,
        _ => return None,
    })
}

fn modifier_key_code(key: Key) -> Option<u32> {
    Some(match key {
        Key::ShiftLeft => 57441,
        Key::ControlLeft => 57442,
        Key::AltLeft => 57443,
        Key::SuperLeft => 57444,
        Key::ShiftRight => 57447,
        Key::ControlRight => 57448,
        Key::AltRight => 57449,
        Key::SuperRight => 57450,
        _ => return None,
    })
}

fn functional_code(key: Key) -> Option<Code> {
    use Key::*;
    Some(match key {
        Escape => Code::Unicode(27),
        Enter => Code::Unicode(13),
        Tab => Code::Unicode(9),
        Backspace => Code::Unicode(127),
        ArrowUp => Code::Csi(1, 'A'),
        ArrowDown => Code::Csi(1, 'B'),
        ArrowRight => Code::Csi(1, 'C'),
        ArrowLeft => Code::Csi(1, 'D'),
        Home => Code::Csi(1, 'H'),
        End => Code::Csi(1, 'F'),
        Insert => Code::Csi(2, '~'),
        Delete => Code::Csi(3, '~'),
        PageUp => Code::Csi(5, '~'),
        PageDown => Code::Csi(6, '~'),
        F1 => Code::Csi(1, 'P'),
        F2 => Code::Csi(1, 'Q'),
        F3 => Code::Csi(13, '~'),
        F4 => Code::Csi(1, 'S'),
        F5 => Code::Csi(15, '~'),
        F6 => Code::Csi(17, '~'),
        F7 => Code::Csi(18, '~'),
        F8 => Code::Csi(19, '~'),
        F9 => Code::Csi(20, '~'),
        F10 => Code::Csi(21, '~'),
        F11 => Code::Csi(23, '~'),
        F12 => Code::Csi(24, '~'),
        F13 => Code::Unicode(57376),
        F14 => Code::Unicode(57377),
        F15 => Code::Unicode(57378),
        F16 => Code::Unicode(57379),
        F17 => Code::Unicode(57380),
        F18 => Code::Unicode(57381),
        F19 => Code::Unicode(57382),
        F20 => Code::Unicode(57383),
        F21 => Code::Unicode(57384),
        F22 => Code::Unicode(57385),
        F23 => Code::Unicode(57386),
        F24 => Code::Unicode(57387),
        F25 => Code::Unicode(57388),
        F26 => Code::Unicode(57389),
        F27 => Code::Unicode(57390),
        F28 => Code::Unicode(57391),
        F29 => Code::Unicode(57392),
        F30 => Code::Unicode(57393),
        F31 => Code::Unicode(57394),
        F32 => Code::Unicode(57395),
        F33 => Code::Unicode(57396),
        F34 => Code::Unicode(57397),
        F35 => Code::Unicode(57398),
        _ => return None,
    })
}

fn legacy_basic(key: Key, modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    let mut bytes = if modifiers.alt {
        vec![0x1b]
    } else {
        Vec::new()
    };
    bytes.extend_from_slice(match key {
        Key::Enter if mode.contains(TermMode::LINE_FEED_NEW_LINE) => b"\r\n",
        Key::Enter => b"\r",
        Key::Backspace if modifiers.ctrl => b"\x08",
        Key::Backspace => b"\x7f",
        Key::Tab if modifiers.shift => b"\x1b[Z",
        Key::Tab => b"\t",
        Key::Escape => b"\x1b",
        _ => b"",
    });
    bytes
}

fn legacy_functional(key: Key, modifiers: Modifiers, mode: TermMode) -> Option<Vec<u8>> {
    if key == Key::Escape {
        return Some(legacy_basic(key, modifiers, mode));
    }
    let modifier = modifier_parameter(modifiers);
    let code = functional_code(key)?;
    if matches!(key, Key::F1 | Key::F2 | Key::F3 | Key::F4) {
        let suffix = match key {
            Key::F1 => 'P',
            Key::F2 => 'Q',
            Key::F3 => 'R',
            _ => 'S',
        };
        return Some(if modifier == 1 {
            format!("\x1bO{suffix}").into_bytes()
        } else {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        });
    }
    Some(match code {
        Code::Csi(1, suffix) if modifier == 1 => {
            if mode.contains(TermMode::APP_CURSOR) {
                format!("\x1bO{suffix}").into_bytes()
            } else {
                format!("\x1b[{suffix}").into_bytes()
            }
        }
        Code::Csi(number, suffix) if modifier == 1 => format!("\x1b[{number}{suffix}").into_bytes(),
        Code::Csi(number, suffix) => format!("\x1b[{number};{modifier}{suffix}").into_bytes(),
        Code::Unicode(number) => kitty_sequence(Code::Unicode(number), modifier, None, None, None),
    })
}

/// Encode a mouse event at zero-based terminal cell coordinates.
/// Codes: left 0, middle 1, right 2, wheel up/down 64/65; motion adds 32.
/// Shift is left to the caller's selection override policy, not suppressed here.
pub fn mouse(
    button: u8,
    column: u16,
    row: u16,
    pressed: bool,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    let motion = button & 32 != 0;
    let wheel = button & 64 != 0;
    if wheel && !pressed {
        return None;
    }
    if motion
        && !mode.contains(TermMode::MOUSE_MOTION)
        && !(mode.contains(TermMode::MOUSE_DRAG) && button & 3 != 3)
    {
        return None;
    }
    let modifier =
        u8::from(modifiers.shift) * 4 + u8::from(modifiers.alt) * 8 + u8::from(modifiers.ctrl) * 16;
    let code = u16::from(button | modifier);
    let column = u32::from(column) + 1;
    let row = u32::from(row) + 1;
    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if pressed { 'M' } else { 'm' };
        return Some(format!("\x1b[<{code};{column};{row}{suffix}").into_bytes());
    }
    let code = if pressed {
        code
    } else {
        3 + u16::from(modifier)
    } + 32;
    if mode.contains(TermMode::UTF8_MOUSE) {
        if column > 2015 || row > 2015 {
            return None;
        }
        let mut bytes = b"\x1b[M".to_vec();
        for value in [u32::from(code), column + 32, row + 32] {
            let character = char::from_u32(value)?;
            let mut buffer = [0; 4];
            bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
        }
        Some(bytes)
    } else {
        if column > 223 || row > 223 || code > 255 {
            return None;
        }
        Some(vec![
            0x1b,
            b'[',
            b'M',
            code as u8,
            (column + 32) as u8,
            (row + 32) as u8,
        ])
    }
}

/// Motion with a held button (`0..=2`) or `None` when no button is down.
/// The caller should deduplicate successive events in the same cell.
pub fn encode_mouse_motion(
    button: Option<u8>,
    column: u16,
    row: u16,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    mouse(32 + button.unwrap_or(3), column, row, true, modifiers, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: Modifiers) -> InputEvent {
        InputEvent::Key {
            key,
            physical_key: Some(key),
            modifiers,
            pressed: true,
            repeat: false,
        }
    }

    fn bytes(events: &[InputEvent], mode: TermMode) -> Vec<u8> {
        route_events(RoutingContext::TerminalPane(42), events, mode)
            .into_iter()
            .flat_map(|routed| match routed.action {
                InputAction::Write(bytes) => bytes,
                _ => Vec::new(),
            })
            .collect()
    }

    #[test]
    fn recorded_kitty_key_text_repeat_release_is_delivered_once() {
        let modifier = Modifiers::NONE;
        let events = [
            press(Key::E, modifier),
            InputEvent::Text {
                text: "é".into(),
                modifiers: modifier,
            },
            InputEvent::Key {
                key: Key::E,
                physical_key: Some(Key::E),
                modifiers: modifier,
                pressed: true,
                repeat: true,
            },
            InputEvent::Text {
                text: "é".into(),
                modifiers: modifier,
            },
            InputEvent::Key {
                key: Key::E,
                physical_key: Some(Key::E),
                modifiers: modifier,
                pressed: false,
                repeat: false,
            },
        ];
        let mode = TermMode::REPORT_ALL_KEYS_AS_ESC
            | TermMode::REPORT_EVENT_TYPES
            | TermMode::REPORT_ASSOCIATED_TEXT;
        assert_eq!(
            bytes(&events, mode),
            b"\x1b[101;1;233u\x1b[101;1:2;233u\x1b[101;1:3u"
        );
    }

    #[test]
    fn recorded_altgr_keeps_layout_text_and_drops_control_key() {
        let modifier = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            bytes(
                &[
                    press(Key::E, modifier),
                    InputEvent::Text {
                        text: "€".into(),
                        modifiers: modifier
                    }
                ],
                TermMode::empty()
            ),
            "€".as_bytes()
        );
    }

    #[test]
    fn recorded_composition_commits_once_and_clears_preedit() {
        let events = [
            InputEvent::Preedit("に".into()),
            InputEvent::Commit {
                text: "日本".into(),
                modifiers: Modifiers::NONE,
            },
            InputEvent::Text {
                text: "日本".into(),
                modifiers: Modifiers::NONE,
            },
        ];
        assert_eq!(bytes(&events, TermMode::empty()), "日本".as_bytes());
        assert_eq!(
            route_events(RoutingContext::TerminalPane(42), &events, TermMode::empty())[1].action,
            InputAction::Preedit(String::new())
        );
    }

    #[test]
    fn nonterminal_owners_never_emit_terminal_effects() {
        let events = [
            press(Key::C, Modifiers::CTRL),
            InputEvent::Text {
                text: "query".into(),
                modifiers: Modifiers::NONE,
            },
            InputEvent::Paste {
                text: "value".into(),
                modifiers: Modifiers::NONE,
            },
        ];
        for owner in [
            RoutingContext::Overlay,
            RoutingContext::TextField,
            RoutingContext::TerminalSearch,
        ] {
            assert!(route_events(owner, &events, TermMode::empty()).is_empty());
        }
    }

    #[test]
    fn legacy_clipboard_key_and_framework_mirror_deliver_one_control_byte() {
        for (key, mirrored, expected) in [
            (Key::C, InputEvent::Copy(Modifiers::CTRL), 3),
            (
                Key::V,
                InputEvent::Paste {
                    text: "clipboard".into(),
                    modifiers: Modifiers::CTRL,
                },
                22,
            ),
            (Key::X, InputEvent::Cut(Modifiers::CTRL), 24),
        ] {
            assert_eq!(
                bytes(&[press(key, Modifiers::CTRL), mirrored], TermMode::empty()),
                [expected]
            );
        }
    }

    #[test]
    fn routed_effects_keep_original_target_and_shifted_paste_ownership() {
        let modifier = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            route_events(
                RoutingContext::TerminalPane(91),
                &[InputEvent::Paste {
                    text: "paste".into(),
                    modifiers: modifier
                }],
                TermMode::empty()
            ),
            vec![RoutedInput {
                target: 91,
                action: InputAction::Paste("paste".into())
            }]
        );
    }

    #[test]
    fn pointer_batches_have_one_explicit_owner_and_keep_order() {
        let position = PointerPosition { x: 16.0, y: 48.0 };
        let events = [
            PointerEvent::Button {
                position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            PointerEvent::Moved {
                position: PointerPosition { x: 24.0, y: 48.0 },
                modifiers: Modifiers::NONE,
            },
            PointerEvent::Button {
                position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ];
        for context in [
            RoutingContext::Overlay,
            RoutingContext::TextField,
            RoutingContext::TerminalSearch,
        ] {
            assert!(route_pointer_events(context, &events).is_empty());
        }
        let routed = route_pointer_events(RoutingContext::TerminalPane(73), &events);
        assert_eq!(
            routed.iter().map(|input| input.event).collect::<Vec<_>>(),
            events
        );
        assert!(routed.iter().all(|input| input.target == 73));
    }

    #[test]
    fn control_chords_preserve_shell_semantics() {
        for (key, byte) in [
            (Key::C, 3),
            (Key::W, 23),
            (Key::R, 18),
            (Key::Num2, 0),
            (Key::Num3, 27),
            (Key::Num4, 28),
            (Key::Num5, 29),
            (Key::Num6, 30),
            (Key::Num7, 31),
            (Key::Num8, 127),
            (Key::Backslash, 28),
            (Key::Slash, 31),
        ] {
            assert_eq!(
                encode_key(key, Modifiers::CTRL, TermMode::empty()),
                Some(vec![byte])
            );
        }
        let alt_ctrl = Modifiers {
            alt: true,
            ctrl: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Key::C, alt_ctrl, TermMode::empty()),
            Some(vec![27, 3])
        );
        assert!(encode_text("c", Modifiers::CTRL, TermMode::empty()).is_empty());
    }

    #[test]
    fn cursor_and_function_sequences_follow_xterm_modes() {
        assert_eq!(
            encode_key(Key::ArrowUp, Modifiers::NONE, TermMode::APP_CURSOR).unwrap(),
            b"\x1bOA"
        );
        assert_eq!(
            encode_key(Key::Home, Modifiers::NONE, TermMode::APP_CURSOR).unwrap(),
            b"\x1bOH"
        );
        assert_eq!(
            encode_key(Key::ArrowLeft, Modifiers::CTRL, TermMode::APP_CURSOR).unwrap(),
            b"\x1b[1;5D"
        );
        assert_eq!(
            encode_key(Key::F3, Modifiers::NONE, TermMode::empty()).unwrap(),
            b"\x1bOR"
        );
        assert_eq!(
            encode_key(Key::F12, Modifiers::ALT, TermMode::empty()).unwrap(),
            b"\x1b[24;3~"
        );
        assert_eq!(
            encode_key(Key::Tab, Modifiers::SHIFT, TermMode::empty()).unwrap(),
            b"\x1b[Z"
        );
        assert_eq!(
            encode_key(Key::Enter, Modifiers::NONE, TermMode::LINE_FEED_NEW_LINE).unwrap(),
            b"\r\n"
        );
    }

    #[test]
    fn text_is_unicode_safe_and_owned_once() {
        assert_eq!(encode_key(Key::A, Modifiers::NONE, TermMode::empty()), None);
        assert_eq!(
            encode_text("你好 café 🦀", Modifiers::NONE, TermMode::empty()),
            "你好 café 🦀".as_bytes()
        );
        assert_eq!(
            encode_text("é", Modifiers::ALT, TermMode::empty()),
            "\x1bé".as_bytes()
        );
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode_text("€", altgr, TermMode::empty()), "€".as_bytes());
    }

    #[test]
    fn kitty_disambiguates_without_breaking_reset_keys() {
        let mode = TermMode::DISAMBIGUATE_ESC_CODES;
        assert_eq!(
            encode_key(Key::C, Modifiers::CTRL, mode).unwrap(),
            b"\x1b[99;5u"
        );
        assert_eq!(
            encode_key(Key::Escape, Modifiers::NONE, mode).unwrap(),
            b"\x1b[27;1u"
        );
        assert_eq!(
            encode_key(Key::Enter, Modifiers::NONE, mode).unwrap(),
            b"\r"
        );
        assert_eq!(encode_key(Key::Tab, Modifiers::NONE, mode).unwrap(), b"\t");
        assert_eq!(
            encode_key(Key::Backspace, Modifiers::NONE, mode).unwrap(),
            b"\x7f"
        );
        assert_eq!(encode_key(Key::A, Modifiers::SHIFT, mode), None);
        assert_eq!(
            encode_key(Key::F3, Modifiers::NONE, mode).unwrap(),
            b"\x1b[13;1~"
        );
    }

    #[test]
    fn kitty_repeats_and_releases_require_requested_flags() {
        let mode = TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        assert_eq!(
            encode_key_event(Key::C, Modifiers::CTRL, mode, true, true).unwrap(),
            b"\x1b[99;5:2u"
        );
        assert_eq!(
            encode_key_event(Key::C, Modifiers::CTRL, mode, false, false).unwrap(),
            b"\x1b[99;5:3u"
        );
        assert!(
            encode_key_event(Key::C, Modifiers::CTRL, TermMode::empty(), false, false).is_none()
        );
        assert!(encode_key_event(Key::Enter, Modifiers::NONE, mode, false, false).is_none());
        assert!(encode_key_event(Key::A, Modifiers::NONE, mode, false, false).is_none());
    }

    #[test]
    fn kitty_report_all_and_associated_unicode_text() {
        let mode = TermMode::REPORT_ALL_KEYS_AS_ESC
            | TermMode::REPORT_EVENT_TYPES
            | TermMode::REPORT_ALTERNATE_KEYS
            | TermMode::REPORT_ASSOCIATED_TEXT;
        assert_eq!(
            encode_key(Key::A, Modifiers::SHIFT, mode).unwrap(),
            b"\x1b[97:65;2;65u"
        );
        assert_eq!(
            encode_key_event(Key::A, Modifiers::SHIFT, mode, false, false).unwrap(),
            b"\x1b[97:65;2:3u"
        );
        assert_eq!(
            encode_key(Key::Enter, Modifiers::NONE, mode).unwrap(),
            b"\x1b[13;1u"
        );
        assert_eq!(
            encode_key_event_with_text(Key::E, Modifiers::NONE, mode, true, false, Some("é"))
                .unwrap(),
            b"\x1b[101;1;233u"
        );
        assert_eq!(
            encode_text("你好", Modifiers::NONE, mode),
            b"\x1b[0;1;20320:22909u"
        );
        let plain = TermMode::REPORT_ALL_KEYS_AS_ESC;
        assert_eq!(encode_text("é", Modifiers::NONE, plain), b"\x1b[233;1u");
        assert_eq!(
            encode_key(Key::F35, Modifiers::NONE, plain).unwrap(),
            b"\x1b[57398;1u"
        );
        assert_eq!(
            encode_key(Key::ShiftLeft, Modifiers::SHIFT, plain).unwrap(),
            b"\x1b[57441;2u"
        );
    }

    #[test]
    fn sgr_mouse_encodes_buttons_modifiers_and_release() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert_eq!(
            mouse(0, 0, 0, true, Modifiers::NONE, mode).unwrap(),
            b"\x1b[<0;1;1M"
        );
        assert_eq!(
            mouse(2, 14, 8, false, Modifiers::CTRL, mode).unwrap(),
            b"\x1b[<18;15;9m"
        );
        assert_eq!(
            mouse(64, 300, 500, true, Modifiers::ALT, mode).unwrap(),
            b"\x1b[<72;301;501M"
        );
        assert!(mouse(64, 0, 0, false, Modifiers::NONE, mode).is_none());
        assert!(mouse(0, 0, 0, true, Modifiers::NONE, TermMode::empty()).is_none());
    }

    #[test]
    fn legacy_mouse_limits_coordinates_and_utf8_extends_them() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(
            mouse(0, 0, 0, true, Modifiers::NONE, mode).unwrap(),
            b"\x1b[M !!"
        );
        assert_eq!(
            mouse(2, 1, 2, false, Modifiers::NONE, mode).unwrap(),
            b"\x1b[M#\"#"
        );
        assert_eq!(
            mouse(0, 222, 222, true, Modifiers::NONE, mode).unwrap(),
            vec![27, 91, 77, 32, 255, 255]
        );
        assert!(mouse(0, 223, 0, true, Modifiers::NONE, mode).is_none());
        let utf8 = mode | TermMode::UTF8_MOUSE;
        assert_eq!(
            mouse(0, 223, 0, true, Modifiers::NONE, utf8).unwrap(),
            "\x1b[M Ā!".as_bytes()
        );
        assert!(mouse(0, 2015, 0, true, Modifiers::NONE, utf8).is_none());
    }

    #[test]
    fn mouse_motion_respects_drag_and_any_motion_modes() {
        let sgr = TermMode::SGR_MOUSE;
        assert!(
            encode_mouse_motion(
                Some(0),
                0,
                0,
                Modifiers::NONE,
                sgr | TermMode::MOUSE_REPORT_CLICK
            )
            .is_none()
        );
        assert_eq!(
            encode_mouse_motion(Some(0), 0, 0, Modifiers::NONE, sgr | TermMode::MOUSE_DRAG)
                .unwrap(),
            b"\x1b[<32;1;1M"
        );
        assert!(
            encode_mouse_motion(None, 0, 0, Modifiers::NONE, sgr | TermMode::MOUSE_DRAG).is_none()
        );
        assert_eq!(
            encode_mouse_motion(None, 0, 0, Modifiers::NONE, sgr | TermMode::MOUSE_MOTION).unwrap(),
            b"\x1b[<35;1;1M"
        );
    }
}
