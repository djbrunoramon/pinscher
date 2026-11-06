//! Application delegate: owns the status items and wires user actions to `core`.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSImage, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSString};
use std::cell::OnceCell;

#[derive(Default)]
pub struct Ivars {
    icon: OnceCell<Retained<NSStatusItem>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = self.mtm();
            let bar = NSStatusBar::systemStatusBar();
            let item = bar.statusItemWithLength(NSVariableStatusItemLength);
            item.setAutosaveName(Some(&NSString::from_str("pinscher.icon")));
            if let Some(button) = item.button(mtm) {
                let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str("dog.fill"),
                    Some(&NSString::from_str("Pinscher")),
                );
                if let Some(image) = image {
                    image.setTemplate(true);
                    button.setImage(Some(&image));
                }
            }
            let _ = self.ivars().icon.set(item);
            tracing::info!("launched");
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        // SAFETY: `NSObject::init` is the designated initializer and ivars are already set.
        unsafe { msg_send![super(this), init] }
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
