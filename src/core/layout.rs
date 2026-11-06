//! Menu bar items and which section each one is in.
//!
//! The window server reports every status item as a small window on the status layer. This
//! module filters those windows, orders them left to right and assigns each to a section by
//! comparing its position with Pinscher's dividers:
//!
//! ```text
//! [always hidden ...] |always-hidden divider| [hidden ...] |separator| [visible ...] (Pinscher)
//! ```

use crate::core::config::{SavedItem, Section};

/// Window layer of menu bar status items (`kCGStatusWindowLevel`).
pub const STATUS_ITEM_LAYER: i32 = 25;

/// A rectangle in global screen points (origin top-left, as the window server reports it).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// A window as reported by the window server, already converted from macOS types.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawWindow {
    pub id: u32,
    pub layer: i32,
    pub owner_pid: i32,
    /// Owner's bundle identifier, or its process name when it has none.
    pub owner: String,
    /// Window title; empty without Screen Recording permission.
    pub title: String,
    pub frame: Rect,
}

/// A status item that belongs to another app.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MenuBarItem {
    pub id: u32,
    pub owner_pid: i32,
    pub owner: String,
    pub title: String,
    pub frame: Rect,
}

/// Where Pinscher's dividers are on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dividers {
    pub separator_x: f64,
    pub always_hidden_x: Option<f64>,
}

/// Items grouped by section, each group ordered left to right.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sections {
    pub visible: Vec<MenuBarItem>,
    pub hidden: Vec<MenuBarItem>,
    pub always_hidden: Vec<MenuBarItem>,
}

/// Keeps other apps' status items, ordered left to right.
#[must_use]
pub fn menu_bar_items(
    windows: impl IntoIterator<Item = RawWindow>,
    own_pid: i32,
) -> Vec<MenuBarItem> {
    let mut items: Vec<MenuBarItem> = windows
        .into_iter()
        .filter(|window| {
            window.layer == STATUS_ITEM_LAYER
                && window.owner_pid != own_pid
                && window.frame.width > 0.0
                && window.frame.height > 0.0
        })
        .map(|window| MenuBarItem {
            id: window.id,
            owner_pid: window.owner_pid,
            owner: window.owner,
            title: window.title,
            frame: window.frame,
        })
        .collect();
    items.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    items
}

/// The section an item at horizontal position `x` is in.
#[must_use]
pub fn section_at(x: f64, dividers: Dividers) -> Section {
    if x > dividers.separator_x {
        Section::Visible
    } else if dividers
        .always_hidden_x
        .is_some_and(|always_hidden_x| x < always_hidden_x)
    {
        Section::AlwaysHidden
    } else {
        Section::Hidden
    }
}

/// Splits `items` into sections.
#[must_use]
pub fn group(items: Vec<MenuBarItem>, dividers: Dividers) -> Sections {
    let mut sections = Sections::default();
    for item in items {
        match section_at(item.frame.x, dividers) {
            Section::Visible => sections.visible.push(item),
            Section::Hidden => sections.hidden.push(item),
            Section::AlwaysHidden => sections.always_hidden.push(item),
        }
    }
    sections
}

/// The section the user saved for `item`, matched by owner and title.
#[must_use]
pub fn saved_section(item: &MenuBarItem, saved: &[SavedItem]) -> Option<Section> {
    saved
        .iter()
        .find(|entry| entry.owner == item.owner && entry.title == item.title)
        .map(|entry| entry.section)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN_PID: i32 = 42;

    fn window(id: u32, owner: &str, x: f64) -> RawWindow {
        RawWindow {
            id,
            layer: STATUS_ITEM_LAYER,
            owner_pid: i32::try_from(id).unwrap() + 100,
            owner: owner.into(),
            title: format!("item {id}"),
            frame: Rect {
                x,
                y: 0.0,
                width: 30.0,
                height: 24.0,
            },
        }
    }

    #[test]
    fn keeps_only_other_apps_status_items_in_order() {
        let mut own = window(1, "pinscher", 500.0);
        own.owner_pid = OWN_PID;
        let mut normal_window = window(2, "app.window", 100.0);
        normal_window.layer = 0;
        let mut empty = window(3, "app.empty", 200.0);
        empty.frame.width = 0.0;
        let windows = vec![
            window(5, "app.b", 900.0),
            own,
            normal_window,
            empty,
            window(4, "app.a", 300.0),
        ];
        let items = menu_bar_items(windows, OWN_PID);
        let ids: Vec<u32> = items.iter().map(|item| item.id).collect();
        assert_eq!(ids, [4, 5]);
    }

    #[test]
    fn assigns_sections_by_position() {
        let dividers = Dividers {
            separator_x: 600.0,
            always_hidden_x: Some(300.0),
        };
        assert_eq!(section_at(700.0, dividers), Section::Visible);
        assert_eq!(section_at(400.0, dividers), Section::Hidden);
        assert_eq!(section_at(100.0, dividers), Section::AlwaysHidden);
    }

    #[test]
    fn without_always_hidden_divider_everything_left_is_hidden() {
        let dividers = Dividers {
            separator_x: 600.0,
            always_hidden_x: None,
        };
        assert_eq!(section_at(-9000.0, dividers), Section::Hidden);
    }

    #[test]
    fn groups_items() {
        let items = menu_bar_items(
            vec![
                window(1, "a", 100.0),
                window(2, "b", 400.0),
                window(3, "c", 700.0),
            ],
            OWN_PID,
        );
        let sections = group(
            items,
            Dividers {
                separator_x: 600.0,
                always_hidden_x: Some(300.0),
            },
        );
        assert_eq!(sections.always_hidden[0].owner, "a");
        assert_eq!(sections.hidden[0].owner, "b");
        assert_eq!(sections.visible[0].owner, "c");
    }

    #[test]
    fn matches_saved_items() {
        let item = &menu_bar_items(vec![window(7, "com.example.app", 10.0)], OWN_PID)[0];
        let saved = [SavedItem {
            owner: "com.example.app".into(),
            title: "item 7".into(),
            section: Section::AlwaysHidden,
        }];
        assert_eq!(saved_section(item, &saved), Some(Section::AlwaysHidden));
    }

    #[test]
    fn ignores_missing() {
        let item = &menu_bar_items(vec![window(7, "com.example.app", 10.0)], OWN_PID)[0];
        let saved = [SavedItem {
            owner: "com.example.app".into(),
            title: "another title".into(),
            section: Section::Hidden,
        }];
        assert_eq!(saved_section(item, &saved), None);
    }

    #[test]
    fn rect_center_and_contains() {
        let rect = Rect {
            x: 10.0,
            y: 0.0,
            width: 20.0,
            height: 24.0,
        };
        assert_eq!(rect.center(), (20.0, 12.0));
        assert!(rect.contains(10.0, 24.0));
        assert!(!rect.contains(31.0, 5.0));
    }
}
