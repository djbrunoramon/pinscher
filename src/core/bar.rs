//! Whether hidden icons are shown, and when a change is allowed.
//!
//! The menu bar lays items out right to left: the Pinscher icon sits on the right and the
//! separator somewhere to its left. Collapsing grows the separator so everything to its left is
//! pushed off screen. If the separator were to the right of the icon, collapsing would push the
//! icon itself away and leave the user with no way back (AM-06), so that is refused.

use std::time::Duration;

/// What the menu bar currently shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Visibility {
    /// Every item is visible; the separator is a thin divider.
    #[default]
    Expanded,
    /// Items left of the separator are pushed off screen.
    Collapsed,
}

/// Horizontal screen position (left edge, in points) of the items Pinscher owns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub icon_x: f64,
    pub separator_x: f64,
}

/// Why a requested change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// The separator is not to the left of the icon, so collapsing would hide the icon.
    SeparatorRightOfIcon,
    /// Item positions are unknown (for example, the menu bar is not on screen).
    LayoutUnknown,
}

/// What the platform layer must do after a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show the given state.
    Apply(Visibility),
    /// Keep the current state and tell the user why.
    Blocked(Blocked),
    /// Nothing changes.
    None,
}

/// How the always-hidden divider should be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    /// A thin, visible divider: items left of it are shown.
    Thin,
    /// Grown so items left of it stay off screen even when the bar is expanded.
    Collapsed,
}

/// A one-shot collapse request for the platform timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapseTimer {
    pub generation: u64,
    pub after: Duration,
}

/// The show/hide state machine.
#[derive(Debug)]
pub struct Bar {
    visibility: Visibility,
    auto_hide: Option<Duration>,
    generation: u64,
    user_expanded: bool,
    revealing_all: bool,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            visibility: Visibility::Expanded,
            auto_hide: None,
            generation: 0,
            user_expanded: false,
            revealing_all: false,
        }
    }
}

impl Bar {
    /// A new bar always starts expanded so nothing is hidden by surprise.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Configures the one-shot delay used after a user expansion.
    pub fn set_auto_hide(&mut self, auto_hide: Option<Duration>) {
        self.auto_hide = auto_hide;
    }

    /// Returns the timer request for the current user expansion, if any.
    #[must_use]
    pub fn pending_collapse(&self) -> Option<CollapseTimer> {
        if self.visibility == Visibility::Expanded && self.user_expanded {
            self.auto_hide.map(|after| CollapseTimer {
                generation: self.generation,
                after,
            })
        } else {
            None
        }
    }

    /// Handles a timer callback, ignoring callbacks for older visibility generations.
    pub fn on_timer(&mut self, generation: u64, layout: Option<Layout>) -> Action {
        if generation != self.generation || self.visibility != Visibility::Expanded {
            return Action::None;
        }
        self.collapse(layout)
    }

    /// Expands and also shows the always-hidden section until the next collapse, so the user can
    /// see and ⌘-drag those items.
    pub fn reveal_all(&mut self) -> Action {
        self.revealing_all = true;
        self.visibility = Visibility::Expanded;
        self.user_expanded = true;
        self.generation = self.generation.wrapping_add(1);
        Action::Apply(Visibility::Expanded)
    }

    /// Whether the always-hidden section is currently revealed.
    #[must_use]
    pub fn revealing_all(&self) -> bool {
        self.revealing_all
    }

    /// How to draw the always-hidden divider at `always_hidden_x`.
    ///
    /// It only grows when it sits left of the separator, which sits left of the icon; otherwise
    /// growing it could push the separator or the Pinscher icon itself off screen (AM-06).
    #[must_use]
    pub fn always_hidden_divider(&self, layout: Option<Layout>, always_hidden_x: f64) -> Divider {
        let ordered = layout.is_some_and(|layout| {
            [always_hidden_x, layout.separator_x, layout.icon_x]
                .iter()
                .all(|x| x.is_finite())
                && always_hidden_x < layout.separator_x
                && layout.separator_x < layout.icon_x
        });
        if ordered && !self.revealing_all {
            Divider::Collapsed
        } else {
            Divider::Thin
        }
    }

    /// Flips between expanded and collapsed. `layout` is only needed to collapse.
    pub fn toggle(&mut self, layout: Option<Layout>) -> Action {
        match self.visibility {
            Visibility::Expanded => self.collapse(layout),
            Visibility::Collapsed => self.expand(),
        }
    }

    /// Shows every item.
    pub fn expand(&mut self) -> Action {
        self.set(Visibility::Expanded)
    }

    /// Hides the items left of the separator, if that cannot hide the icon itself.
    pub fn collapse(&mut self, layout: Option<Layout>) -> Action {
        if self.visibility == Visibility::Collapsed {
            return Action::None;
        }
        match layout {
            None => Action::Blocked(Blocked::LayoutUnknown),
            Some(layout) if !layout.icon_x.is_finite() || !layout.separator_x.is_finite() => {
                Action::Blocked(Blocked::LayoutUnknown)
            }
            Some(layout) if layout.separator_x >= layout.icon_x => {
                Action::Blocked(Blocked::SeparatorRightOfIcon)
            }
            Some(_) => self.set(Visibility::Collapsed),
        }
    }

    fn set(&mut self, visibility: Visibility) -> Action {
        if self.visibility == visibility {
            return Action::None;
        }
        self.visibility = visibility;
        self.generation = self.generation.wrapping_add(1);
        self.user_expanded = visibility == Visibility::Expanded;
        if visibility == Visibility::Collapsed {
            self.revealing_all = false;
        }
        Action::Apply(visibility)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDERED: Layout = Layout {
        icon_x: 1200.0,
        separator_x: 1100.0,
    };

    #[test]
    fn starts_expanded() {
        assert_eq!(Bar::new().visibility(), Visibility::Expanded);
    }

    #[test]
    fn toggle_collapses_then_expands() {
        let mut bar = Bar::new();
        assert_eq!(
            bar.toggle(Some(ORDERED)),
            Action::Apply(Visibility::Collapsed)
        );
        assert_eq!(bar.visibility(), Visibility::Collapsed);
        assert_eq!(bar.toggle(None), Action::Apply(Visibility::Expanded));
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn blocks_collapse_when_separator_right_of_icon() {
        let mut bar = Bar::new();
        let swapped = Layout {
            icon_x: 1100.0,
            separator_x: 1200.0,
        };
        assert_eq!(
            bar.toggle(Some(swapped)),
            Action::Blocked(Blocked::SeparatorRightOfIcon)
        );
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn blocks_collapse_when_items_overlap() {
        let mut bar = Bar::new();
        let same = Layout {
            icon_x: 1100.0,
            separator_x: 1100.0,
        };
        assert_eq!(
            bar.collapse(Some(same)),
            Action::Blocked(Blocked::SeparatorRightOfIcon)
        );
    }

    #[test]
    fn blocks_collapse_when_layout_unknown() {
        let mut bar = Bar::new();
        assert_eq!(bar.toggle(None), Action::Blocked(Blocked::LayoutUnknown));
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn repeated_requests_are_no_ops() {
        let mut bar = Bar::new();
        assert_eq!(bar.expand(), Action::None);
        bar.collapse(Some(ORDERED));
        assert_eq!(bar.collapse(Some(ORDERED)), Action::None);
        assert_eq!(bar.collapse(None), Action::None);
    }

    #[test]
    fn expanding_never_needs_layout() {
        let mut bar = Bar::new();
        bar.collapse(Some(ORDERED));
        assert_eq!(bar.expand(), Action::Apply(Visibility::Expanded));
    }

    #[test]
    fn auto_hide_schedules_after_user_expand() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.collapse(Some(ORDERED));

        assert_eq!(bar.expand(), Action::Apply(Visibility::Expanded));
        assert_eq!(
            bar.pending_collapse(),
            Some(CollapseTimer {
                generation: 2,
                after: Duration::from_secs(5),
            })
        );
    }

    #[test]
    fn auto_hide_off_schedules_nothing() {
        let mut bar = Bar::new();
        bar.set_auto_hide(None);
        bar.collapse(Some(ORDERED));
        bar.expand();

        assert_eq!(bar.pending_collapse(), None);
    }

    #[test]
    fn timer_collapses_when_current() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.collapse(Some(ORDERED));
        bar.expand();

        assert_eq!(
            bar.on_timer(2, Some(ORDERED)),
            Action::Apply(Visibility::Collapsed)
        );
        assert_eq!(bar.visibility(), Visibility::Collapsed);
    }

    #[test]
    fn stale_timer_is_ignored() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.collapse(Some(ORDERED));
        bar.expand();
        bar.collapse(Some(ORDERED));
        bar.expand();

        assert_eq!(bar.on_timer(2, Some(ORDERED)), Action::None);
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn timer_respects_order_guard() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.collapse(Some(ORDERED));
        bar.expand();
        let swapped = Layout {
            icon_x: 1100.0,
            separator_x: 1200.0,
        };

        assert_eq!(
            bar.on_timer(2, Some(swapped)),
            Action::Blocked(Blocked::SeparatorRightOfIcon)
        );
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn launch_expansion_does_not_schedule() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));

        assert_eq!(bar.pending_collapse(), None);
    }

    #[test]
    fn always_hidden_divider_collapses_when_ordered() {
        let bar = Bar::new();
        assert_eq!(
            bar.always_hidden_divider(Some(ORDERED), 900.0),
            Divider::Collapsed
        );
    }

    #[test]
    fn always_hidden_divider_stays_thin_when_out_of_order() {
        let bar = Bar::new();
        // Right of the separator: growing it would hide the separator and the icon.
        assert_eq!(
            bar.always_hidden_divider(Some(ORDERED), 1150.0),
            Divider::Thin
        );
        assert_eq!(bar.always_hidden_divider(None, 900.0), Divider::Thin);
    }

    #[test]
    fn reveal_all_shows_always_hidden_until_next_collapse() {
        let mut bar = Bar::new();
        assert_eq!(bar.reveal_all(), Action::Apply(Visibility::Expanded));
        assert!(bar.revealing_all());
        assert_eq!(
            bar.always_hidden_divider(Some(ORDERED), 900.0),
            Divider::Thin
        );
        bar.collapse(Some(ORDERED));
        assert!(!bar.revealing_all());
        bar.expand();
        assert_eq!(
            bar.always_hidden_divider(Some(ORDERED), 900.0),
            Divider::Collapsed
        );
    }

    #[test]
    fn reveal_all_from_collapsed_expands_and_schedules_auto_hide() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.collapse(Some(ORDERED));
        assert_eq!(bar.reveal_all(), Action::Apply(Visibility::Expanded));
        assert!(bar.pending_collapse().is_some());
    }
    #[test]
    fn reveal_all_restarts_auto_hide_when_already_expanded() {
        let mut bar = Bar::new();
        bar.set_auto_hide(Some(Duration::from_secs(5)));
        bar.reveal_all();
        let first = bar
            .pending_collapse()
            .expect("user reveal schedules a timer");
        bar.reveal_all();
        let second = bar.pending_collapse().unwrap();
        assert_ne!(first.generation, second.generation);
        assert_eq!(bar.on_timer(first.generation, Some(ORDERED)), Action::None);
    }

    #[test]
    fn invalid_positions_never_collapse_dividers() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let layout = Layout {
                icon_x: invalid,
                separator_x: 1100.0,
            };
            let mut bar = Bar::new();
            assert_eq!(
                bar.collapse(Some(layout)),
                Action::Blocked(Blocked::LayoutUnknown)
            );
            assert_eq!(
                bar.always_hidden_divider(Some(layout), 900.0),
                Divider::Thin
            );
        }
    }
}
