//! Validated command-drag plans in Accessibility's global, top-left screen coordinates.

use crate::core::config::Section;
use crate::core::layout::{MenuBarItem, Rect};

/// A point from a validated drag plan. It cannot be constructed by a platform adapter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    #[must_use]
    pub fn x(self) -> f64 {
        self.x
    }
    #[must_use]
    pub fn y(self) -> f64 {
        self.y
    }
}

/// Full divider frames, measured while the dividers are thin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DividerFrames {
    pub separator: Rect,
    pub always_hidden: Option<Rect>,
}

/// A drag whose endpoints stay in one menu bar, clear of Pinscher's icon.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    from: Point,
    to: Point,
    item: MenuBarItem,
}

impl Plan {
    /// Identity and frame to revalidate immediately before posting events.
    #[must_use]
    pub fn item(&self) -> &MenuBarItem {
        &self.item
    }

    #[must_use]
    pub fn from(&self) -> Point {
        self.from
    }
    #[must_use]
    pub fn to(&self) -> Point {
        self.to
    }
}

/// Why moving this item is unsafe or impossible with the current layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("invalid menu bar geometry")]
    InvalidGeometry,
    #[error("item is outside the menu bar")]
    OutsideMenuBar,
    #[error("dividers are not in a safe order")]
    InvalidDividers,
    #[error("the always-hidden divider is unavailable")]
    MissingAlwaysHidden,
    #[error("there is no room in the destination section")]
    NoRoom,
    #[error("the item is already at the destination")]
    ZeroDistance,
    #[error("a drag endpoint is over the Pinscher icon")]
    ProtectedIcon,
}

/// Plans a move to `section` using the divider as an insertion target.
/// `menu_bar` must be the visible menu bar of the item's screen, and all dividers must be thin.
///
/// # Errors
/// Refuses invalid or offscreen geometry, unsafe divider order and endpoints on Pinscher's icon.
pub fn plan_for_section(
    item: &MenuBarItem,
    section: Section,
    dividers: DividerFrames,
    menu_bar: Rect,
    icon: Rect,
) -> Result<Plan, PlanError> {
    if !valid_rect(menu_bar) || !valid_rect(item.frame) || !valid_rect(icon) {
        return Err(PlanError::InvalidGeometry);
    }
    if !contains_rect(menu_bar, item.frame) || !contains_rect(menu_bar, icon) {
        return Err(PlanError::OutsideMenuBar);
    }
    let separator = dividers.separator.x;
    if !valid_rect(dividers.separator)
        || !contains_rect(menu_bar, dividers.separator)
        || separator <= menu_bar.x
        || separator + dividers.separator.width > icon.x
    {
        return Err(PlanError::InvalidDividers);
    }
    if let Some(always) = dividers.always_hidden
        && (!valid_rect(always)
            || !contains_rect(menu_bar, always)
            || always.x <= menu_bar.x
            || always.x + always.width > separator)
    {
        return Err(PlanError::InvalidDividers);
    }
    let (x, y) = item.frame.center();
    let from = Point { x, y };
    // AX and AppKit include different edge insets. Adjacent frames may overlap slightly;
    // the actual mouse-down point must be outside the owned dividers, not the whole frame.
    if dividers.separator.contains(from.x, from.y)
        || dividers
            .always_hidden
            .is_some_and(|frame| frame.contains(from.x, from.y))
    {
        return Err(PlanError::InvalidDividers);
    }
    // Drop into the divider's appropriate half. AppKit creates the insertion space during
    // the drag, so requiring an empty item-width gap would reject the default adjacent layout.
    let x = match section {
        Section::Visible => dividers.separator.x + dividers.separator.width * 0.75,
        Section::Hidden => dividers.separator.x + dividers.separator.width * 0.25,
        Section::AlwaysHidden => {
            let divider = dividers
                .always_hidden
                .ok_or(PlanError::MissingAlwaysHidden)?;
            divider.x + divider.width * 0.25
        }
    };
    let to = Point { x, y };
    if !menu_bar.contains(to.x, to.y) {
        return Err(PlanError::NoRoom);
    }
    if icon.contains(from.x, from.y) || icon.contains(to.x, to.y) {
        return Err(PlanError::ProtectedIcon);
    }
    if (from.x - to.x).abs() <= f64::EPSILON {
        return Err(PlanError::ZeroDistance);
    }
    Ok(Plan {
        from,
        to,
        item: item.clone(),
    })
}

fn valid_rect(rect: Rect) -> bool {
    [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.x + rect.width,
        rect.y + rect.height,
    ]
    .iter()
    .all(|number| number.is_finite())
        && rect.width > 0.0
        && rect.height > 0.0
}

fn contains_rect(outer: Rect, inner: Rect) -> bool {
    outer.contains(inner.x, inner.y)
        && outer.contains(inner.x + inner.width, inner.y + inner.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1440.0,
        height: 24.0,
    };
    const ICON: Rect = Rect {
        x: 1200.0,
        y: 0.0,
        width: 24.0,
        height: 24.0,
    };
    const DIVIDERS: DividerFrames = DividerFrames {
        separator: Rect {
            x: 1000.0,
            y: 0.0,
            width: 31.5,
            height: 24.0,
        },
        always_hidden: Some(Rect {
            x: 600.0,
            y: 0.0,
            width: 31.5,
            height: 24.0,
        }),
    };

    fn item(x: f64) -> MenuBarItem {
        MenuBarItem {
            frame: Rect {
                x,
                y: 0.0,
                width: 20.0,
                height: 24.0,
            },
            ..MenuBarItem::default()
        }
    }

    #[test]
    fn plans_each_section_at_divider_insertion_side() {
        let from = item(800.0);
        for (section, expected) in [
            (Section::Visible, 1023.625),
            (Section::Hidden, 1007.875),
            (Section::AlwaysHidden, 607.875),
        ] {
            let plan = plan_for_section(&from, section, DIVIDERS, BAR, ICON).unwrap();
            assert_eq!(plan.from(), Point { x: 810.0, y: 12.0 });
            assert_eq!(
                plan.to(),
                Point {
                    x: expected,
                    y: 12.0
                }
            );
        }
    }

    #[test]
    fn rejects_points_outside_menu_bar() {
        assert_eq!(
            plan_for_section(&item(-100.0), Section::Visible, DIVIDERS, BAR, ICON),
            Err(PlanError::OutsideMenuBar)
        );
        let mut below = item(800.0);
        below.frame.y = 30.0;
        assert_eq!(
            plan_for_section(&below, Section::Visible, DIVIDERS, BAR, ICON),
            Err(PlanError::OutsideMenuBar)
        );
    }

    #[test]
    fn rejects_source_overlapping_divider() {
        assert_eq!(
            plan_for_section(&item(1010.0), Section::Visible, DIVIDERS, BAR, ICON),
            Err(PlanError::InvalidDividers)
        );
    }

    #[test]
    fn always_hidden_requires_ordered_visible_divider() {
        let missing = DividerFrames {
            always_hidden: None,
            ..DIVIDERS
        };
        assert_eq!(
            plan_for_section(&item(800.0), Section::AlwaysHidden, missing, BAR, ICON),
            Err(PlanError::MissingAlwaysHidden)
        );
        for x in [-1.0, 1100.0, f64::NAN] {
            let bad = DividerFrames {
                always_hidden: Some(Rect {
                    x,
                    ..DIVIDERS.always_hidden.unwrap()
                }),
                ..DIVIDERS
            };
            assert_eq!(
                plan_for_section(&item(800.0), Section::Hidden, bad, BAR, ICON),
                Err(PlanError::InvalidDividers)
            );
        }
    }

    #[test]
    fn rejects_invalid_geometry() {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut bad_item = item(800.0);
            bad_item.frame.x = n;
            assert_eq!(
                plan_for_section(&bad_item, Section::Visible, DIVIDERS, BAR, ICON),
                Err(PlanError::InvalidGeometry)
            );
            let bad_bar = Rect { width: n, ..BAR };
            assert_eq!(
                plan_for_section(&item(800.0), Section::Visible, DIVIDERS, bad_bar, ICON),
                Err(PlanError::InvalidGeometry)
            );
        }
        for width in [0.0, -1.0] {
            let bad_item = MenuBarItem {
                frame: Rect {
                    width,
                    ..item(800.0).frame
                },
                ..item(800.0)
            };
            assert_eq!(
                plan_for_section(&bad_item, Section::Visible, DIVIDERS, BAR, ICON),
                Err(PlanError::InvalidGeometry)
            );
        }
    }

    #[test]
    fn protects_icon_endpoints_but_allows_dragging_across_it() {
        assert_eq!(
            plan_for_section(&item(1200.0), Section::Hidden, DIVIDERS, BAR, ICON),
            Err(PlanError::ProtectedIcon)
        );
        assert!(plan_for_section(&item(1300.0), Section::Hidden, DIVIDERS, BAR, ICON).is_ok());
    }

    #[test]
    fn supports_screens_left_of_and_above_primary() {
        let bar = Rect {
            x: -1440.0,
            y: -900.0,
            ..BAR
        };
        let icon = Rect {
            x: -240.0,
            y: -900.0,
            ..ICON
        };
        let dividers = DividerFrames {
            separator: Rect {
                x: -440.0,
                y: -900.0,
                ..DIVIDERS.separator
            },
            always_hidden: Some(Rect {
                x: -840.0,
                y: -900.0,
                ..DIVIDERS.always_hidden.unwrap()
            }),
        };
        let item = MenuBarItem {
            frame: Rect {
                x: -640.0,
                y: -900.0,
                ..item(0.0).frame
            },
            ..item(0.0)
        };
        let plan = plan_for_section(&item, Section::Visible, dividers, bar, icon).unwrap();
        assert_eq!(
            plan.to(),
            Point {
                x: -416.375,
                y: -888.0
            }
        );
    }
    #[test]
    fn rejects_offscreen_destination_and_dividers() {
        let near_edge = DividerFrames {
            always_hidden: Some(Rect {
                x: 15.0,
                ..DIVIDERS.always_hidden.unwrap()
            }),
            ..DIVIDERS
        };
        assert!(
            plan_for_section(&item(800.0), Section::AlwaysHidden, near_edge, BAR, ICON).is_ok()
        );
        for x in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1200.0, 1500.0] {
            let bad = DividerFrames {
                separator: Rect {
                    x,
                    ..DIVIDERS.separator
                },
                ..DIVIDERS
            };
            assert_eq!(
                plan_for_section(&item(800.0), Section::Visible, bad, BAR, ICON),
                Err(PlanError::InvalidDividers)
            );
        }
    }

    #[test]
    fn requires_entire_item_in_bar_and_finite_rectangle_edges() {
        assert_eq!(
            plan_for_section(&item(1430.0), Section::Hidden, DIVIDERS, BAR, ICON),
            Err(PlanError::OutsideMenuBar)
        );
        let overflow = Rect {
            x: f64::MAX,
            width: f64::MAX,
            ..BAR
        };
        assert_eq!(
            plan_for_section(&item(800.0), Section::Visible, DIVIDERS, overflow, ICON),
            Err(PlanError::InvalidGeometry)
        );
        for frame in [
            Rect {
                y: f64::NAN,
                ..item(800.0).frame
            },
            Rect {
                width: f64::INFINITY,
                ..item(800.0).frame
            },
            Rect {
                height: 0.0,
                ..item(800.0).frame
            },
        ] {
            let bad = MenuBarItem {
                frame,
                ..item(800.0)
            };
            assert_eq!(
                plan_for_section(&bad, Section::Visible, DIVIDERS, BAR, ICON),
                Err(PlanError::InvalidGeometry)
            );
        }
    }
    #[test]
    fn adjacent_dividers_do_not_overlap() {
        let dividers = DividerFrames {
            separator: Rect {
                x: 1168.0,
                width: 32.0,
                ..DIVIDERS.separator
            },
            always_hidden: Some(Rect {
                x: 1136.0,
                width: 32.0,
                ..DIVIDERS.separator
            }),
        };
        assert!(plan_for_section(&item(800.0), Section::AlwaysHidden, dividers, BAR, ICON).is_ok());
    }

    #[test]
    fn visible_destination_is_in_right_half_of_full_separator() {
        let plan = plan_for_section(&item(800.0), Section::Visible, DIVIDERS, BAR, ICON).unwrap();
        assert!(plan.to().x() > DIVIDERS.separator.x + DIVIDERS.separator.width / 2.0);
        assert!(plan.to().x() < DIVIDERS.separator.x + DIVIDERS.separator.width);
    }
    #[test]
    fn plans_visible_insertion_when_separator_touches_icon() {
        let dividers = DividerFrames {
            separator: Rect {
                x: 1168.0,
                width: 32.0,
                ..DIVIDERS.separator
            },
            ..DIVIDERS
        };
        let plan = plan_for_section(&item(800.0), Section::Visible, dividers, BAR, ICON).unwrap();
        assert!(plan.to().x() < ICON.x);
        assert!(plan.to().x() > dividers.separator.x + dividers.separator.width / 2.0);
    }
    #[test]
    fn permits_adjacent_ax_item_with_fractional_frame_overlap() {
        let item = MenuBarItem {
            frame: Rect {
                x: 1461.0,
                y: 0.0,
                width: 104.0,
                height: 24.0,
            },
            ..MenuBarItem::default()
        };
        let dividers = DividerFrames {
            separator: Rect {
                x: 1431.0,
                y: 0.0,
                width: 31.5,
                height: 24.0,
            },
            always_hidden: None,
        };
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 2056.0,
            height: 24.0,
        };
        let icon = Rect {
            x: 1601.0,
            y: 0.0,
            width: 38.5,
            height: 24.0,
        };
        assert!(plan_for_section(&item, Section::Hidden, dividers, bounds, icon).is_ok());
    }
}
