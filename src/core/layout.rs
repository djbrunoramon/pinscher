//! Menu bar items and which section each one is in.
//!
//! Accessibility reports status items with their owner, title and frame. This module filters
//! those items, orders them left to right and assigns each to a section by
//! comparing its position with Pinscher's dividers:
//!
//! ```text
//! [always hidden ...] |always-hidden divider| [hidden ...] |separator| [visible ...] (Pinscher)
//! ```

use crate::core::config::{Config, MAX_FILE_BYTES, MAX_ITEMS, MAX_TEXT_LEN, SavedItem, Section};

/// A rectangle in global screen points (origin top-left, as Accessibility reports it).
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

/// A status item reported by Accessibility, already converted from macOS types.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawItem {
    pub owner_pid: i32,
    /// Owner's bundle identifier, or its process name when it has none.
    pub owner: String,
    /// Accessible title, description or identifier; empty when none is available.
    pub title: String,
    pub frame: Rect,
}

/// A status item that belongs to another app.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MenuBarItem {
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
pub fn menu_bar_items(raw: impl IntoIterator<Item = RawItem>, own_pid: i32) -> Vec<MenuBarItem> {
    let mut items: Vec<MenuBarItem> = raw
        .into_iter()
        .filter(|item| {
            item.owner_pid != own_pid && item.frame.width > 0.0 && item.frame.height > 0.0
        })
        .map(|item| MenuBarItem {
            owner_pid: item.owner_pid,
            owner: item.owner,
            title: item.title,
            frame: item.frame,
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

/// Resolves a snapshot identity only when exactly one live item still matches it.
#[must_use]
pub fn unique_item<'a>(
    items: &'a [MenuBarItem],
    expected: &MenuBarItem,
) -> Option<&'a MenuBarItem> {
    let mut matches = items.iter().filter(|item| {
        item.owner_pid == expected.owner_pid
            && item.owner == expected.owner
            && item.title == expected.title
    });
    let item = matches.next()?;
    matches.next().is_none().then_some(item)
}

/// Remembers a verified move only if it fits the same bounds used when loading preferences.
/// A rejected update leaves all existing choices intact.
pub fn remember_section(config: &mut Config, item: &MenuBarItem, section: Section) -> bool {
    if item.owner.is_empty()
        || item.owner.chars().count() > MAX_TEXT_LEN
        || item.title.chars().count() > MAX_TEXT_LEN
    {
        return false;
    }
    let mut next = config.clone();
    next.items
        .retain(|entry| entry.owner != item.owner || entry.title != item.title);
    if next.items.len() >= MAX_ITEMS {
        return false;
    }
    next.items.push(SavedItem {
        owner: item.owner.clone(),
        title: item.title.clone(),
        section,
    });
    if !toml::to_string_pretty(&next)
        .is_ok_and(|text| u64::try_from(text.len()).unwrap_or(u64::MAX) <= MAX_FILE_BYTES)
    {
        return false;
    }
    *config = next;
    true
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

    fn item(id: i32, owner: &str, x: f64) -> RawItem {
        RawItem {
            owner_pid: id + 100,
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
    fn keeps_only_other_apps_items_in_order() {
        let mut own = item(1, "pinscher", 500.0);
        own.owner_pid = OWN_PID;
        let ordinary = item(2, "app.ordinary", 100.0);
        let mut empty = item(3, "app.empty", 200.0);
        empty.frame.width = 0.0;
        let mut flat = item(6, "app.flat", 250.0);
        flat.frame.height = 0.0;
        let raw = vec![
            item(5, "app.b", 900.0),
            own,
            ordinary,
            empty,
            flat,
            item(4, "app.a", 300.0),
        ];
        let items = menu_bar_items(raw, OWN_PID);
        let owners: Vec<&str> = items.iter().map(|item| item.owner.as_str()).collect();
        assert_eq!(owners, ["app.ordinary", "app.a", "app.b"]);
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
                item(1, "a", 100.0),
                item(2, "b", 400.0),
                item(3, "c", 700.0),
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
        let item = &menu_bar_items(vec![item(7, "com.example.app", 10.0)], OWN_PID)[0];
        let saved = [SavedItem {
            owner: "com.example.app".into(),
            title: "item 7".into(),
            section: Section::AlwaysHidden,
        }];
        assert_eq!(saved_section(item, &saved), Some(Section::AlwaysHidden));
    }

    #[test]
    fn ignores_missing() {
        let item = &menu_bar_items(vec![item(7, "com.example.app", 10.0)], OWN_PID)[0];
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
    #[test]
    fn refuses_ambiguous_or_restarted_item_identity() {
        let expected = menu_bar_items(vec![item(7, "app", 10.0)], OWN_PID).remove(0);
        assert!(unique_item(&[expected.clone(), expected.clone()], &expected).is_none());
        let mut restarted = expected.clone();
        restarted.owner_pid += 1;
        assert!(unique_item(&[restarted], &expected).is_none());
        let mut moved = expected.clone();
        moved.frame.x += 100.0;
        assert_eq!(
            unique_item(std::slice::from_ref(&moved), &expected),
            Some(&moved)
        );
    }
    #[test]
    fn remembers_verified_section_without_losing_other_choices() {
        let mut config = Config::default();
        let item = menu_bar_items(vec![item(7, "app", 10.0)], OWN_PID).remove(0);
        assert!(remember_section(&mut config, &item, Section::Hidden));
        assert!(remember_section(&mut config, &item, Section::Visible));
        assert_eq!(config.items.len(), 1);
        assert_eq!(saved_section(&item, &config.items), Some(Section::Visible));
        let before = config.clone();
        let oversized = MenuBarItem {
            title: "x".repeat(MAX_TEXT_LEN + 1),
            ..item
        };
        assert!(!remember_section(
            &mut config,
            &oversized,
            Section::AlwaysHidden
        ));
        assert_eq!(config, before);
    }
}
