//! Application delegate: owns the platform adapters and wires user actions to `core`.

use crate::core::bar::{Action, Bar, Blocked};
use crate::core::config::Config;
use crate::core::l10n::{Lang, Text};
use crate::core::login::LoginRequest;
use crate::core::permissions::Permission;
use crate::core::shortcut::{self, Modifiers};
use crate::platform::hotkey::Hotkey;
use crate::platform::menu::{self, MenuActions, MenuState};
use crate::platform::permissions;
use crate::platform::preferences::{self, SettingsActions, SettingsState, SettingsWindow};
use crate::platform::status_bar::StatusBar;
use crate::platform::{language, login_item, paths, timer::Timer};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSButton, NSEvent,
    NSEventMask, NSEventModifierFlags, NSEventType, NSStepper, NSWindowDelegate,
};
use objc2_foundation::{NSNotification, NSNumber, NSObject, NSObjectProtocol};
use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Ivars {
    lang: OnceCell<Lang>,
    config_path: OnceCell<Option<PathBuf>>,
    config: RefCell<Config>,
    status_bar: OnceCell<StatusBar>,
    hotkey: RefCell<Option<Hotkey>>,
    settings: OnceCell<SettingsWindow>,
    recording_shortcut: Cell<bool>,
    key_monitor: RefCell<Option<Retained<AnyObject>>>,
    bar: RefCell<Bar>,
    timer: RefCell<Timer>,
    warnings: RefCell<Vec<Text>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    #[derive(Debug)]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        /// Opening Pinscher again while it runs (Finder, Spotlight, `open`) shows Settings.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _app: &NSApplication, _visible: bool) -> bool {
            self.open_settings();
            false
        }

        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = self.mtm();
            let lang = language::current();
            let _ = self.ivars().lang.set(lang);

            let config_path = paths::config_file();
            let loaded = config_path
                .as_deref()
                .map(crate::core::config::load)
                .unwrap_or_default();
            for warning in &loaded.warnings {
                tracing::warn!(?warning, "config");
            }
            *self.ivars().config.borrow_mut() = loaded.config;
            self.apply_auto_hide();
            let _ = self.ivars().config_path.set(config_path);

            let status_bar = StatusBar::new(mtm, lang, self.as_target(), sel!(iconClicked:));
            tracing::debug!(layout = ?status_bar.layout(mtm), "status items created");
            let granted = crate::platform::permissions::granted();
            let items = crate::core::layout::menu_bar_items(
                crate::platform::window_list::windows(),
                i32::try_from(std::process::id()).unwrap_or_default(),
            );
            tracing::debug!(count = items.len(), ?granted, "menu bar items");
            let _ = self.ivars().status_bar.set(status_bar);

            match Hotkey::new(mtm) {
                Ok(hotkey) => *self.ivars().hotkey.borrow_mut() = Some(hotkey),
                Err(error) => tracing::warn!(error = %error, "shortcut manager"),
            }
            self.apply_shortcut();
            tracing::info!("launched");
        }
    }

    impl AppDelegate {
        #[unsafe(method(hotkeyPressed:))]
        fn hotkey_pressed(&self, _sender: Option<&AnyObject>) {
            self.toggle();
        }

        #[unsafe(method(autoHideFired:))]
        fn auto_hide_fired(&self, generation: &NSNumber) {
            let mtm = self.mtm();
            let Some(status_bar) = self.ivars().status_bar.get() else {
                return;
            };
            let layout = status_bar.layout(mtm);
            let action = self.ivars().bar.borrow_mut().on_timer(generation.as_u64(), layout);
            tracing::debug!(?layout, ?action, "auto-hide timer fired");
            match action {
                Action::Apply(visibility) => {
                    status_bar.apply(mtm, visibility);
                    if visibility == crate::core::bar::Visibility::Collapsed {
                        self.ivars().timer.borrow_mut().cancel();
                    }
                }
                Action::Blocked(blocked) => tracing::debug!(?blocked, "auto-hide collapse blocked"),
                Action::None => {}
            }
        }

        #[unsafe(method(iconClicked:))]
        fn icon_clicked(&self, _sender: Option<&AnyObject>) {
            let mtm = self.mtm();
            let event = NSApplication::sharedApplication(mtm).currentEvent();
            let wants_menu = event.is_some_and(|event| {
                event.r#type() == NSEventType::RightMouseUp
                    || event.modifierFlags().contains(NSEventModifierFlags::Control)
            });
            if wants_menu {
                self.show_menu();
            } else {
                self.toggle();
            }
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            let app = NSApplication::sharedApplication(self.mtm());
            app.activate();
            app.orderFrontStandardAboutPanel(None);
        }

        #[unsafe(method(showSettings:))]
        fn show_settings(&self, _sender: Option<&AnyObject>) {
            self.open_settings();
        }

        #[unsafe(method(settingsRecordShortcut:))]
        fn settings_record_shortcut(&self, _sender: Option<&AnyObject>) {
            self.start_recording();
        }

        #[unsafe(method(settingsClearShortcut:))]
        fn settings_clear_shortcut(&self, _sender: Option<&AnyObject>) {
            self.stop_recording();
            self.ivars().config.borrow_mut().shortcut.toggle.clear();
            self.save_config();
            self.apply_shortcut();
            self.refresh_settings();
        }

        /// Called by the local key monitor; returns whether the key press was consumed.
        #[unsafe(method(settingsKeyDown:))]
        fn settings_key_down(&self, event: &NSEvent) -> Bool {
            Bool::new(self.handle_recorder_key(event))
        }

        #[unsafe(method(settingsAutoHide:))]
        fn settings_auto_hide(&self, sender: &NSButton) {
            self.ivars().config.borrow_mut().auto_hide.enabled = preferences::is_on(sender);
            self.save_config();
            self.apply_auto_hide();
            self.refresh_settings();
        }

        #[unsafe(method(settingsAutoHideDelay:))]
        fn settings_auto_hide_delay(&self, sender: &NSStepper) {
            let range = crate::core::config::AUTO_HIDE_DELAY_RANGE;
            let delay = u32::try_from(sender.integerValue())
                .unwrap_or(*range.start())
                .clamp(*range.start(), *range.end());
            self.ivars().config.borrow_mut().auto_hide.delay_secs = delay;
            self.save_config();
            self.apply_auto_hide();
            self.refresh_settings();
        }

        #[unsafe(method(settingsLaunchAtLogin:))]
        fn settings_launch_at_login(&self, _sender: Option<&AnyObject>) {
            run_login_request();
            self.refresh_settings();
        }

        #[unsafe(method(settingsAlwaysHidden:))]
        fn settings_always_hidden(&self, sender: &NSButton) {
            self.ivars().config.borrow_mut().advanced.always_hidden = preferences::is_on(sender);
            self.save_config();
            self.refresh_settings();
        }

        #[unsafe(method(settingsSecondaryBar:))]
        fn settings_secondary_bar(&self, sender: &NSButton) {
            let wanted = preferences::is_on(sender);
            // Without Accessibility the feature stays off (REQ-31); ask only now, on demand.
            let enabled = wanted
                && (permissions::granted().accessibility
                    || permissions::request(Permission::Accessibility));
            self.ivars().config.borrow_mut().advanced.secondary_bar = enabled;
            self.save_config();
            self.refresh_settings();
        }

        #[unsafe(method(settingsOpenAccessibility:))]
        fn settings_open_accessibility(&self, _sender: Option<&AnyObject>) {
            permissions::open_settings(Permission::Accessibility);
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: Option<&AnyObject>) {
            run_login_request();
        }
    }

    unsafe impl NSWindowDelegate for AppDelegate {
        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.stop_recording();
            self.refresh_settings();
        }
    }
);

fn settings_actions() -> SettingsActions {
    SettingsActions {
        record_shortcut: sel!(settingsRecordShortcut:),
        clear_shortcut: sel!(settingsClearShortcut:),
        auto_hide: sel!(settingsAutoHide:),
        auto_hide_delay: sel!(settingsAutoHideDelay:),
        launch_at_login: sel!(settingsLaunchAtLogin:),
        always_hidden: sel!(settingsAlwaysHidden:),
        secondary_bar: sel!(settingsSecondaryBar:),
        open_accessibility: sel!(settingsOpenAccessibility:),
    }
}

impl AppDelegate {
    /// Records `event` as the new shortcut while recording; returns whether it was consumed.
    fn handle_recorder_key(&self, event: &NSEvent) -> bool {
        const ESCAPE: u16 = 53;
        if !self.ivars().recording_shortcut.get() {
            return false;
        }
        if event.keyCode() == ESCAPE {
            self.stop_recording();
            self.refresh_settings();
            return true;
        }
        let flags = event.modifierFlags();
        let modifiers = Modifiers {
            ctrl: flags.contains(NSEventModifierFlags::Control),
            alt: flags.contains(NSEventModifierFlags::Option),
            shift: flags.contains(NSEventModifierFlags::Shift),
            cmd: flags.contains(NSEventModifierFlags::Command),
        };
        match shortcut::from_key_press(modifiers, event.keyCode()) {
            Ok(recorded) => {
                self.ivars().config.borrow_mut().shortcut.toggle = recorded.to_config_string();
                self.stop_recording();
                self.save_config();
                self.apply_shortcut();
                self.refresh_settings();
            }
            // Keep listening: the user may still be reaching for a modifier.
            Err(_) => objc2_app_kit::NSBeep(),
        }
        true
    }
}

/// Carries out what the launch-at-login item currently offers (enable, disable, open settings).
fn run_login_request() {
    match crate::core::login::menu_item(login_item::state()).action {
        Some(LoginRequest::Enable) => {
            if let Err(error) = login_item::enable() {
                tracing::warn!(code = error.code, "login item");
            }
        }
        Some(LoginRequest::Disable) => {
            if let Err(error) = login_item::disable() {
                tracing::warn!(code = error.code, "login item");
            }
        }
        Some(LoginRequest::OpenSettings) => login_item::open_settings(),
        None => {}
    }
}

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        // SAFETY: `NSObject::init` is the designated initializer and ivars are already set.
        unsafe { msg_send![super(this), init] }
    }

    fn as_target(&self) -> &AnyObject {
        self.as_ref()
    }

    fn lang(&self) -> Lang {
        self.ivars().lang.get().copied().unwrap_or_default()
    }

    fn toggle(&self) {
        let mtm = self.mtm();
        let Some(status_bar) = self.ivars().status_bar.get() else {
            return;
        };
        let layout = status_bar.layout(mtm);
        let action = self.ivars().bar.borrow_mut().toggle(layout);
        tracing::debug!(?layout, ?action, "toggle");
        match action {
            Action::Apply(visibility) => {
                self.ivars().warnings.borrow_mut().clear();
                status_bar.apply(mtm, visibility);
                if visibility == crate::core::bar::Visibility::Expanded {
                    if let Some(request) = self.ivars().bar.borrow().pending_collapse() {
                        self.ivars().timer.borrow_mut().schedule_once(
                            request.after,
                            request.generation,
                            self.as_target(),
                        );
                    }
                } else {
                    self.ivars().timer.borrow_mut().cancel();
                }
            }
            Action::Blocked(Blocked::SeparatorRightOfIcon) => {
                self.set_warning(Text::WarningSeparatorOrder);
                self.show_menu();
            }
            Action::Blocked(Blocked::LayoutUnknown) => {
                tracing::warn!("toggle ignored: status item positions unknown");
            }
            Action::None => {}
        }
    }

    fn open_settings(&self) {
        let mtm = self.mtm();
        let settings = self.ivars().settings.get_or_init(|| {
            SettingsWindow::new(
                mtm,
                self.lang(),
                self.as_target(),
                ProtocolObject::from_ref(self),
                settings_actions(),
            )
        });
        settings.refresh(&self.settings_state());
        settings.show(mtm);
    }

    fn apply_auto_hide(&self) {
        let auto_hide = self.ivars().config.borrow().auto_hide.clone();
        self.ivars().bar.borrow_mut().set_auto_hide(
            auto_hide
                .enabled
                .then(|| Duration::from_secs(u64::from(auto_hide.delay_secs))),
        );
    }

    /// Registers the configured shortcut (or none) and updates the menu warning.
    fn apply_shortcut(&self) {
        let toggle = self.ivars().config.borrow().shortcut.toggle.clone();
        let mut hotkey = self.ivars().hotkey.borrow_mut();
        let result = match (hotkey.as_mut(), toggle.is_empty()) {
            (None, _) => Err(String::from("shortcut manager unavailable")),
            (Some(hotkey), true) => hotkey.unregister().map_err(|error| error.to_string()),
            (Some(hotkey), false) => shortcut::parse(&toggle)
                .map_err(|error| error.to_string())
                .and_then(|parsed| hotkey.register(&parsed).map_err(|error| error.to_string())),
        };
        drop(hotkey);
        self.ivars()
            .warnings
            .borrow_mut()
            .retain(|warning| *warning != Text::WarningShortcutUnavailable);
        if let Err(error) = result {
            self.set_warning(Text::WarningShortcutUnavailable);
            tracing::warn!(error = %error, "shortcut");
        }
    }

    fn save_config(&self) {
        let Some(Some(path)) = self.ivars().config_path.get() else {
            return;
        };
        if let Err(error) = crate::core::config::save(path, &self.ivars().config.borrow()) {
            tracing::warn!(error = %error, "config");
        }
    }

    fn settings_state(&self) -> SettingsState {
        let config = self.ivars().config.borrow();
        let login = crate::core::login::menu_item(login_item::state());
        SettingsState {
            shortcut: shortcut::parse(&config.shortcut.toggle)
                .ok()
                .map(|parsed| parsed.display()),
            recording_shortcut: self.ivars().recording_shortcut.get(),
            shortcut_unavailable: self
                .ivars()
                .warnings
                .borrow()
                .contains(&Text::WarningShortcutUnavailable),
            auto_hide: config.auto_hide.enabled,
            auto_hide_delay: config.auto_hide.delay_secs,
            launch_at_login: login.checked,
            launch_at_login_enabled: login.action.is_some(),
            launch_at_login_note: login.note,
            always_hidden: config.advanced.always_hidden,
            secondary_bar: config.advanced.secondary_bar,
            accessibility_missing: !crate::core::permissions::missing(
                &crate::core::permissions::required(&config.advanced),
                permissions::granted(),
            )
            .is_empty(),
        }
    }

    fn refresh_settings(&self) {
        if let Some(settings) = self.ivars().settings.get() {
            settings.refresh(&self.settings_state());
        }
    }

    /// Listens for the next key press in the Settings window (local monitor: other apps' keys
    /// never reach Pinscher).
    fn start_recording(&self) {
        if self.ivars().recording_shortcut.replace(true) {
            return;
        }
        let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
            let Some(mtm) = MainThreadMarker::new() else {
                return event.as_ptr();
            };
            let Some(delegate) = NSApplication::sharedApplication(mtm).delegate() else {
                return event.as_ptr();
            };
            // SAFETY: the delegate is `AppDelegate`, which implements `settingsKeyDown:` taking
            // an `NSEvent` and returning `BOOL`; the event pointer is valid for this call.
            let consumed: Bool = unsafe { msg_send![&*delegate, settingsKeyDown: event.as_ref()] };
            if consumed.as_bool() {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            }
        });
        // SAFETY: the handler matches the documented signature and is copied by AppKit.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
        };
        *self.ivars().key_monitor.borrow_mut() = monitor;
        self.ivars()
            .hotkey
            .borrow_mut()
            .as_mut()
            .map(Hotkey::unregister);
        self.refresh_settings();
    }

    fn stop_recording(&self) {
        if !self.ivars().recording_shortcut.replace(false) {
            return;
        }
        if let Some(monitor) = self.ivars().key_monitor.borrow_mut().take() {
            // SAFETY: `monitor` was returned by `addLocalMonitorForEventsMatchingMask_handler`.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        self.apply_shortcut();
    }

    fn set_warning(&self, warning: Text) {
        let mut warnings = self.ivars().warnings.borrow_mut();
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }

    fn show_menu(&self) {
        let mtm = self.mtm();
        let Some(button) = self
            .ivars()
            .status_bar
            .get()
            .and_then(|status_bar| status_bar.icon_button(mtm))
        else {
            return;
        };
        let state = MenuState {
            warnings: self.ivars().warnings.borrow().clone(),
            launch_at_login: Some(crate::core::login::menu_item(login_item::state())),
        };
        let actions = MenuActions {
            settings: sel!(showSettings:),
            about: sel!(showAbout:),
            toggle_login: sel!(toggleLaunchAtLogin:),
        };
        menu::show(mtm, self.lang(), &state, self.as_target(), actions, &button);
    }
}

pub fn run() {
    let mtm = MainThreadMarker::new().expect("Pinscher must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
