//! The Pinscher icon and the separator, as `NSStatusItem`s.
//!
//! Holds no decisions: `core::bar` decides, this module only reads positions and applies state.

use crate::core::bar::{Layout, Visibility};
use crate::core::l10n::{Lang, Text, text};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2_app_kit::{
    NSCellImagePosition, NSEventMask, NSImage, NSStatusBar, NSStatusBarButton, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::NSString;

/// Wide enough to push every item to its left off any screen.
const COLLAPSED_LENGTH: f64 = 10_000.0;

/// The status items Pinscher owns.
#[derive(Debug)]
pub struct StatusBar {
    icon: Retained<NSStatusItem>,
    separator: Retained<NSStatusItem>,
}

impl StatusBar {
    /// Creates the icon (rightmost) and then the separator, which macOS places to its left on
    /// first launch. Autosave names keep any reordering the user does across launches.
    /// Clicks on the icon (left and right button) are sent to `action` on `target`.
    #[must_use]
    pub fn new(mtm: MainThreadMarker, lang: Lang, target: &AnyObject, action: Sel) -> Self {
        let bar = NSStatusBar::systemStatusBar();

        let icon = bar.statusItemWithLength(NSVariableStatusItemLength);
        icon.setAutosaveName(Some(&NSString::from_str("pinscher.icon")));
        if let Some(button) = icon.button(mtm) {
            set_symbol(&button, "dog.fill", text(lang, Text::IconAccessibility));
            button.setToolTip(Some(&NSString::from_str(text(lang, Text::IconTooltip))));
            // SAFETY: `target` outlives the status item (it is the app delegate) and `action`
            // is a method it implements with the `-(void)x:(id)sender` signature.
            unsafe {
                button.setTarget(Some(target));
                button.setAction(Some(action));
            }
            button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
        }

        let separator = bar.statusItemWithLength(NSVariableStatusItemLength);
        separator.setAutosaveName(Some(&NSString::from_str("pinscher.separator")));
        if let Some(button) = separator.button(mtm) {
            set_symbol(
                &button,
                "line.diagonal",
                text(lang, Text::SeparatorAccessibility),
            );
        }

        Self { icon, separator }
    }

    /// Current left edge of each item on screen, or `None` while either is not on screen.
    #[must_use]
    pub fn layout(&self, mtm: MainThreadMarker) -> Option<Layout> {
        Some(Layout {
            icon_x: left_edge(&self.icon, mtm)?,
            separator_x: left_edge(&self.separator, mtm)?,
        })
    }

    /// Shows `visibility` by resizing the separator.
    pub fn apply(&self, mtm: MainThreadMarker, visibility: Visibility) {
        let collapsed = visibility == Visibility::Collapsed;
        self.separator.setLength(if collapsed {
            COLLAPSED_LENGTH
        } else {
            NSVariableStatusItemLength
        });
        if let Some(button) = self.separator.button(mtm) {
            // A collapsed separator is a blank spacer; hide its glyph.
            button.setImagePosition(if collapsed {
                NSCellImagePosition::NoImage
            } else {
                NSCellImagePosition::ImageOnly
            });
        }
    }

    /// The icon's button, used to anchor menus and panels.
    #[must_use]
    pub fn icon_button(&self, mtm: MainThreadMarker) -> Option<Retained<NSStatusBarButton>> {
        self.icon.button(mtm)
    }
}

fn set_symbol(button: &NSStatusBarButton, symbol: &str, description: &str) {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(symbol),
        Some(&NSString::from_str(description)),
    );
    if let Some(image) = image {
        image.setTemplate(true);
        button.setImage(Some(&image));
    } else {
        tracing::warn!(symbol, "SF Symbol missing; using text");
        button.setTitle(&NSString::from_str(description));
    }
}

fn left_edge(item: &NSStatusItem, mtm: MainThreadMarker) -> Option<f64> {
    let window = item.button(mtm)?.window()?;
    if !window.isVisible() {
        return None;
    }
    Some(window.frame().origin.x)
}
