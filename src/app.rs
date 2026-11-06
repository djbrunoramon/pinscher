//! Application delegate: owns the platform adapters and wires user actions to `core`.

use crate::core::bar::{Action, Bar, Blocked};
use crate::core::config::{Config, SavedItem, Section};
use crate::core::l10n::{Lang, Text};
use crate::core::layout::{self, Dividers, MenuBarItem, unique_item};
use crate::core::login::LoginRequest;
use crate::core::permissions::Permission;
use crate::core::shortcut::{self, Modifiers};
use crate::platform::hotkey::Hotkey;
use crate::platform::menu::{self, MenuActions, MenuState};
use crate::platform::permissions;
use crate::platform::preferences::{self, SettingsActions, SettingsState, SettingsWindow};
use crate::platform::status_bar::StatusBar;
use crate::platform::{capture::CaptureCache, events::Drag, secondary_bar::SecondaryBar};
use crate::platform::{language, login_item, paths, timer::Timer};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSButton, NSEvent,
    NSEventMask, NSEventModifierFlags, NSEventType, NSPopUpButton, NSStepper, NSWindowDelegate,
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
    divider_timer: RefCell<Timer>,
    advanced_timer: RefCell<Timer>,
    capture: RefCell<CaptureCache>,
    secondary: OnceCell<SecondaryBar>,
    secondary_items: RefCell<Vec<MenuBarItem>>,
    layout_items: RefCell<Vec<MenuBarItem>>,
    layout_session: Cell<bool>,
    layout_needs_refresh: Cell<bool>,
    last_discovery: Cell<Option<std::time::Instant>>,
    drag: RefCell<Option<Drag>>,
    active_move: RefCell<Option<(MenuBarItem, Section)>>,
    restore_queue: RefCell<std::collections::VecDeque<SavedItem>>,
    restoring: Cell<bool>,
    pending_press: RefCell<Option<MenuBarItem>>,
    secondary_return: Cell<Option<crate::core::bar::Visibility>>,
    secondary_timer: RefCell<Timer>,
    layout_note: Cell<Option<Text>>,
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
            if crate::platform::permissions::granted().accessibility {
                let items = crate::core::layout::menu_bar_items(
                    crate::platform::menu_extras::items(),
                    i32::try_from(std::process::id()).unwrap_or_default(),
                );
                tracing::debug!(count = items.len(), "menu bar items");
            }
            let _ = self.ivars().status_bar.set(status_bar);
            self.configure_always_hidden();
            self.configure_advanced_timer();

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
            if self.ivars().layout_session.get() || self.ivars().pending_press.borrow().is_some() { return; }
            let layout = status_bar.layout(mtm);
            let action = self.ivars().bar.borrow_mut().on_timer(generation.as_u64(), layout);
            tracing::debug!(?layout, ?action, "auto-hide timer fired");
            match action {
                Action::Apply(visibility) => {
                    status_bar.apply(mtm, visibility);
                    self.refresh_dividers();
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
            } else if NSApplication::sharedApplication(mtm).currentEvent().is_some_and(|event| event.modifierFlags().contains(NSEventModifierFlags::Option)) {
                self.show_secondary();
            } else {
                self.toggle();
            }
        }

        #[unsafe(method(refreshDividers:))]
        fn refresh_dividers_fired(&self, _sender: Option<&AnyObject>) {
            self.refresh_dividers();
        }

        #[unsafe(method(revealAll:))]
        fn reveal_all(&self, _sender: Option<&AnyObject>) {
            self.cancel_secondary_restore();
            self.reveal_icons();
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
            self.configure_always_hidden();
            self.save_config();
            self.refresh_settings();
        }

        #[unsafe(method(settingsSecondaryBar:))]
        fn settings_secondary_bar(&self, sender: &NSButton) {
            let wanted = preferences::is_on(sender);
            // Without Accessibility the feature stays off (REQ-31); ask only now, on demand.
            let enabled = wanted && Self::request_secondary_permissions();
            self.ivars().config.borrow_mut().advanced.secondary_bar = enabled;
            if !enabled {
                self.ivars().pending_press.borrow_mut().take();
                self.restore_secondary();
                self.ivars().capture.borrow_mut().clear();
                if let Some(panel) = self.ivars().secondary.get() { panel.close(); }
            }
            self.configure_advanced_timer();
            self.save_config();
            self.refresh_settings();
        }

        #[unsafe(method(settingsOpenAccessibility:))]
        fn settings_open_accessibility(&self, _sender: Option<&AnyObject>) {
            permissions::open_settings(Permission::Accessibility);
        }

        #[unsafe(method(settingsOpenScreenCapture:))]
        fn settings_open_screen_capture(&self, _sender: Option<&AnyObject>) { permissions::open_settings(Permission::ScreenCapture); }

        #[unsafe(method(layoutRefresh:))]
        fn layout_refresh(&self, _sender: Option<&AnyObject>) { self.begin_layout(); }

        #[unsafe(method(layoutRestore:))]
        fn layout_restore(&self, _sender: Option<&AnyObject>) {
            if self.ivars().drag.borrow().as_ref().is_some_and(Drag::is_running) { return; }
            self.ivars().restoring.set(true);
            self.ivars().restore_queue.borrow_mut().clone_from(&self.ivars().config.borrow().items.iter().cloned().collect());
            self.begin_layout();
        }

        #[unsafe(method(layoutMove:))]
        fn layout_move(&self, sender: &NSPopUpButton) {
            let item = self.ivars().settings.get().and_then(|settings| settings.layout.item_for_sender(sender));
            let section = match sender.indexOfSelectedItem() { 0 => Section::Visible, 1 => Section::Hidden, 2 => Section::AlwaysHidden, _ => return };
            if let Some(item) = item { self.move_item(&item, section); }
        }

        #[unsafe(method(secondaryItem:))]
        fn secondary_item(&self, sender: &NSButton) {
            let item = usize::try_from(sender.tag()).ok().and_then(|index| self.ivars().secondary_items.borrow().get(index).cloned());
            let Some(item) = item else { return; };
            if let Some(panel) = self.ivars().secondary.get() { panel.close(); }
            if self.ivars().secondary_return.get().is_none() {
                self.ivars().secondary_return.set(Some(self.ivars().bar.borrow().visibility()));
            }
            self.ivars().secondary_timer.borrow_mut().cancel();
            *self.ivars().pending_press.borrow_mut() = Some(item);
            self.reveal_icons();
            self.ivars().timer.borrow_mut().cancel();
            self.configure_advanced_timer();
        }

        #[unsafe(method(secondaryRestore:))]
        fn secondary_restore(&self, _sender: Option<&AnyObject>) { self.restore_secondary(); }

        #[unsafe(method(advancedTick:))]
        fn advanced_tick(&self, _sender: Option<&AnyObject>) { self.poll_advanced(); }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: Option<&AnyObject>) {
            run_login_request();
        }
    }

    unsafe impl NSWindowDelegate for AppDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) { self.end_layout(); }

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
        open_screen_capture: sel!(settingsOpenScreenCapture:),
        refresh_layout: sel!(layoutRefresh:),
        restore_layout: sel!(layoutRestore:),
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
        self.cancel_secondary_restore();
        if self.ivars().layout_session.get()
            || self
                .ivars()
                .drag
                .borrow()
                .as_ref()
                .is_some_and(Drag::is_running)
        {
            return;
        }
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
                self.refresh_dividers();
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

    fn configure_always_hidden(&self) {
        let enabled = self.ivars().config.borrow().advanced.always_hidden;
        if let Some(status_bar) = self.ivars().status_bar.get() {
            status_bar.set_always_hidden(self.mtm(), self.lang(), enabled);
        }
        if enabled {
            self.ivars()
                .divider_timer
                .borrow_mut()
                .observe_layout(self.as_target(), sel!(refreshDividers:));
        } else {
            self.ivars().divider_timer.borrow_mut().cancel();
        }
        self.refresh_dividers();
    }

    fn refresh_dividers(&self) {
        let Some(status_bar) = self.ivars().status_bar.get() else {
            return;
        };
        let state =
            status_bar
                .always_hidden_x(self.mtm())
                .map_or(crate::core::bar::Divider::Thin, |x| {
                    self.ivars()
                        .bar
                        .borrow()
                        .always_hidden_divider(status_bar.layout(self.mtm()), x)
                });
        status_bar.apply_always_hidden(self.mtm(), state);
    }

    fn reveal_icons(&self) {
        let action = self.ivars().bar.borrow_mut().reveal_all();
        if let Action::Apply(visibility) = action {
            if let Some(status_bar) = self.ivars().status_bar.get() {
                status_bar.apply(self.mtm(), visibility);
            }
            self.refresh_dividers();
            if let Some(request) = self.ivars().bar.borrow().pending_collapse() {
                self.ivars().timer.borrow_mut().schedule_once(
                    request.after,
                    request.generation,
                    self.as_target(),
                );
            }
        }
    }

    fn request_secondary_permissions() -> bool {
        for permission in [Permission::Accessibility, Permission::ScreenCapture] {
            let granted = permissions::granted();
            let available = match permission {
                Permission::Accessibility => granted.accessibility,
                Permission::ScreenCapture => granted.screen_capture,
            };
            if !available && !permissions::request(permission) {
                return false;
            }
        }
        true
    }

    fn live_items() -> Vec<MenuBarItem> {
        layout::menu_bar_items(
            crate::platform::menu_extras::items(),
            i32::try_from(std::process::id()).unwrap_or_default(),
        )
        .into_iter()
        .take(crate::core::config::MAX_ITEMS)
        .collect()
    }

    fn dividers(&self) -> Option<Dividers> {
        let status = self.ivars().status_bar.get()?;
        Some(Dividers {
            separator_x: status.layout(self.mtm())?.separator_x,
            always_hidden_x: status.always_hidden_x(self.mtm()),
        })
    }

    fn configure_advanced_timer(&self) {
        if self.ivars().config.borrow().advanced.secondary_bar
            || self.ivars().layout_session.get()
            || self.ivars().pending_press.borrow().is_some()
        {
            self.ivars()
                .advanced_timer
                .borrow_mut()
                .observe_layout(self.as_target(), sel!(advancedTick:));
        } else {
            self.ivars().advanced_timer.borrow_mut().cancel();
        }
    }

    fn show_secondary(&self) {
        if self.ivars().layout_session.get() {
            self.open_settings();
            return;
        }
        let granted = permissions::granted();
        if !self.ivars().config.borrow().advanced.secondary_bar
            || !granted.accessibility
            || !granted.screen_capture
        {
            self.ivars().capture.borrow_mut().clear();
            self.set_warning(Text::SecondaryUnavailable);
            self.open_settings();
            return;
        }
        let panel = self
            .ivars()
            .secondary
            .get_or_init(|| SecondaryBar::new(self.mtm(), self.lang()));
        if panel.is_shown() {
            panel.close();
            return;
        }
        let Some(dividers) = self.dividers() else {
            return;
        };
        let items = Self::live_items();
        self.ivars().capture.borrow_mut().collect(&items);
        let hidden = items
            .into_iter()
            .filter(|item| layout::section_at(item.frame.x, dividers) != Section::Visible)
            .collect();
        *self.ivars().secondary_items.borrow_mut() = hidden;
        self.render_secondary();
    }

    fn render_secondary(&self) {
        let Some(panel) = self.ivars().secondary.get() else {
            return;
        };
        let Some(anchor) = self
            .ivars()
            .status_bar
            .get()
            .and_then(|status| status.icon_button(self.mtm()))
        else {
            return;
        };
        panel.show(
            &self.ivars().secondary_items.borrow(),
            &mut self.ivars().capture.borrow_mut(),
            self.as_target(),
            sel!(secondaryItem:),
            &anchor,
        );
    }

    fn begin_layout(&self) {
        if self
            .ivars()
            .drag
            .borrow()
            .as_ref()
            .is_some_and(Drag::is_running)
        {
            return;
        }
        if !permissions::granted().accessibility {
            self.ivars().restore_queue.borrow_mut().clear();
            self.ivars().restoring.set(false);
            let _ = permissions::request(Permission::Accessibility);
            self.ivars()
                .layout_note
                .set(Some(Text::SettingsAccessibilityMissing));
            self.refresh_layout_view();
            return;
        }
        self.cancel_secondary_restore();
        self.reveal_icons();
        self.ivars().layout_session.set(true);
        self.ivars().layout_needs_refresh.set(true);
        self.ivars().timer.borrow_mut().cancel();
        self.ivars().layout_note.set(Some(Text::LayoutInstructions));
        self.configure_advanced_timer();
        // The next run-loop tick reads settled frames after revealing the dividers.
    }

    fn end_layout(&self) {
        if !self.ivars().layout_session.replace(false) {
            return;
        }
        self.ivars().restore_queue.borrow_mut().clear();
        self.ivars().restoring.set(false);
        self.ivars().layout_items.borrow_mut().clear();
        self.ivars().active_move.borrow_mut().take();
        if let Some(drag) = self.ivars().drag.borrow_mut().as_mut() {
            drag.cancel();
        }
        if let Some(status) = self.ivars().status_bar.get() {
            let action = self
                .ivars()
                .bar
                .borrow_mut()
                .collapse(status.layout(self.mtm()));
            if let Action::Apply(visibility) = action {
                status.apply(self.mtm(), visibility);
            }
        }
        self.refresh_dividers();
        self.configure_advanced_timer();
    }

    fn refresh_layout_view(&self) {
        if let Some(settings) = self.ivars().settings.get() {
            let busy = !self.ivars().layout_session.get()
                || self
                    .ivars()
                    .drag
                    .borrow()
                    .as_ref()
                    .is_some_and(Drag::is_running);
            settings.layout.refresh(
                &self.ivars().layout_items.borrow(),
                self.dividers(),
                self.as_target(),
                sel!(layoutMove:),
                self.ivars()
                    .layout_note
                    .get()
                    .unwrap_or(Text::LayoutInstructions),
                busy,
            );
        }
    }

    fn move_item(&self, expected: &MenuBarItem, section: Section) {
        if !self.ivars().layout_session.get() || self.ivars().active_move.borrow().is_some() {
            return;
        }
        let live = Self::live_items();
        let Some(item) = unique_item(&live, expected).cloned() else {
            self.move_failed();
            return;
        };
        let Some((bounds, icon, dividers)) = self
            .ivars()
            .status_bar
            .get()
            .and_then(|status| status.geometry(self.mtm()))
        else {
            self.move_failed();
            return;
        };
        let plan = match crate::core::drag::plan_for_section(&item, section, dividers, bounds, icon)
        {
            Ok(plan) => plan,
            Err(error) => {
                tracing::warn!(?error, "move plan refused");
                self.move_failed();
                return;
            }
        };
        let mut slot = self.ivars().drag.borrow_mut();
        let drag = slot.get_or_insert_with(|| Drag::new(self.mtm()));
        let Some(status) = self.ivars().status_bar.get().cloned() else {
            return;
        };
        let expected_geometry = (bounds, icon, dividers);
        let geometry_is_current = move || {
            MainThreadMarker::new().and_then(|mtm| status.geometry(mtm)) == Some(expected_geometry)
        };
        if let Err(error) = drag.start(&plan, geometry_is_current) {
            tracing::warn!(error = %error, "move refused");
            drop(slot);
            self.move_failed();
            return;
        }
        drop(slot);
        *self.ivars().active_move.borrow_mut() = Some((item, section));
        self.ivars().layout_note.set(Some(Text::LayoutBusy));
        self.refresh_layout_view();
    }

    fn move_failed(&self) {
        self.ivars().restoring.set(false);
        self.ivars().restore_queue.borrow_mut().clear();
        self.ivars().layout_note.set(Some(Text::LayoutMoveFailed));
        self.refresh_layout_view();
    }

    fn finish_move(&self, items: &[MenuBarItem]) {
        let active = self.ivars().active_move.borrow_mut().take();
        if let Some((expected, section)) = active {
            self.ivars().layout_needs_refresh.set(true);
            let moved = unique_item(items, &expected)
                .zip(self.dividers())
                .is_some_and(|(item, dividers)| {
                    layout::section_at(item.frame.x, dividers) == section
                });
            if moved {
                let remembered = layout::remember_section(
                    &mut self.ivars().config.borrow_mut(),
                    &expected,
                    section,
                );
                if remembered && self.save_config() {
                    self.ivars().layout_note.set(Some(Text::LayoutInstructions));
                } else {
                    self.ivars().restoring.set(false);
                    self.ivars().restore_queue.borrow_mut().clear();
                    self.ivars().layout_note.set(Some(Text::LayoutSaveFailed));
                }
            } else {
                self.ivars().restore_queue.borrow_mut().clear();
                self.move_failed();
            }
        }
    }

    fn cancel_secondary_restore(&self) {
        self.ivars().secondary_return.set(None);
        self.ivars().secondary_timer.borrow_mut().cancel();
        self.ivars().pending_press.borrow_mut().take();
    }

    fn restore_secondary(&self) {
        let Some(previous) = self.ivars().secondary_return.take() else {
            return;
        };
        self.ivars().secondary_timer.borrow_mut().cancel();
        if let Some(status) = self.ivars().status_bar.get() {
            let action = self
                .ivars()
                .bar
                .borrow_mut()
                .finish_temporary_reveal(previous, status.layout(self.mtm()));
            if let Action::Apply(visibility) = action {
                status.apply(self.mtm(), visibility);
            }
        }
        self.refresh_dividers();
        if let Some(request) = self.ivars().bar.borrow().pending_collapse() {
            self.ivars().timer.borrow_mut().schedule_once(
                request.after,
                request.generation,
                self.as_target(),
            );
        }
    }

    fn press_pending(&self, items: &[MenuBarItem]) {
        let pending = self.ivars().pending_press.borrow_mut().take();
        if let Some(expected) = pending {
            let result = unique_item(items, &expected)
                .zip(
                    self.ivars()
                        .status_bar
                        .get()
                        .and_then(|status| status.geometry(self.mtm())),
                )
                .filter(|(item, (bounds, _, _))| {
                    bounds.contains(item.frame.x, item.frame.y)
                        && bounds.contains(
                            item.frame.x + item.frame.width,
                            item.frame.y + item.frame.height,
                        )
                })
                .map(|(item, _)| {
                    crate::platform::menu_extras::press(item.owner_pid, &item.title, item.frame.x)
                });
            if !matches!(result, Some(Ok(()))) {
                self.set_warning(Text::SecondaryItemUnavailable);
                self.show_menu();
            }
            let delay = self.ivars().config.borrow().auto_hide.delay_secs;
            self.ivars().secondary_timer.borrow_mut().schedule_action(
                Duration::from_secs(u64::from(delay)),
                self.as_target(),
                sel!(secondaryRestore:),
            );
            self.configure_advanced_timer();
        }
    }

    fn poll_advanced(&self) {
        if self
            .ivars()
            .drag
            .borrow()
            .as_ref()
            .is_some_and(Drag::is_running)
        {
            return;
        }
        let granted = permissions::granted();
        if !granted.accessibility {
            self.ivars().restore_queue.borrow_mut().clear();
            self.ivars().restoring.set(false);
            self.ivars().active_move.borrow_mut().take();
            self.ivars().pending_press.borrow_mut().take();
            self.restore_secondary();
            self.ivars().layout_items.borrow_mut().clear();
            self.ivars()
                .layout_note
                .set(Some(Text::SettingsAccessibilityMissing));
            self.ivars().capture.borrow_mut().clear();
            if let Some(panel) = self.ivars().secondary.get() {
                panel.close();
            }
            self.refresh_layout_view();
            self.refresh_settings();
            return;
        }
        let urgent = self.ivars().layout_needs_refresh.get()
            || self.ivars().active_move.borrow().is_some()
            || self.ivars().pending_press.borrow().is_some()
            || !self.ivars().restore_queue.borrow().is_empty();
        if !urgent
            && self
                .ivars()
                .last_discovery
                .get()
                .is_some_and(|last| last.elapsed() < Duration::from_secs(5))
        {
            return;
        }
        self.ivars()
            .last_discovery
            .set(Some(std::time::Instant::now()));
        let items = Self::live_items();
        self.finish_move(&items);
        if self.ivars().layout_session.get() {
            if self.ivars().layout_needs_refresh.replace(false) {
                self.ivars().layout_items.borrow_mut().clone_from(&items);
            }
            let saved = self.ivars().restore_queue.borrow_mut().pop_front();
            if let Some(saved) = saved {
                let candidates: Vec<_> = items
                    .iter()
                    .filter(|item| item.owner == saved.owner && item.title == saved.title)
                    .collect();
                if candidates.len() == 1
                    && self.dividers().is_some_and(|dividers| {
                        layout::section_at(candidates[0].frame.x, dividers) != saved.section
                    })
                {
                    self.move_item(candidates[0], saved.section);
                }
            }
            if self.ivars().restoring.get()
                && self.ivars().restore_queue.borrow().is_empty()
                && self.ivars().active_move.borrow().is_none()
            {
                self.ivars().restoring.set(false);
                self.ivars().layout_note.set(Some(Text::LayoutRestored));
            }
            self.refresh_layout_view();
        }
        self.press_pending(&items);
        if self.ivars().config.borrow().advanced.secondary_bar {
            if !granted.screen_capture {
                self.ivars().capture.borrow_mut().clear();
                if let Some(panel) = self.ivars().secondary.get() {
                    panel.close();
                }
                self.refresh_settings();
                return;
            }
            let mut cache = self.ivars().capture.borrow_mut();
            cache.collect(&items);
            if let Some((bounds, _, _)) = self
                .ivars()
                .status_bar
                .get()
                .and_then(|status| status.geometry(self.mtm()))
            {
                cache.request(&items, bounds);
            }
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
        self.refresh_layout_view();
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

    fn save_config(&self) -> bool {
        let Some(Some(path)) = self.ivars().config_path.get() else {
            return false;
        };
        if let Err(error) = crate::core::config::save(path, &self.ivars().config.borrow()) {
            tracing::warn!(error = %error, "config");
            return false;
        }
        true
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
            accessibility_missing: !permissions::granted().accessibility,
            screen_capture_missing: !permissions::granted().screen_capture,
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
            reveal_all: sel!(revealAll:),
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
