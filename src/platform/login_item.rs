//! Thin adapter over `SMAppService` for the main application login item.

use crate::core::login::LoginState;
use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// The system error code returned by an enable/disable operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginError {
    pub code: isize,
}

/// Reads the real launch-at-login state from the operating system.
#[must_use]
pub fn state() -> LoginState {
    // SAFETY: `mainAppService` is a pure constructor for the current app's service.
    let service = unsafe { SMAppService::mainAppService() };
    // SAFETY: `service` is a valid SMAppService returned by the system.
    match unsafe { service.status() }.0 {
        status if status == SMAppServiceStatus::Enabled.0 => LoginState::Enabled,
        status if status == SMAppServiceStatus::NotRegistered.0 => LoginState::Disabled,
        status if status == SMAppServiceStatus::RequiresApproval.0 => LoginState::RequiresApproval,
        _ => LoginState::Unavailable,
    }
}

/// Registers the main application to launch at login.
///
/// # Errors
///
/// Returns the system error code when registration fails.
pub fn enable() -> Result<(), LoginError> {
    // SAFETY: `mainAppService` is a pure constructor for the current app's service.
    let service = unsafe { SMAppService::mainAppService() };
    // SAFETY: `service` is a valid SMAppService returned by the system.
    unsafe { service.registerAndReturnError() }.map_err(|error| LoginError { code: error.code() })
}

/// Unregisters the main application from launching at login.
///
/// # Errors
///
/// Returns the system error code when unregistration fails.
pub fn disable() -> Result<(), LoginError> {
    // SAFETY: `mainAppService` is a pure constructor for the current app's service.
    let service = unsafe { SMAppService::mainAppService() };
    // SAFETY: `service` is a valid SMAppService returned by the system.
    unsafe { service.unregisterAndReturnError() }.map_err(|error| LoginError { code: error.code() })
}

/// Opens System Settings at the Login Items panel.
pub fn open_settings() {
    // SAFETY: The system method opens the user-facing Login Items panel and has no Rust-owned
    // arguments or retained values.
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}
