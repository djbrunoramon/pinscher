//! Well-known file locations.

use std::path::PathBuf;

/// `~/Library/Application Support/Pinscher/config.toml`, or `None` when there is no home directory.
#[must_use]
pub fn config_file() -> Option<PathBuf> {
    std::env::home_dir().map(|home| {
        home.join("Library")
            .join("Application Support")
            .join("Pinscher")
            .join("config.toml")
    })
}
