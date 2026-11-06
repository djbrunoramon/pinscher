//! Which system permissions the enabled features need, and what is missing.
//!
//! The basic show/hide mode needs none. Permissions are only asked for when the user turns on a
//! feature that needs them (REQ-31, AM-05).

use crate::core::config::Advanced;

/// A system permission Pinscher can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Read other apps' status item titles and capture their images.
    ScreenRecording,
    /// Post synthetic mouse events (clicking and moving status items).
    Accessibility,
}

/// What the system has granted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Granted {
    pub screen_recording: bool,
    pub accessibility: bool,
}

/// Permissions needed by the enabled advanced features, in the order to ask for them.
#[must_use]
pub fn required(advanced: &Advanced) -> Vec<Permission> {
    if advanced.secondary_bar {
        vec![Permission::ScreenRecording, Permission::Accessibility]
    } else {
        Vec::new()
    }
}

/// Permissions needed to rearrange items from Preferences.
#[must_use]
pub fn required_for_reordering() -> Vec<Permission> {
    vec![Permission::ScreenRecording, Permission::Accessibility]
}

/// The subset of `needed` that is not granted.
#[must_use]
pub fn missing(needed: &[Permission], granted: Granted) -> Vec<Permission> {
    needed
        .iter()
        .copied()
        .filter(|permission| match permission {
            Permission::ScreenRecording => !granted.screen_recording,
            Permission::Accessibility => !granted.accessibility,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_mode_needs_nothing() {
        assert_eq!(required(&Advanced::default()), []);
    }

    #[test]
    fn always_hidden_alone_needs_nothing() {
        let advanced = Advanced {
            always_hidden: true,
            ..Advanced::default()
        };
        assert_eq!(required(&advanced), []);
    }

    #[test]
    fn secondary_bar_needs_capture_and_events() {
        let advanced = Advanced {
            secondary_bar: true,
            ..Advanced::default()
        };
        assert_eq!(
            required(&advanced),
            [Permission::ScreenRecording, Permission::Accessibility]
        );
    }

    #[test]
    fn reports_only_missing_permissions() {
        let needed = required_for_reordering();
        let granted = Granted {
            screen_recording: true,
            accessibility: false,
        };
        assert_eq!(missing(&needed, granted), [Permission::Accessibility]);
        assert_eq!(
            missing(
                &needed,
                Granted {
                    screen_recording: true,
                    accessibility: true
                }
            ),
            []
        );
    }
}
