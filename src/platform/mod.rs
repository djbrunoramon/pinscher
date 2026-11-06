//! Adapters over macOS APIs. Keep them thin: decisions belong in `core`.

pub mod capture;
pub mod events;
pub mod hotkey;
pub mod language;
pub mod logging;
pub mod login_item;
pub mod menu;
pub mod menu_extras;
pub mod paths;
pub mod permissions;
pub mod preferences;
pub mod status_bar;
pub mod timer;

pub mod layout_view;
pub mod secondary_bar;
