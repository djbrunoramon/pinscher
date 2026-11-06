//! User preferences stored as TOML in `~/Library/Application Support/Pinscher/config.toml`.
//!
//! Loading never fails: a missing file yields defaults, and a file that is too large or cannot
//! be parsed is moved aside to `config.toml.bak` before falling back to defaults. Values outside
//! their allowed range are clamped. Every such event is reported as a [`Warning`] so the caller
//! can log it; nothing here logs or touches `AppKit`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

/// Current schema version written to new files.
pub const SCHEMA_VERSION: u32 = 1;
/// Files larger than this are treated as corrupt (AM-01).
pub const MAX_FILE_BYTES: u64 = 64 * 1024;
/// Default global shortcut, in `global-hotkey` string format.
pub const DEFAULT_SHORTCUT: &str = "alt+cmd+KeyP";
/// Allowed auto-hide delay in seconds.
pub const AUTO_HIDE_DELAY_RANGE: RangeInclusive<u32> = 1..=60;
/// Default auto-hide delay in seconds.
pub const DEFAULT_AUTO_HIDE_DELAY: u32 = 5;
/// Maximum number of remembered menu bar items.
pub const MAX_ITEMS: usize = 256;
/// Maximum length (in chars) of any stored string.
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub shortcut: Shortcut,
    pub auto_hide: AutoHide,
    pub advanced: Advanced,
    pub items: Vec<SavedItem>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            shortcut: Shortcut::default(),
            auto_hide: AutoHide::default(),
            advanced: Advanced::default(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcut {
    /// Toggle shortcut, e.g. `alt+cmd+KeyP`. Empty disables it.
    pub toggle: String,
}

impl Default for Shortcut {
    fn default() -> Self {
        Self {
            toggle: DEFAULT_SHORTCUT.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoHide {
    pub enabled: bool,
    pub delay_secs: u32,
    /// Hide the icons right after Pinscher starts (and after every login), instead of waiting
    /// for the first click.
    pub hide_on_launch: bool,
}

impl Default for AutoHide {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_secs: DEFAULT_AUTO_HIDE_DELAY,
            hide_on_launch: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Advanced {
    pub secondary_bar: bool,
    pub always_hidden: bool,
}

/// Where the user wants a menu bar item to live.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    #[default]
    Visible,
    Hidden,
    AlwaysHidden,
}

/// A remembered menu bar item, identified by its owner's bundle id and the item title.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedItem {
    pub owner: String,
    pub title: String,
    pub section: Section,
}

/// Something worth logging that happened while loading. Carries no paths or values (AM-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// The file exists but could not be read.
    Unreadable(io::ErrorKind),
    /// The file was larger than [`MAX_FILE_BYTES`] and was moved aside.
    TooLarge,
    /// The file was not valid TOML for this schema and was moved aside.
    Corrupt,
    /// Moving a bad file aside failed; it was left in place.
    BackupFailed(io::ErrorKind),
    /// A value was out of range and was clamped; the payload is the key name.
    Clamped(&'static str),
}

/// Result of [`load`]: always a usable config, plus what went wrong on the way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    pub config: Config,
    pub warnings: Vec<Warning>,
}

/// Errors from [`save`]. The `Display` text never includes the path (AM-02).
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("could not write config: {0}")]
    Io(io::ErrorKind),
    #[error("could not serialize config")]
    Serialize,
    #[error("configuration exceeds the file size limit")]
    TooLarge,
}

impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

/// Loads the config at `path`. Never fails; see the module docs.
#[must_use]
pub fn load(path: &Path) -> Loaded {
    let mut warnings = Vec::new();
    let config = match read_capped(path) {
        Ok(None) => Config::default(),
        Ok(Some(bytes)) => parse(&bytes).unwrap_or_else(|| {
            warnings.push(Warning::Corrupt);
            back_up(path, &mut warnings);
            Config::default()
        }),
        Err(ReadError::TooLarge) => {
            warnings.push(Warning::TooLarge);
            back_up(path, &mut warnings);
            Config::default()
        }
        Err(ReadError::Io(kind)) => {
            warnings.push(Warning::Unreadable(kind));
            Config::default()
        }
    };
    let config = sanitize(config, &mut warnings);
    Loaded { config, warnings }
}

/// Writes `config` atomically (temp file + rename), creating the parent directory if needed.
///
/// # Errors
/// Returns [`SaveError`] when the directory or file cannot be written.
pub fn save(path: &Path, config: &Config) -> Result<(), SaveError> {
    let text = toml::to_string_pretty(config).map_err(|_| SaveError::Serialize)?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(SaveError::TooLarge);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = sibling(path, "tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Path used to keep a copy of a file that could not be loaded.
#[must_use]
pub fn backup_path(path: &Path) -> PathBuf {
    sibling(path, "bak")
}

enum ReadError {
    TooLarge,
    Io(io::ErrorKind),
}

/// Invalid UTF-8 is just another kind of corrupt file.
fn parse(bytes: &[u8]) -> Option<Config> {
    let text = std::str::from_utf8(bytes).ok()?;
    toml::from_str(text).ok()
}

fn read_capped(path: &Path) -> Result<Option<Vec<u8>>, ReadError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ReadError::Io(error.kind())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReadError::Io(error.kind()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ReadError::TooLarge);
    }
    Ok(Some(bytes))
}

fn back_up(path: &Path, warnings: &mut Vec<Warning>) {
    if let Err(error) = fs::rename(path, backup_path(path)) {
        warnings.push(Warning::BackupFailed(error.kind()));
    }
}

fn sanitize(mut config: Config, warnings: &mut Vec<Warning>) -> Config {
    let delay = config.auto_hide.delay_secs;
    let clamped = delay.clamp(*AUTO_HIDE_DELAY_RANGE.start(), *AUTO_HIDE_DELAY_RANGE.end());
    if clamped != delay {
        config.auto_hide.delay_secs = clamped;
        warnings.push(Warning::Clamped("auto_hide.delay_secs"));
    }
    if config.shortcut.toggle.chars().count() > MAX_TEXT_LEN {
        DEFAULT_SHORTCUT.clone_into(&mut config.shortcut.toggle);
        warnings.push(Warning::Clamped("shortcut.toggle"));
    }
    if config.items.len() > MAX_ITEMS {
        config.items.truncate(MAX_ITEMS);
        warnings.push(Warning::Clamped("items"));
    }
    let before = config.items.len();
    config.items.retain(|item| {
        !item.owner.is_empty()
            && item.owner.chars().count() <= MAX_TEXT_LEN
            && item.title.chars().count() <= MAX_TEXT_LEN
    });
    if config.items.len() != before {
        warnings.push(Warning::Clamped("items"));
    }
    config.version = SCHEMA_VERSION;
    config
}

fn sibling(path: &Path, extension: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(extension);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use tempfile::TempDir;

    fn config_in(dir: &TempDir) -> PathBuf {
        dir.path().join("Pinscher").join("config.toml")
    }

    fn write(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn missing_file_yields_defaults_without_warnings() {
        let dir = TempDir::new().unwrap();
        let loaded = load(&config_in(&dir));
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        let config = Config {
            shortcut: Shortcut {
                toggle: "ctrl+shift+KeyH".into(),
            },
            auto_hide: AutoHide {
                enabled: false,
                delay_secs: 12,
                hide_on_launch: false,
            },
            advanced: Advanced {
                secondary_bar: true,
                always_hidden: true,
            },
            items: vec![SavedItem {
                owner: "com.example.app".into(),
                title: "Example".into(),
                section: Section::AlwaysHidden,
            }],
            ..Config::default()
        };
        save(&path, &config).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.config, config);
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        save(&path, &Config::default()).unwrap();
        assert!(!sibling(&path, "tmp").exists());
    }

    #[test]
    fn missing_keys_get_defaults() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"[auto_hide]\nenabled = false\n");
        let loaded = load(&path);
        assert!(!loaded.config.auto_hide.enabled);
        assert_eq!(loaded.config.auto_hide.delay_secs, DEFAULT_AUTO_HIDE_DELAY);
        assert_eq!(loaded.config.shortcut.toggle, DEFAULT_SHORTCUT);
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn configs_saved_before_hide_on_launch_existed_hide_on_launch() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"[auto_hide]\nenabled = true\ndelay_secs = 7\n");
        let loaded = load(&path);
        assert!(loaded.config.auto_hide.hide_on_launch);
        assert_eq!(loaded.config.auto_hide.delay_secs, 7);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(
            &path,
            b"future_feature = true\n[auto_hide]\ndelay_secs = 9\n",
        );
        let loaded = load(&path);
        assert_eq!(loaded.config.auto_hide.delay_secs, 9);
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn corrupt_file_backs_up_and_uses_defaults() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"this is = = not toml [[[");
        let loaded = load(&path);
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.warnings, vec![Warning::Corrupt]);
        assert!(!path.exists());
        assert_eq!(
            fs::read(backup_path(&path)).unwrap(),
            b"this is = = not toml [[["
        );
    }

    #[test]
    fn wrong_types_count_as_corrupt() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"[auto_hide]\ndelay_secs = \"five\"\n");
        let loaded = load(&path);
        assert_eq!(loaded.warnings, vec![Warning::Corrupt]);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn invalid_utf8_counts_as_corrupt() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, &[0xff, 0xfe, 0x00, 0x41]);
        let loaded = load(&path);
        assert_eq!(loaded.warnings, vec![Warning::Corrupt]);
    }

    #[test]
    fn empty_file_yields_defaults() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"");
        let loaded = load(&path);
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn rejects_oversized_file() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        let mut big = b"# ".to_vec();
        big.resize(usize::try_from(MAX_FILE_BYTES).unwrap() + 10, b'a');
        write(&path, &big);
        let loaded = load(&path);
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.warnings, vec![Warning::TooLarge]);
        assert!(backup_path(&path).exists());
    }

    #[test]
    fn clamps_out_of_range_delay() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        write(&path, b"[auto_hide]\ndelay_secs = 0\n");
        assert_eq!(load(&path).config.auto_hide.delay_secs, 1);
        write(&path, b"[auto_hide]\ndelay_secs = 4000000000\n");
        let loaded = load(&path);
        assert_eq!(loaded.config.auto_hide.delay_secs, 60);
        assert_eq!(
            loaded.warnings,
            vec![Warning::Clamped("auto_hide.delay_secs")]
        );
    }

    #[test]
    fn drops_invalid_and_excess_items() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        let mut text = String::from("[[items]]\nowner = \"\"\ntitle = \"no owner\"\n");
        for i in 0..MAX_ITEMS + 5 {
            writeln!(text, "[[items]]\nowner = \"app.{i}\"\nsection = \"hidden\"").unwrap();
        }
        write(&path, text.as_bytes());
        let loaded = load(&path);
        assert!(loaded.config.items.len() <= MAX_ITEMS);
        assert!(
            loaded
                .config
                .items
                .iter()
                .all(|item| !item.owner.is_empty())
        );
        assert!(loaded.warnings.contains(&Warning::Clamped("items")));
    }

    #[test]
    fn overlong_shortcut_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        let path = config_in(&dir);
        let text = format!(
            "[shortcut]\ntoggle = \"{}\"\n",
            "a".repeat(MAX_TEXT_LEN + 1)
        );
        write(&path, text.as_bytes());
        let loaded = load(&path);
        assert_eq!(loaded.config.shortcut.toggle, DEFAULT_SHORTCUT);
        assert_eq!(loaded.warnings, vec![Warning::Clamped("shortcut.toggle")]);
    }

    #[test]
    fn error_display_has_no_path() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("secret-user-folder");
        fs::write(&blocker, b"file, not a directory").unwrap();
        let error = save(&blocker.join("config.toml"), &Config::default()).unwrap_err();
        let shown = error.to_string();
        assert!(!shown.contains("secret-user-folder"), "{shown}");
        assert!(!shown.contains('/'), "{shown}");
    }
    #[test]
    fn oversized_save_preserves_existing_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        save(&path, &Config::default()).unwrap();
        let before = fs::read(&path).unwrap();
        let mut config = Config::default();
        config.shortcut.toggle = "x".repeat(usize::try_from(MAX_FILE_BYTES).unwrap());
        assert!(matches!(save(&path, &config), Err(SaveError::TooLarge)));
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
