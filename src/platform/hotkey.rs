//! Adapter for registering the app's global toggle shortcut.

use crate::core::shortcut::Shortcut;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use objc2::MainThreadMarker;
use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSApplication;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

static REGISTERED_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("could not create global shortcut manager")]
    Manager,
    #[error("shortcut format rejected")]
    Parse,
    #[error("could not unregister previous shortcut")]
    Unregister,
    #[error("could not register shortcut")]
    Register,
}

/// Owns the process-wide global shortcut manager and the currently registered shortcut.
pub struct Hotkey {
    manager: GlobalHotKeyManager,
    registered: Option<HotKey>,
}

impl Hotkey {
    /// Creates the manager and installs the event bridge to the main-thread delegate.
    ///
    /// # Errors
    ///
    /// Returns [`HotkeyError::Manager`] if the operating system rejects manager creation.
    pub fn new(_mtm: MainThreadMarker) -> Result<Self, HotkeyError> {
        GlobalHotKeyEvent::set_event_handler(Some(|event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed
                || event.id != REGISTERED_ID.load(Ordering::Acquire)
            {
                return;
            }
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let app = NSApplication::sharedApplication(mtm);
            let Some(delegate) = app.delegate() else {
                return;
            };
            // SAFETY: the application delegate implements `hotkeyPressed:` and the nil sender
            // matches the Objective-C action method's nullable sender parameter.
            unsafe {
                let _: () = msg_send![&*delegate, hotkeyPressed: None::<&AnyObject>];
            }
        }));
        let manager = GlobalHotKeyManager::new().map_err(|_| HotkeyError::Manager)?;
        Ok(Self {
            manager,
            registered: None,
        })
    }

    /// Replaces the registered shortcut, leaving the manager usable on every failure.
    ///
    /// # Errors
    ///
    /// Returns an error when the shortcut cannot be converted, the previous registration cannot
    /// be removed, or the new registration is rejected by the operating system.
    pub fn register(&mut self, shortcut: &Shortcut) -> Result<(), HotkeyError> {
        let hotkey =
            HotKey::from_str(&shortcut.to_config_string()).map_err(|_| HotkeyError::Parse)?;
        self.unregister()?;
        self.manager
            .register(hotkey)
            .map_err(|_| HotkeyError::Register)?;
        REGISTERED_ID.store(hotkey.id(), Ordering::Release);
        self.registered = Some(hotkey);
        Ok(())
    }
}

impl Hotkey {
    /// Removes the registered shortcut, if any.
    ///
    /// # Errors
    ///
    /// Returns [`HotkeyError::Unregister`] when the operating system refuses.
    pub fn unregister(&mut self) -> Result<(), HotkeyError> {
        REGISTERED_ID.store(0, Ordering::Release);
        if let Some(previous) = self.registered.take() {
            self.manager
                .unregister(previous)
                .map_err(|_| HotkeyError::Unregister)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Hotkey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hotkey")
            .field("registered", &self.registered)
            .finish_non_exhaustive()
    }
}
