//! Whether hidden icons are shown, and when a change is allowed.
//!
//! The menu bar lays items out right to left: the Pinscher icon sits on the right and the
//! separator somewhere to its left. Collapsing grows the separator so everything to its left is
//! pushed off screen. If the separator were to the right of the icon, collapsing would push the
//! icon itself away and leave the user with no way back (AM-06), so that is refused.

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

/// The show/hide state machine.
#[derive(Debug, Default)]
pub struct Bar {
    visibility: Visibility,
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
}
