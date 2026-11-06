//! Layout tab: shows current sections and forwards explicit rearrangement requests.

use crate::core::config::Section;
use crate::core::l10n::{Lang, Text, text};
use crate::core::layout::{Dividers, MenuBarItem, section_at};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSFont, NSImageView, NSPopUpButton, NSRunningApplication, NSScrollView, NSTextField,
    NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
struct Snapshot {
    items: Vec<MenuBarItem>,
    dividers: Option<Dividers>,
    busy: bool,
}

#[derive(Debug)]
pub struct LayoutView {
    pub view: Retained<NSView>,
    scroll: Retained<NSScrollView>,
    note: Retained<NSTextField>,
    restore: Retained<NSButton>,
    lang: Lang,
    snapshot: RefCell<Option<Snapshot>>,
    controls: RefCell<Vec<(Retained<NSPopUpButton>, MenuBarItem)>>,
}

impl LayoutView {
    #[must_use]
    pub fn new(
        mtm: MainThreadMarker,
        lang: Lang,
        target: &AnyObject,
        refresh: Sel,
        restore: Sel,
    ) -> Self {
        let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 640.0, 440.0));
        let refresh = action_button(mtm, text(lang, Text::LayoutRefresh), target, refresh);
        refresh.setFrame(rect(12.0, 397.0, 230.0, 32.0));
        view.addSubview(&refresh);
        let restore = action_button(mtm, text(lang, Text::LayoutRestore), target, restore);
        restore.setFrame(rect(250.0, 397.0, 375.0, 32.0));
        view.addSubview(&restore);
        let note = NSTextField::wrappingLabelWithString(
            &NSString::from_str(text(lang, Text::LayoutInstructions)),
            mtm,
        );
        note.setFrame(rect(12.0, 340.0, 612.0, 54.0));
        view.addSubview(&note);
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(10.0, 10.0, 620.0, 325.0));
        scroll.setHasVerticalScroller(true);
        view.addSubview(&scroll);
        Self {
            view,
            scroll,
            note,
            restore,
            lang,
            snapshot: RefCell::new(None),
            controls: RefCell::new(Vec::new()),
        }
    }

    pub fn refresh(
        &self,
        items: &[MenuBarItem],
        dividers: Option<Dividers>,
        target: &AnyObject,
        action: Sel,
        note: Text,
        busy: bool,
    ) {
        let mtm = self.view.mtm();
        self.note
            .setStringValue(&NSString::from_str(text(self.lang, note)));
        self.restore.setEnabled(!busy && dividers.is_some());
        let next = Snapshot {
            items: items.to_vec(),
            dividers,
            busy,
        };
        if self.snapshot.borrow().as_ref() == Some(&next) {
            // A popup changes its value before the action runs. Restore the observed section
            // even when a refused move leaves the underlying snapshot unchanged.
            if let Some(dividers) = dividers {
                for (control, item) in self.controls.borrow().iter() {
                    control.selectItemAtIndex(section_index(section_at(item.frame.x, dividers)));
                }
            }
            return;
        }
        let previous_origin = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|_| self.scroll.contentView().bounds().origin);
        *self.snapshot.borrow_mut() = Some(next);
        self.controls.borrow_mut().clear();
        let count = u32::try_from(items.len().min(256)).unwrap_or(256);
        let height = (f64::from(count) * 36.0 + 120.0).max(320.0);
        let document = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 600.0, height));
        let mut y = height - 30.0;
        for (section, key) in [
            (Section::Visible, Text::SectionVisible),
            (Section::Hidden, Text::SectionHidden),
            (Section::AlwaysHidden, Text::SectionAlwaysHidden),
        ] {
            let heading =
                NSTextField::labelWithString(&NSString::from_str(text(self.lang, key)), mtm);
            heading.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
            heading.setFrame(rect(8.0, y, 580.0, 24.0));
            document.addSubview(&heading);
            y -= 30.0;
            for (index, item) in items.iter().take(256).enumerate().filter(|(_, item)| {
                dividers.is_some_and(|d| section_at(item.frame.x, d) == section)
            }) {
                self.add_identity(&document, item, y);
                let popup = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    rect(365.0, y - 3.0, 225.0, 30.0),
                    false,
                );
                for key in [
                    Text::SectionVisible,
                    Text::SectionHidden,
                    Text::SectionAlwaysHidden,
                ] {
                    popup.addItemWithTitle(&NSString::from_str(text(self.lang, key)));
                }
                popup.selectItemAtIndex(section_index(section));
                popup.setTag(isize::try_from(index).unwrap_or(-1));
                popup.setEnabled(!busy);
                if let Some(menu) = popup.menu() {
                    menu.setAutoenablesItems(false);
                }
                if dividers.is_some_and(|d| d.always_hidden_x.is_none())
                    && let Some(item) = popup.itemAtIndex(2)
                {
                    item.setEnabled(false);
                }
                // SAFETY: the app delegate outlives this tab and implements the action signature.
                unsafe {
                    popup.setTarget(Some(target));
                    popup.setAction(Some(action));
                }
                document.addSubview(&popup);
                self.controls.borrow_mut().push((popup, item.clone()));
                y -= 36.0;
            }
        }
        if items.is_empty() {
            let label = NSTextField::labelWithString(
                &NSString::from_str(text(self.lang, Text::LayoutEmpty)),
                mtm,
            );
            label.setFrame(rect(12.0, 130.0, 580.0, 32.0));
            document.addSubview(&label);
        }
        self.scroll.setDocumentView(Some(&document));
        document.scrollPoint(previous_origin.unwrap_or(NSPoint::new(0.0, height)));
    }
    /// Adds the app icon and the readable name of `item` at row `y`.
    fn add_identity(&self, document: &NSView, item: &MenuBarItem, y: f64) {
        let mtm = self.view.mtm();
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(item.owner_pid);
        if let Some(icon) = app.as_ref().and_then(|app| app.icon()) {
            let image = NSImageView::imageViewWithImage(&icon, mtm);
            image.setFrame(rect(12.0, y, 24.0, 24.0));
            document.addSubview(&image);
        }
        let title = display_name(
            item,
            app.as_ref()
                .and_then(|app| app.localizedName())
                .map(|name| name.to_string()),
            text(self.lang, Text::LayoutSystemItem),
        );
        let label = NSTextField::labelWithString(&NSString::from_str(&title), mtm);
        label.setFrame(rect(44.0, y, 313.0, 24.0));
        document.addSubview(&label);
    }

    /// Only current controls can issue moves; an old popup from a replaced snapshot is refused.
    #[must_use]
    pub fn item_for_sender(&self, sender: &NSPopUpButton) -> Option<MenuBarItem> {
        self.controls
            .borrow()
            .iter()
            .find(|(control, _)| {
                std::ptr::eq(std::ptr::from_ref(&**control), std::ptr::from_ref(sender))
            })
            .map(|(_, item)| item.clone())
    }
}

/// What to call an item: its own title, else its app's name, else a generic label for items
/// that the system's menu bar host publishes on behalf of other processes.
fn display_name(item: &MenuBarItem, app_name: Option<String>, system_label: &str) -> String {
    const HOSTS: [&str; 3] = ["MenuBarAgent", "SystemUIServer", "ControlCenter"];
    if !item.title.is_empty() {
        return item.title.clone();
    }
    match app_name {
        Some(name) if !HOSTS.contains(&name.as_str()) => name,
        _ => system_label.to_owned(),
    }
}

fn section_index(section: Section) -> isize {
    match section {
        Section::Visible => 0,
        Section::Hidden => 1,
        Section::AlwaysHidden => 2,
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

fn action_button(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    // SAFETY: target is the retained app delegate and action has -(void)x:(id)sender signature.
    unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(action),
            mtm,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str) -> MenuBarItem {
        MenuBarItem {
            title: title.into(),
            owner: "com.example.app".into(),
            ..MenuBarItem::default()
        }
    }

    #[test]
    fn prefers_the_items_own_title() {
        assert_eq!(
            display_name(&item("Wi-Fi"), Some("Control".into()), "System"),
            "Wi-Fi"
        );
    }

    #[test]
    fn falls_back_to_the_app_name() {
        assert_eq!(
            display_name(&item(""), Some("Shottr".into()), "System"),
            "Shottr"
        );
    }

    #[test]
    fn labels_items_hosted_by_the_system() {
        assert_eq!(
            display_name(&item(""), Some("MenuBarAgent".into()), "System"),
            "System"
        );
        assert_eq!(display_name(&item(""), None, "System"), "System");
    }
}
