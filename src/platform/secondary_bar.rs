//! Transient, memory-only second bar anchored to Pinscher's status item.

use crate::core::l10n::{Lang, Text, text};
use crate::core::layout::MenuBarItem;
use crate::platform::capture::CaptureCache;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSImage, NSImageScaling, NSPopover, NSPopoverBehavior, NSRunningApplication,
    NSScrollView, NSStatusBarButton, NSTextField, NSView, NSViewController,
};
use objc2_foundation::{NSPoint, NSRect, NSRectEdge, NSSize, NSString};

#[derive(Debug)]
pub struct SecondaryBar {
    popover: Retained<NSPopover>,
    controller: Retained<NSViewController>,
    lang: Lang,
}

impl SecondaryBar {
    #[must_use]
    pub fn new(mtm: MainThreadMarker, lang: Lang) -> Self {
        let popover = NSPopover::new(mtm);
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setAnimates(false);
        let controller = NSViewController::new(mtm);
        popover.setContentViewController(Some(&controller));
        Self {
            popover,
            controller,
            lang,
        }
    }

    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.popover.isShown()
    }

    pub fn close(&self) {
        // SAFETY: nil sender is supported; the retained popover is used on its main thread.
        unsafe { self.popover.performClose(None) };
    }

    pub fn show(
        &self,
        items: &[MenuBarItem],
        cache: &mut CaptureCache,
        target: &AnyObject,
        action: Sel,
        anchor: &NSStatusBarButton,
    ) {
        let mtm = self.popover.mtm();
        let count = u32::try_from(items.len().min(256)).unwrap_or(256);
        let width = if count == 0 {
            440.0
        } else {
            (f64::from(count) * 40.0 + 16.0).min(600.0)
        };
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, 0.0, width, 64.0));
        scroll.setHasHorizontalScroller(true);
        let document = NSView::initWithFrame(
            NSView::alloc(mtm),
            rect(0.0, 0.0, (f64::from(count) * 40.0 + 16.0).max(width), 48.0),
        );
        for (index, item) in items.iter().take(256).enumerate() {
            let image = cache
                .image(item, mtm)
                .or_else(|| {
                    NSRunningApplication::runningApplicationWithProcessIdentifier(item.owner_pid)?
                        .icon()
                })
                .or_else(|| {
                    NSImage::imageWithSystemSymbolName_accessibilityDescription(
                        &NSString::from_str("app"),
                        Some(&NSString::from_str(&item.title)),
                    )
                });
            let Some(image) = image else {
                continue;
            };
            // SAFETY: target is the app delegate and implements the one-argument action.
            let button = unsafe {
                NSButton::buttonWithImage_target_action(&image, Some(target), Some(action), mtm)
            };
            button.setTag(isize::try_from(index).unwrap_or(-1));
            button.setFrame(rect(
                8.0 + f64::from(u32::try_from(index).unwrap_or(0)) * 40.0,
                8.0,
                32.0,
                32.0,
            ));
            button.setImageScaling(NSImageScaling::ScaleProportionallyDown);
            button.setToolTip(Some(&NSString::from_str(if item.title.is_empty() {
                &item.owner
            } else {
                &item.title
            })));
            document.addSubview(&button);
        }
        if count == 0 {
            let label = NSTextField::wrappingLabelWithString(
                &NSString::from_str(text(self.lang, Text::SecondaryEmpty)),
                mtm,
            );
            label.setFrame(rect(10.0, 2.0, 420.0, 42.0));
            document.addSubview(&label);
        }
        scroll.setDocumentView(Some(&document));
        self.controller.setView(&scroll);
        self.popover.setContentSize(NSSize::new(width, 64.0));
        self.popover.showRelativeToRect_ofView_preferredEdge(
            anchor.bounds(),
            anchor,
            NSRectEdge::MinY,
        );
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
