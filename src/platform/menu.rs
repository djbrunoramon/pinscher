//! The icon's menu. Rebuilt every time it opens so it always shows the current state.

use crate::core::l10n::{Lang, Text, text};
use crate::core::login::LoginMenuItem;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBarButton};
use objc2_foundation::{NSPoint, NSString};

/// What the menu should show right now.
#[derive(Debug, Default)]
pub struct MenuState {
    /// Problems to explain at the top of the menu.
    pub warnings: Vec<Text>,
    pub launch_at_login: Option<LoginMenuItem>,
}

/// Selectors the menu sends to its target.
#[derive(Debug, Clone, Copy)]
pub struct MenuActions {
    pub settings: Sel,
    pub about: Sel,
    pub toggle_login: Sel,
    pub reveal_all: Sel,
}

/// Builds the menu and pops it up under `button`.
pub fn show(
    mtm: MainThreadMarker,
    lang: Lang,
    state: &MenuState,
    target: &AnyObject,
    actions: MenuActions,
    button: &NSStatusBarButton,
) {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);

    for warning in &state.warnings {
        let item = add(&menu, mtm, text(lang, *warning), None, None);
        item.setEnabled(false);
    }
    if !state.warnings.is_empty() {
        menu.addItem(&NSMenuItem::separatorItem(mtm));
    }

    add(
        &menu,
        mtm,
        text(lang, Text::MenuRevealAll),
        Some(actions.reveal_all),
        Some(target),
    );
    let settings = add(
        &menu,
        mtm,
        text(lang, Text::MenuPreferences),
        Some(actions.settings),
        Some(target),
    );
    settings.setKeyEquivalent(&NSString::from_str(","));
    add(
        &menu,
        mtm,
        text(lang, Text::MenuAbout),
        Some(actions.about),
        Some(target),
    );
    if let Some(login) = state.launch_at_login {
        let item = add(
            &menu,
            mtm,
            text(lang, Text::MenuLaunchAtLogin),
            Some(actions.toggle_login),
            Some(target),
        );
        item.setState(if login.checked {
            objc2_app_kit::NSControlStateValueOn
        } else {
            objc2_app_kit::NSControlStateValueOff
        });
        if login.action.is_none() {
            item.setEnabled(false);
        }
        if let Some(note) = login.note {
            let note_item = add(&menu, mtm, text(lang, note), None, None);
            note_item.setEnabled(false);
        }
    }
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    // `terminate:` with no target travels the responder chain to NSApplication.
    let quit = add(
        &menu,
        mtm,
        text(lang, Text::MenuQuit),
        Some(sel!(terminate:)),
        None,
    );
    quit.setKeyEquivalent(&NSString::from_str("q"));

    let below = NSPoint::new(0.0, button.bounds().size.height + 5.0);
    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(button));
}

fn add(
    menu: &NSMenu,
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    target: Option<&AnyObject>,
) -> Retained<NSMenuItem> {
    // SAFETY: `action` is either `None` or a selector implemented by `target` (or by
    // NSApplication via the responder chain) with the `-(void)x:(id)sender` signature.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::new(),
        )
    };
    // SAFETY: `target` is the app delegate, which lives for the whole process.
    unsafe { item.setTarget(target) };
    menu.addItem(&item);
    item
}
