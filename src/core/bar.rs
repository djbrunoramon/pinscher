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

/// Longest width macOS 27 still honors for a status item is roughly half the screen width; a
/// wider item is ignored (it takes no space and pushes nothing). Measured on macOS 27 with a
/// 2056 pt display: 950 pt hid the neighbours to its left, 1020 pt and above did not.
const COLLAPSED_SCREEN_FRACTION: f64 = 0.45;
/// Upper bound, in case the limit is absolute rather than relative to the screen.
const COLLAPSED_MAX_LENGTH: f64 = 900.0;
/// Width used when the screen size is unknown.
const COLLAPSED_FALLBACK_LENGTH: f64 = 600.0;

/// Width of a collapsed separator on a screen `screen_width` points wide.
///
/// Items to the left of a wide separator are pushed out of the visible menu bar. The width must
/// stay below what macOS accepts, or the separator stops working altogether.
#[must_use]
pub fn collapsed_length(screen_width: f64) -> f64 {
    if screen_width.is_finite() && screen_width > 0.0 {
        (screen_width * COLLAPSED_SCREEN_FRACTION).min(COLLAPSED_MAX_LENGTH)
    } else {
        COLLAPSED_FALLBACK_LENGTH
    }
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

/// Wait before the first collapse after launch, so macOS has placed the status items.
pub const STARTUP_COLLAPSE_DELAY: Duration = Duration::from_millis(1500);
/// Wait between collapse attempts while the status items are still settling.
const STARTUP_RETRY_DELAY: Duration = Duration::from_secs(1);
/// Collapse attempts after which Pinscher stops trying on its own (about 20 s in all).
const STARTUP_MAX_ATTEMPTS: u8 = 20;

/// When to try the launch-time collapse again after it was `blocked`, or `None` to give up.
///
/// Right after login macOS may not have placed the status items yet, so positions are unknown
/// or briefly out of order. Trying again a moment later is safe: the collapse still goes through
/// the same order check, so it can never hide the Pinscher icon.
#[must_use]
pub fn startup_retry(attempt: u8) -> Option<Duration> {
    (attempt < STARTUP_MAX_ATTEMPTS).then_some(STARTUP_RETRY_DELAY)
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

    /// The timer request for the collapse that hides the icons right after launch.
    #[must_use]
    pub fn startup_collapse(&self, after: Duration) -> CollapseTimer {
        CollapseTimer {
            generation: self.generation,
            after,
        }
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

    /// Ends an explicit temporary reveal, restoring the previous visibility when safe.
    pub fn finish_temporary_reveal(
        &mut self,
        previous: Visibility,
        layout: Option<Layout>,
    ) -> Action {
        self.revealing_all = false;
        self.generation = self.generation.wrapping_add(1);
        if previous == Visibility::Collapsed {
            match self.collapse(layout) {
                Action::Blocked(_) => Action::Apply(self.visibility),
                action => action,
            }
        } else {
            self.visibility = Visibility::Expanded;
            Action::Apply(Visibility::Expanded)
        }
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
    fn collapsed_length_stays_under_the_limit_macos_accepts() {
        // 2056 pt: 950 pt worked and 1020 pt did not.
        assert!((collapsed_length(2056.0) - 900.0).abs() < f64::EPSILON);
        assert!(collapsed_length(3440.0) <= 900.0);
    }

    #[test]
    fn collapsed_length_scales_down_on_small_screens() {
        assert!((collapsed_length(1440.0) - 648.0).abs() < f64::EPSILON);
        assert!(collapsed_length(1440.0) < 1440.0 / 2.0);
    }

    #[test]
    fn collapsed_length_falls_back_for_unknown_screens() {
        for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                (collapsed_length(width) - 600.0).abs() < f64::EPSILON,
                "{width}"
            );
        }
    }

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
    #[test]
    fn temporary_reveal_restores_always_hidden_even_without_auto_hide() {
        let mut bar = Bar::new();
        bar.set_auto_hide(None);
        bar.reveal_all();
        assert_eq!(
            bar.finish_temporary_reveal(Visibility::Expanded, Some(ORDERED)),
            Action::Apply(Visibility::Expanded)
        );
        assert!(!bar.revealing_all());
        assert_eq!(
            bar.always_hidden_divider(Some(ORDERED), 900.0),
            Divider::Collapsed
        );
        bar.reveal_all();
        assert_eq!(
            bar.finish_temporary_reveal(Visibility::Collapsed, Some(ORDERED)),
            Action::Apply(Visibility::Collapsed)
        );
    }

    #[test]
    fn startup_collapse_targets_the_current_generation_and_collapses() {
        let mut bar = Bar::new();
        let request = bar.startup_collapse(STARTUP_COLLAPSE_DELAY);
        assert_eq!(request.after, STARTUP_COLLAPSE_DELAY);
        assert_eq!(
            bar.on_timer(request.generation, Some(ORDERED)),
            Action::Apply(Visibility::Collapsed)
        );
    }

    #[test]
    fn startup_collapse_is_dropped_when_the_user_acts_first() {
        let mut bar = Bar::new();
        let request = bar.startup_collapse(STARTUP_COLLAPSE_DELAY);
        bar.collapse(Some(ORDERED));
        bar.expand();
        assert_eq!(
            bar.on_timer(request.generation, Some(ORDERED)),
            Action::None
        );
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn startup_collapse_never_hides_the_icon_even_when_unsettled() {
        let mut bar = Bar::new();
        let request = bar.startup_collapse(STARTUP_COLLAPSE_DELAY);
        let swapped = Layout {
            icon_x: 100.0,
            separator_x: 900.0,
        };
        assert_eq!(
            bar.on_timer(request.generation, Some(swapped)),
            Action::Blocked(Blocked::SeparatorRightOfIcon)
        );
        assert_eq!(
            bar.on_timer(request.generation, None),
            Action::Blocked(Blocked::LayoutUnknown)
        );
        assert_eq!(bar.visibility(), Visibility::Expanded);
    }

    #[test]
    fn startup_retries_for_a_while_then_gives_up() {
        assert_eq!(startup_retry(0), Some(Duration::from_secs(1)));
        assert_eq!(startup_retry(19), Some(Duration::from_secs(1)));
        assert_eq!(startup_retry(20), None);
    }
}
