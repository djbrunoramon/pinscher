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
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            visibility: Visibility::Expanded,
            auto_hide: None,
            generation: 0,
            user_expanded: false,
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
}
