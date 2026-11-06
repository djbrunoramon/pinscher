//! The Settings window, built in code with `AppKit` controls.
//!
//! The window only shows state and forwards control actions to its target (the app delegate);
//! every decision and every write happens in the delegate and `core`.

use crate::core::l10n::{Lang, Text, text};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSButton, NSColor, NSControlStateValueOff,
    NSControlStateValueOn, NSFont, NSLayoutAttribute, NSStackView, NSStepper, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};

/// Everything the window shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SettingsState {
    /// Display form of the shortcut (e.g. `⌥⌘P`), or `None` when disabled.
    pub shortcut: Option<String>,
    pub recording_shortcut: bool,
    pub shortcut_unavailable: bool,
    pub auto_hide: bool,
    pub auto_hide_delay: u32,
    pub launch_at_login: bool,
    pub launch_at_login_enabled: bool,
    pub launch_at_login_note: Option<Text>,
    pub always_hidden: bool,
    pub secondary_bar: bool,
    pub accessibility_missing: bool,
}

/// Selectors the controls send to the target.
#[derive(Debug, Clone, Copy)]
pub struct SettingsActions {
    pub record_shortcut: Sel,
    pub clear_shortcut: Sel,
    pub auto_hide: Sel,
    pub auto_hide_delay: Sel,
    pub launch_at_login: Sel,
    pub always_hidden: Sel,
    pub secondary_bar: Sel,
    pub open_accessibility: Sel,
}

/// The Settings window and the controls whose state changes.
#[derive(Debug)]
pub struct SettingsWindow {
    lang: Lang,
    window: Retained<NSWindow>,
    shortcut: Retained<NSTextField>,
    record: Retained<NSButton>,
    shortcut_warning: Retained<NSTextField>,
    auto_hide: Retained<NSButton>,
    delay: Retained<NSStepper>,
    delay_value: Retained<NSTextField>,
    launch_at_login: Retained<NSButton>,
    launch_at_login_note: Retained<NSTextField>,
    always_hidden: Retained<NSButton>,
    secondary_bar: Retained<NSButton>,
    accessibility_note: Retained<NSTextField>,
    open_accessibility: Retained<NSButton>,
}

/// Bounds of the auto-hide delay stepper, matching `core::config::AUTO_HIDE_DELAY_RANGE`.
const DELAY_MIN: f64 = 1.0;
const DELAY_MAX: f64 = 60.0;

impl SettingsWindow {
    /// Builds the window and every control, top to bottom.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "a flat list of controls reads best in one place"
    )]
    pub fn new(
        mtm: MainThreadMarker,
        lang: Lang,
        target: &AnyObject,
        delegate: &ProtocolObject<dyn NSWindowDelegate>,
        actions: SettingsActions,
    ) -> Self {
        let t = |key| text(lang, key);

        let shortcut = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        shortcut.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(13.0, 0.0)));
        let record = button(
            mtm,
            t(Text::SettingsShortcutRecord),
            target,
            actions.record_shortcut,
        );
        let clear = button(
            mtm,
            t(Text::SettingsShortcutClear),
            target,
            actions.clear_shortcut,
        );
        let shortcut_row = row(
            mtm,
            &[
                &label(mtm, t(Text::SettingsShortcut)),
                &shortcut,
                &record,
                &clear,
            ],
        );
        let shortcut_warning = note(mtm, t(Text::WarningShortcutUnavailable));

        let auto_hide = checkbox(mtm, t(Text::SettingsAutoHide), target, actions.auto_hide);
        let delay = NSStepper::new(mtm);
        delay.setMinValue(DELAY_MIN);
        delay.setMaxValue(DELAY_MAX);
        delay.setIncrement(1.0);
        delay.setValueWraps(false);
        set_action(&delay, target, actions.auto_hide_delay);
        let delay_value = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        let auto_hide_row = row(
            mtm,
            &[
                &auto_hide,
                &delay_value,
                &delay,
                &label(mtm, t(Text::SettingsAutoHideSeconds)),
            ],
        );

        let launch_at_login = checkbox(
            mtm,
            t(Text::MenuLaunchAtLogin),
            target,
            actions.launch_at_login,
        );
        let launch_at_login_note = note(mtm, "");

        let always_hidden = checkbox(
            mtm,
            t(Text::SettingsAlwaysHidden),
            target,
            actions.always_hidden,
        );
        let secondary_bar = checkbox(
            mtm,
            t(Text::SettingsSecondaryBar),
            target,
            actions.secondary_bar,
        );
        let accessibility_note = note(mtm, t(Text::SettingsAccessibilityMissing));
        let open_accessibility = button(
            mtm,
            t(Text::SettingsOpenSystemSettings),
            target,
            actions.open_accessibility,
        );

        let content = column(
            mtm,
            &[
                &heading(mtm, t(Text::SettingsGeneral)),
                &shortcut_row,
                &shortcut_warning,
                &auto_hide_row,
                &launch_at_login,
                &launch_at_login_note,
                &heading(mtm, t(Text::SettingsAdvanced)),
                &always_hidden,
                &secondary_bar,
                &accessibility_note,
                &open_accessibility,
            ],
        );

        let window = window(mtm, t(Text::SettingsTitle));
        window.setContentView(Some(&content));
        window.setDelegate(Some(delegate));

        Self {
            lang,
            window,
            shortcut,
            record,
            shortcut_warning,
            auto_hide,
            delay,
            delay_value,
            launch_at_login,
            launch_at_login_note,
            always_hidden,
            secondary_bar,
            accessibility_note,
            open_accessibility,
        }
    }

    /// Brings the window to the front, activating the (accessory) app so it can take focus.
    pub fn show(&self, mtm: MainThreadMarker) {
        NSApplication::sharedApplication(mtm).activate();
        if !self.window.isVisible() {
            self.window.center();
        }
        self.window.makeKeyAndOrderFront(None);
    }

    /// Whether the window is the key window (it receives the shortcut recorder's key presses).
    #[must_use]
    pub fn is_key(&self) -> bool {
        self.window.isKeyWindow()
    }

    /// Updates every control to `state`.
    pub fn refresh(&self, state: &SettingsState) {
        let t = |key| text(self.lang, key);
        let shortcut = if state.recording_shortcut {
            t(Text::SettingsShortcutRecording).to_owned()
        } else {
            state
                .shortcut
                .clone()
                .unwrap_or_else(|| t(Text::SettingsShortcutNone).to_owned())
        };
        self.shortcut.setStringValue(&NSString::from_str(&shortcut));
        self.record.setEnabled(!state.recording_shortcut);
        self.shortcut_warning.setHidden(!state.shortcut_unavailable);

        set_on(&self.auto_hide, state.auto_hide);
        self.delay
            .setIntegerValue(isize::try_from(state.auto_hide_delay).unwrap_or(5));
        self.delay.setEnabled(state.auto_hide);
        self.delay_value
            .setStringValue(&NSString::from_str(&state.auto_hide_delay.to_string()));

        set_on(&self.launch_at_login, state.launch_at_login);
        self.launch_at_login
            .setEnabled(state.launch_at_login_enabled);
        match state.launch_at_login_note {
            Some(key) => {
                self.launch_at_login_note
                    .setStringValue(&NSString::from_str(t(key)));
                self.launch_at_login_note.setHidden(false);
            }
            None => self.launch_at_login_note.setHidden(true),
        }

        set_on(&self.always_hidden, state.always_hidden);
        set_on(&self.secondary_bar, state.secondary_bar);
        self.accessibility_note
            .setHidden(!state.accessibility_missing);
        self.open_accessibility
            .setHidden(!state.accessibility_missing);
    }
}

/// Whether a checkbox `sender` is on.
#[must_use]
pub fn is_on(sender: &NSButton) -> bool {
    sender.state() == NSControlStateValueOn
}

fn window(mtm: MainThreadMarker, title: &str) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(520.0, 320.0));
    // SAFETY: standard designated initializer with a valid frame and style; the window is kept
    // alive by `SettingsWindow` and is not released on close (set right below).
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: we own the window through `Retained`, so AppKit must not release it on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(title));
    window
}

fn column(mtm: MainThreadMarker, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = NSStackView::stackViewWithViews(&NSArray::from_slice(views), mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(10.0);
    stack.setEdgeInsets(NSEdgeInsets {
        top: 20.0,
        left: 20.0,
        bottom: 20.0,
        right: 20.0,
    });
    stack
}

fn row(mtm: MainThreadMarker, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = NSStackView::stackViewWithViews(&NSArray::from_slice(views), mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(8.0);
    stack
}

fn label(mtm: MainThreadMarker, title: &str) -> Retained<NSTextField> {
    NSTextField::labelWithString(&NSString::from_str(title), mtm)
}

fn heading(mtm: MainThreadMarker, title: &str) -> Retained<NSTextField> {
    let field = label(mtm, title);
    field.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    field
}

fn note(mtm: MainThreadMarker, title: &str) -> Retained<NSTextField> {
    let field = NSTextField::wrappingLabelWithString(&NSString::from_str(title), mtm);
    field.setTextColor(Some(&NSColor::secondaryLabelColor()));
    field.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    field.setPreferredMaxLayoutWidth(480.0);
    field.setHidden(true);
    field
}

fn checkbox(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    // SAFETY: `target` (the app delegate) outlives the window and implements `action` with the
    // `-(void)x:(id)sender` signature.
    unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(action),
            mtm,
        )
    }
}

fn button(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    // SAFETY: as in `checkbox`.
    unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(action),
            mtm,
        )
    }
}

fn set_action(stepper: &NSStepper, target: &AnyObject, action: Sel) {
    // SAFETY: as in `checkbox`.
    unsafe {
        stepper.setTarget(Some(target));
        stepper.setAction(Some(action));
    }
}

fn set_on(button: &NSButton, on: bool) {
    button.setState(if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
}
