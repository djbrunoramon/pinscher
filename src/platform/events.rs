//! Posts an explicitly requested, validated menu-bar drag without blocking the main thread.
//!
//! Call only from a user action, after expanding the dividers and reading fresh item geometry.
//! Keep the menu bar expanded and disable layout-changing timers until `is_running()` is false.
//! Posting has no acknowledgement API; the caller must rediscover items to verify the move.

use crate::core::drag::{Plan, Point};
use crate::platform::permissions;
use block2::StackBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_core_foundation::{CFRetained, CGPoint};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGEventType,
    CGMouseButton,
};
use objc2_foundation::NSTimer;
use std::cell::Cell;
use std::rc::Rc;

/// Failure before starting a drag. No events are posted when `start` returns an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DragError {
    #[error("accessibility permission is missing")]
    PermissionDenied,
    #[error("another drag is still running")]
    Busy,
    #[error("could not create mouse events")]
    EventUnavailable,
    #[error("the menu bar item changed before the drag")]
    StaleItem,
}

#[derive(Debug)]
struct Release {
    origin: CFRetained<CGEvent>,
    destination: CFRetained<CGEvent>,
    moved: Rc<Cell<bool>>,
}

/// Owns one main-thread drag. Dropping or cancelling it releases its synthetic mouse button.
#[derive(Debug)]
pub struct Drag {
    _mtm: MainThreadMarker,
    timer: Option<Retained<NSTimer>>,
    running: Rc<Cell<bool>>,
    release: Option<Release>,
}

impl Drag {
    #[must_use]
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            _mtm: mtm,
            timer: None,
            running: Rc::new(Cell::new(false)),
            release: None,
        }
    }

    /// Starts a command-drag built by `core::drag`; never requests permission.
    ///
    /// # Errors
    /// Refuses missing Accessibility access, a concurrent drag or event-allocation failure.
    pub fn start(
        &mut self,
        plan: &Plan,
        geometry_is_current: impl Fn() -> bool + 'static,
    ) -> Result<(), DragError> {
        if self.is_running() {
            return Err(DragError::Busy);
        }
        if !permissions::granted().accessibility {
            return Err(DragError::PermissionDenied);
        }
        if !geometry_is_current() || !item_is_current(plan.item()) {
            return Err(DragError::StaleItem);
        }
        self.cancel();
        let source =
            CGEventSource::new(CGEventSourceStateID::Private).ok_or(DragError::EventUnavailable)?;
        CGEventSource::set_local_events_suppression_interval(Some(&source), 0.0);
        // Allocate every event before posting anything, so allocation failure cannot leave a
        // synthetic mouse button pressed. The private source avoids changing hardware state.
        let down = mouse_event(&source, CGEventType::LeftMouseDown, plan.from(), true)?;
        let dragged = mouse_event(&source, CGEventType::LeftMouseDragged, plan.to(), true)?;
        let up = mouse_event(&source, CGEventType::LeftMouseUp, plan.to(), false)?;
        let up_from = mouse_event(&source, CGEventType::LeftMouseUp, plan.from(), false)?;
        let running = Rc::clone(&self.running);
        let release = up.clone();
        let release_from = up_from.clone();
        let moved = Rc::new(Cell::new(false));
        let did_move = Rc::clone(&moved);
        let expected = plan.item().clone();
        let geometry_is_current = Rc::new(geometry_is_current);
        let block = StackBlock::new(move |timer: std::ptr::NonNull<NSTimer>| {
            // SAFETY: NSTimer supplies its own valid timer object for this callback.
            let timer = unsafe { timer.as_ref() };
            if did_move.get()
                || !permissions::granted().accessibility
                || !geometry_is_current()
                || !item_is_current(&expected)
            {
                // Always attempt the release, including after revoked permission. Quartz may
                // discard it, but continuing a command-drag after revocation is never allowed.
                let event = if did_move.get() {
                    &release
                } else {
                    &release_from
                };
                CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event));
                running.set(false);
                timer.invalidate();
            } else {
                CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&dragged));
                did_move.set(true);
            }
        });
        // SAFETY: this non-escaping stack block is copied by NSTimer. It has the documented
        // timer-pointer signature and only captures owned events and main-thread state.
        let timer =
            unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(0.08, true, &block) };
        self.timer = Some(timer);
        self.release = Some(Release {
            origin: up_from,
            destination: up,
            moved,
        });
        self.running.set(true);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&down));
        Ok(())
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.get()
    }

    /// Stops a pending drag and releases its button and Command flag within the menu bar.
    pub fn cancel(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
        if self.running.replace(false)
            && let Some(release) = self.release.as_ref()
        {
            let release = if release.moved.get() {
                &release.destination
            } else {
                &release.origin
            };
            CGEvent::post(CGEventTapLocation::HIDEventTap, Some(release));
        }
        self.release = None;
    }
}

impl Drop for Drag {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn mouse_event(
    source: &CGEventSource,
    kind: CGEventType,
    point: Point,
    command: bool,
) -> Result<CFRetained<CGEvent>, DragError> {
    let event = CGEvent::new_mouse_event(
        Some(source),
        kind,
        CGPoint::new(point.x(), point.y()),
        CGMouseButton::Left,
    )
    .ok_or(DragError::EventUnavailable)?;
    CGEvent::set_flags(
        Some(&event),
        if command {
            CGEventFlags::MaskCommand
        } else {
            CGEventFlags::empty()
        },
    );
    Ok(event)
}

fn item_is_current(expected: &crate::core::layout::MenuBarItem) -> bool {
    let items = crate::core::layout::menu_bar_items(
        crate::platform::menu_extras::items(),
        i32::try_from(std::process::id()).unwrap_or_default(),
    );
    crate::core::layout::unique_item(&items, expected)
        .is_some_and(|current| current.frame == expected.frame)
}
