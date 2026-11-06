//! The Pinscher icon and the separator, as `NSStatusItem`s.
//!
//! Holds no decisions: `core::bar` decides, this module only reads positions and applies state.

use crate::core::bar::{Divider, Layout, Visibility, collapsed_length};
use crate::core::l10n::{Lang, Text, text};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2_app_kit::{
    NSCellImagePosition, NSEventMask, NSImage, NSScreen, NSStatusBar, NSStatusBarButton,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::NSString;
use std::cell::RefCell;
use std::rc::Rc;

/// The status items Pinscher owns.
#[derive(Debug, Clone)]
pub struct StatusBar {
    icon: Retained<NSStatusItem>,
    separator: Retained<NSStatusItem>,
    always_hidden: Rc<RefCell<Option<Retained<NSStatusItem>>>>,
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

        Self {
            icon,
            separator,
            always_hidden: Rc::new(RefCell::new(None)),
        }
    }

    /// Current left edge of each item on screen, or `None` while either is not on screen.
    #[must_use]
    pub fn layout(&self, mtm: MainThreadMarker) -> Option<Layout> {
        Some(Layout {
            icon_x: left_edge(&self.icon, mtm)?,
            separator_x: left_edge(&self.separator, mtm)?,
        })
    }

    /// Width of a collapsed separator for the screen the icon is on.
    fn collapsed_length(&self, mtm: MainThreadMarker) -> f64 {
        let width = self
            .icon
            .button(mtm)
            .and_then(|button| button.window())
            .and_then(|window| window.screen())
            .map_or(0.0, |screen| screen.frame().size.width);
        collapsed_length(width)
    }

    /// Shows `visibility` by resizing the separator.
    pub fn apply(&self, mtm: MainThreadMarker, visibility: Visibility) {
        let collapsed = visibility == Visibility::Collapsed;
        self.separator.setLength(if collapsed {
            self.collapsed_length(mtm)
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

    /// Adds or removes the optional divider, preserving its autosaved position.
    pub fn set_always_hidden(&self, mtm: MainThreadMarker, lang: Lang, enabled: bool) {
        let mut slot = self.always_hidden.borrow_mut();
        if enabled && slot.is_none() {
            let item =
                NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            item.setAutosaveName(Some(&NSString::from_str("pinscher.always-hidden")));
            if let Some(button) = item.button(mtm) {
                set_symbol(
                    &button,
                    "line.diagonal",
                    text(lang, Text::AlwaysHiddenAccessibility),
                );
            }
            *slot = Some(item);
        } else if !enabled && let Some(item) = slot.take() {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
        }
    }

    #[must_use]
    pub fn always_hidden_x(&self, mtm: MainThreadMarker) -> Option<f64> {
        left_edge(self.always_hidden.borrow().as_ref()?, mtm)
    }

    /// Applies the core's order-checked divider state.
    pub fn apply_always_hidden(&self, mtm: MainThreadMarker, state: Divider) {
        let slot = self.always_hidden.borrow();
        let Some(item) = slot.as_ref() else {
            return;
        };
        let collapsed = state == Divider::Collapsed;
        let length = if collapsed {
            self.collapsed_length(mtm)
        } else {
            NSVariableStatusItemLength
        };
        if (item.length() - length).abs() > f64::EPSILON {
            item.setLength(length);
        }
        if let Some(button) = item.button(mtm) {
            button.setImagePosition(if collapsed {
                NSCellImagePosition::NoImage
            } else {
                NSCellImagePosition::ImageOnly
            });
        }
    }

    /// Menu bar and owned item frames in AX's top-left global coordinate space.
    #[must_use]
    pub fn geometry(
        &self,
        mtm: MainThreadMarker,
    ) -> Option<(
        crate::core::layout::Rect,
        crate::core::layout::Rect,
        crate::core::drag::DividerFrames,
    )> {
        let window = self.icon.button(mtm)?.window()?;
        let screen = window.screen()?;
        let primary_top = NSScreen::screens(mtm).firstObject()?.frame().max().y;
        let convert = |r: objc2_foundation::NSRect| crate::core::layout::Rect {
            x: r.origin.x,
            y: primary_top - r.max().y,
            width: r.size.width,
            height: r.size.height,
        };
        let icon = convert(window.frame());
        let screen_frame = screen.frame();
        let bounds = crate::core::layout::Rect {
            x: screen_frame.origin.x,
            y: primary_top - screen_frame.max().y,
            width: screen_frame.size.width,
            height: icon.height,
        };
        let separator = convert(self.separator.button(mtm)?.window()?.frame());
        let always_hidden = self.always_hidden.borrow().as_ref().and_then(|item| {
            item.button(mtm)?
                .window()
                .map(|window| convert(window.frame()))
        });
        Some((
            bounds,
            icon,
            crate::core::drag::DividerFrames {
                separator,
                always_hidden,
            },
        ))
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
