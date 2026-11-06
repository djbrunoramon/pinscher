//! Checks and requests Accessibility access without prompting at launch.

use crate::core::permissions::{Granted, Permission};
use objc2_app_kit::NSWorkspace;
use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary};
use objc2_foundation::{NSString, NSURL};

/// What is granted right now. Never prompts.
#[must_use]
pub fn granted() -> Granted {
    // SAFETY: this process-wide trust check has no arguments and never requests access.
    let accessibility = unsafe { AXIsProcessTrusted() };
    Granted {
        accessibility,
        screen_capture: objc2_core_graphics::CGPreflightScreenCaptureAccess(),
    }
}

/// Asks the system for `permission`. The prompt is asynchronous; callers should also offer
/// [`open_settings`] when access is still missing.
#[must_use = "false means the user still has to grant access in System Settings"]
pub fn request(permission: Permission) -> bool {
    if permission == Permission::ScreenCapture {
        return objc2_core_graphics::CGRequestScreenCaptureAccess();
    }
    // SAFETY: the framework exports this immutable CFString for the lifetime of the process.
    let key = unsafe { kAXTrustedCheckOptionPrompt };
    let options = CFDictionary::from_slices(&[key], &[CFBoolean::new(true)]);
    // SAFETY: options contains the documented CFString key and CFBoolean value; the dictionary
    // retains both and stays alive throughout the call.
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

/// Opens the matching pane of System Settings › Privacy & Security.
pub fn open_settings(permission: Permission) {
    let url = match permission {
        Permission::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        Permission::ScreenCapture => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
    };
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}
