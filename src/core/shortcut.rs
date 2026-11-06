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
    fn rejects_overlong() {
        assert_eq!(parse(&"a".repeat(65)), Err(ShortcutError::Invalid));
    }
}
