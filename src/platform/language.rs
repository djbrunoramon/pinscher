//! Reads the user's preferred languages from the system.

use crate::core::l10n::Lang;
use objc2_foundation::NSLocale;

/// Interface language chosen from the system's preferred languages.
#[must_use]
pub fn current() -> Lang {
    let tags: Vec<String> = NSLocale::preferredLanguages()
        .iter()
        .map(|tag| tag.to_string())
        .collect();
    Lang::from_preferred(tags.iter().map(String::as_str))
}
