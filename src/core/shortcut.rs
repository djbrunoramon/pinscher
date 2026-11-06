//! Validated global keyboard shortcuts and their user-facing display form.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    modifiers: u8,
    key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Letter(char),
    Digit(char),
    Function(u8),
    Space,
}

const CTRL: u8 = 1;
const ALT: u8 = 2;
const SHIFT: u8 = 4;
const CMD: u8 = 8;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShortcutError {
    #[error("shortcut is invalid")]
    Invalid,
}

/// Parses a global shortcut from the config format.
///
/// # Errors
///
/// Returns [`ShortcutError::Invalid`] for unsupported, incomplete, duplicated, or overlong
/// shortcut values.
pub fn parse(value: &str) -> Result<Shortcut, ShortcutError> {
    if value.is_empty() || value.chars().count() > 64 {
        return Err(ShortcutError::Invalid);
    }

    let mut modifiers = 0;
    let mut key = None;
    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return Err(ShortcutError::Invalid);
        }
        let lower = token.to_ascii_lowercase();
        let modifier = match lower.as_str() {
            "ctrl" | "control" => Some(CTRL),
            "alt" | "option" => Some(ALT),
            "shift" => Some(SHIFT),
            "cmd" | "command" | "super" => Some(CMD),
            _ => None,
        };
        if let Some(modifier) = modifier {
            if modifiers & modifier != 0 {
                return Err(ShortcutError::Invalid);
            }
            modifiers |= modifier;
        } else if key.replace(parse_key(&lower)?).is_some() {
            return Err(ShortcutError::Invalid);
        }
    }

    if modifiers & (CTRL | ALT | CMD) == 0 {
        return Err(ShortcutError::Invalid);
    }
    Ok(Shortcut {
        modifiers,
        key: key.ok_or(ShortcutError::Invalid)?,
    })
}

/// Modifier keys held during a key press, as reported by the platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

/// Builds a shortcut from a recorded key press (macOS virtual key code).
///
/// # Errors
///
/// Returns [`ShortcutError::Invalid`] for unsupported keys or when no ⌃, ⌥ or ⌘ is held.
pub fn from_key_press(modifiers: Modifiers, key_code: u16) -> Result<Shortcut, ShortcutError> {
    let key = key_for_code(key_code).ok_or(ShortcutError::Invalid)?;
    let mut bits = 0;
    for (held, bit) in [
        (modifiers.ctrl, CTRL),
        (modifiers.alt, ALT),
        (modifiers.shift, SHIFT),
        (modifiers.cmd, CMD),
    ] {
        if held {
            bits |= bit;
        }
    }
    if bits & (CTRL | ALT | CMD) == 0 {
        return Err(ShortcutError::Invalid);
    }
    Ok(Shortcut {
        modifiers: bits,
        key,
    })
}

/// ANSI virtual key codes (`kVK_*` in Carbon's `Events.h`) for the supported keys.
fn key_for_code(code: u16) -> Option<Key> {
    const LETTERS: [(u16, char); 26] = [
        (0, 'A'),
        (11, 'B'),
        (8, 'C'),
        (2, 'D'),
        (14, 'E'),
        (3, 'F'),
        (5, 'G'),
        (4, 'H'),
        (34, 'I'),
        (38, 'J'),
        (40, 'K'),
        (37, 'L'),
        (46, 'M'),
        (45, 'N'),
        (31, 'O'),
        (35, 'P'),
        (12, 'Q'),
        (15, 'R'),
        (1, 'S'),
        (17, 'T'),
        (32, 'U'),
        (9, 'V'),
        (13, 'W'),
        (7, 'X'),
        (16, 'Y'),
        (6, 'Z'),
    ];
    const DIGITS: [(u16, char); 10] = [
        (29, '0'),
        (18, '1'),
        (19, '2'),
        (20, '3'),
        (21, '4'),
        (23, '5'),
        (22, '6'),
        (26, '7'),
        (28, '8'),
        (25, '9'),
    ];
    const FUNCTIONS: [(u16, u8); 12] = [
        (122, 1),
        (120, 2),
        (99, 3),
        (118, 4),
        (96, 5),
        (97, 6),
        (98, 7),
        (100, 8),
        (101, 9),
        (109, 10),
        (103, 11),
        (111, 12),
    ];
    const SPACE: u16 = 49;
    if code == SPACE {
        return Some(Key::Space);
    }
    LETTERS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, l)| Key::Letter(*l))
        .or_else(|| {
            DIGITS
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, d)| Key::Digit(*d))
        })
        .or_else(|| {
            FUNCTIONS
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, n)| Key::Function(*n))
        })
}

impl Shortcut {
    /// Converts the shortcut to the normalized `global-hotkey` config format.
    #[must_use]
    pub fn to_config_string(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers & CTRL != 0 {
            parts.push("ctrl".to_owned());
        }
        if self.modifiers & ALT != 0 {
            parts.push("alt".to_owned());
        }
        if self.modifiers & SHIFT != 0 {
            parts.push("shift".to_owned());
        }
        if self.modifiers & CMD != 0 {
            parts.push("cmd".to_owned());
        }
        parts.push(self.key.config_name());
        parts.join("+")
    }

    /// Returns the compact macOS display form.
    #[must_use]
    pub fn display(&self) -> String {
        let mut display = String::new();
        if self.modifiers & CTRL != 0 {
            display.push('⌃');
        }
        if self.modifiers & ALT != 0 {
            display.push('⌥');
        }
        if self.modifiers & SHIFT != 0 {
            display.push('⇧');
        }
        if self.modifiers & CMD != 0 {
            display.push('⌘');
        }
        display.push_str(&self.key.display_name());
        display
    }
}

impl Key {
    fn config_name(self) -> String {
        match self {
            Self::Letter(letter) => format!("Key{letter}"),
            Self::Digit(digit) => format!("Digit{digit}"),
            Self::Function(number) => format!("F{number}"),
            Self::Space => "Space".to_owned(),
        }
    }

    fn display_name(self) -> String {
        match self {
            Self::Letter(letter) | Self::Digit(letter) => letter.to_string(),
            Self::Function(number) => format!("F{number}"),
            Self::Space => "Space".to_owned(),
        }
    }
}

fn parse_key(token: &str) -> Result<Key, ShortcutError> {
    if let Some(letter) = token.strip_prefix("key") {
        let mut chars = letter.chars();
        if let (Some(letter), None) = (chars.next(), chars.next())
            && letter.is_ascii_lowercase()
        {
            return Ok(Key::Letter(letter.to_ascii_uppercase()));
        }
    }
    if let Some(digit) = token.strip_prefix("digit") {
        let mut chars = digit.chars();
        if let (Some(digit), None) = (chars.next(), chars.next())
            && digit.is_ascii_digit()
        {
            return Ok(Key::Digit(digit));
        }
    }
    if let Some(number) = token.strip_prefix('f')
        && let Ok(number) = number.parse::<u8>()
        && (1..=12).contains(&number)
        && number.to_string() == token.strip_prefix('f').unwrap_or_default()
    {
        return Ok(Key::Function(number));
    }
    if token == "space" {
        return Ok(Key::Space);
    }
    Err(ShortcutError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_shortcut_and_displays_symbols() {
        let shortcut = parse("alt+cmd+KeyP").unwrap();
        assert_eq!(shortcut.to_config_string(), "alt+cmd+KeyP");
        assert_eq!(shortcut.display(), "⌥⌘P");
    }

    #[test]
    fn round_trips_normalized_config() {
        let shortcut = parse("COMMAND+option+shift+keya").unwrap();
        assert_eq!(parse(&shortcut.to_config_string()), Ok(shortcut));
    }

    #[test]
    fn accepts_supported_keys_and_modifier_aliases() {
        for value in [
            "control+KeyA",
            "option+Digit9",
            "super+F12",
            "ctrl+shift+Space",
        ] {
            assert!(parse(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn rejects_shift_only() {
        assert_eq!(parse("shift+KeyP"), Err(ShortcutError::Invalid));
    }

    #[test]
    fn rejects_unknown_token() {
        assert_eq!(parse("alt+banana+KeyP"), Err(ShortcutError::Invalid));
    }

    #[test]
    fn rejects_duplicate_modifier() {
        assert_eq!(parse("alt+option+KeyP"), Err(ShortcutError::Invalid));
    }

    #[test]
    fn rejects_two_keys() {
        assert_eq!(parse("alt+KeyP+KeyQ"), Err(ShortcutError::Invalid));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(parse(""), Err(ShortcutError::Invalid));
    }

    #[test]
    fn records_key_press() {
        let modifiers = Modifiers {
            alt: true,
            cmd: true,
            ..Modifiers::default()
        };
        assert_eq!(from_key_press(modifiers, 35), parse("alt+cmd+KeyP"));
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        assert_eq!(from_key_press(ctrl, 49), parse("ctrl+Space"));
        assert_eq!(from_key_press(ctrl, 111), parse("ctrl+F12"));
        assert_eq!(from_key_press(ctrl, 29), parse("ctrl+Digit0"));
    }

    #[test]
    fn rejects_recorded_press_without_command_modifier() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(from_key_press(shift, 35), Err(ShortcutError::Invalid));
        assert_eq!(
            from_key_press(Modifiers::default(), 35),
            Err(ShortcutError::Invalid)
        );
    }

    #[test]
    fn rejects_unsupported_key_code() {
        let cmd = Modifiers {
            cmd: true,
            ..Modifiers::default()
        };
        // 53 is Escape, 36 is Return.
        assert_eq!(from_key_press(cmd, 53), Err(ShortcutError::Invalid));
        assert_eq!(from_key_press(cmd, 36), Err(ShortcutError::Invalid));
    }

    #[test]
    fn every_letter_code_is_unique() {
        let cmd = Modifiers {
            cmd: true,
            ..Modifiers::default()
        };
        let mut seen = std::collections::HashSet::new();
        for code in 0..128 {
            if let Ok(shortcut) = from_key_press(cmd, code) {
                assert!(seen.insert(shortcut.to_config_string()), "{code}");
            }
        }
        assert_eq!(seen.len(), 26 + 10 + 12 + 1);
    }

    #[test]
    fn rejects_overlong() {
        assert_eq!(parse(&"a".repeat(65)), Err(ShortcutError::Invalid));
    }
}
