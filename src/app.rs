//! Application delegate: owns the platform adapters and wires user actions to `core`.

use crate::core::bar::{Action, Bar, Blocked};
use crate::core::config::Config;
use crate::core::l10n::{Lang, Text};
use crate::core::login::LoginRequest;
use crate::platform::hotkey::Hotkey;
use crate::platform::menu::{self, MenuActions, MenuState};
use crate::platform::status_bar::StatusBar;
use crate::platform::{language, login_item, paths, timer::Timer};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSEventModifierFlags,
    NSEventType,
};
use objc2_foundation::{NSNotification, NSNumber, NSObject, NSObjectProtocol};
use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Ivars {
    lang: OnceCell<Lang>,
    config_path: OnceCell<Option<PathBuf>>,
    config: RefCell<Config>,
    status_bar: OnceCell<StatusBar>,
    hotkey: OnceCell<Hotkey>,
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
            let auto_hide = self.ivars().config.borrow().auto_hide.clone();
            self.ivars().bar.borrow_mut().set_auto_hide(if auto_hide.enabled {
                Some(Duration::from_secs(u64::from(auto_hide.delay_secs)))
            } else {
                None
            });
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

            let shortcut_text = self.ivars().config.borrow().shortcut.toggle.clone();
            if !shortcut_text.is_empty() {
                let result = crate::core::shortcut::parse(&shortcut_text)
                    .map_err(|error| error.to_string())
                    .and_then(|shortcut| {
                        let mut hotkey = Hotkey::new(mtm).map_err(|error| error.to_string())?;
                        hotkey
                            .register(&shortcut)
                            .map_err(|error| error.to_string())?;
                        Ok(hotkey)
                    });
                match result {
                    Ok(hotkey) => {
                        let _ = self.ivars().hotkey.set(hotkey);
                    }
                    Err(error) => {
                        self.set_warning(Text::WarningShortcutUnavailable);
                        tracing::warn!(error = %error, "shortcut");
                    }
                }
            }
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

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: Option<&AnyObject>) {
            let request = crate::core::login::menu_item(login_item::state()).action;
            match request {
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
    }
);

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
