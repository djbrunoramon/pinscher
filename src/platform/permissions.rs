//! Checks and requests Screen Recording and Accessibility (event posting) access.

use crate::core::permissions::{Granted, Permission};
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::{
    CGPreflightPostEventAccess, CGPreflightScreenCaptureAccess, CGRequestPostEventAccess,
    CGRequestScreenCaptureAccess,
};
use objc2_foundation::{NSString, NSURL};

/// What is granted right now. Never prompts.
#[must_use]
pub fn granted() -> Granted {
    Granted {
        screen_recording: CGPreflightScreenCaptureAccess(),
        accessibility: CGPreflightPostEventAccess(),
    }
}

/// Asks the system for `permission`. The first call shows the system prompt; later calls only
/// report the current state, so callers should offer [`open_settings`] as well.
#[must_use = "false means the user still has to grant access in System Settings"]
pub fn request(permission: Permission) -> bool {
    match permission {
        Permission::ScreenRecording => CGRequestScreenCaptureAccess(),
        Permission::Accessibility => CGRequestPostEventAccess(),
    }
}

/// Opens the matching pane of System Settings › Privacy & Security.
pub fn open_settings(permission: Permission) {
    let anchor = match permission {
        Permission::ScreenRecording => "Privacy_ScreenCapture",
        Permission::Accessibility => "Privacy_Accessibility",
    };
    let url = format!("x-apple.systempreferences:com.apple.preference.security?{anchor}");
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(&url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}
