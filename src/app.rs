//! Application delegate: owns the platform adapters and wires user actions to `core`.

use crate::core::bar::{Action, Bar, Blocked};
use crate::core::config::Config;
use crate::core::l10n::{Lang, Text};
use crate::platform::menu::{self, MenuActions, MenuState};
use crate::platform::status_bar::StatusBar;
use crate::platform::{language, paths};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSEventModifierFlags,
    NSEventType,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};
use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Ivars {
    lang: OnceCell<Lang>,
    config_path: OnceCell<Option<PathBuf>>,
    config: RefCell<Config>,
    status_bar: OnceCell<StatusBar>,
    bar: RefCell<Bar>,
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
            let _ = self.ivars().config_path.set(config_path);

            let status_bar = StatusBar::new(mtm, lang, self.as_target(), sel!(iconClicked:));
            tracing::debug!(layout = ?status_bar.layout(mtm), "status items created");
            let _ = self.ivars().status_bar.set(status_bar);
            tracing::info!("launched");
        }
    }

    impl AppDelegate {
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
        };
        let actions = MenuActions {
            about: sel!(showAbout:),
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
