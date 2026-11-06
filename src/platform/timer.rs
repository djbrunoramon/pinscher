//! Main-run-loop timer used by the auto-hide state machine.

use block2::StackBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2_foundation::{NSNumber, NSTimer};
use std::time::Duration;

/// Owns the currently scheduled one-shot auto-hide timer.
#[derive(Debug, Default)]
pub struct Timer {
    timer: Option<Retained<NSTimer>>,
}

impl Timer {
    /// Invalidates any previous timer and schedules one callback on the main run loop.
    pub fn schedule_once(&mut self, after: Duration, generation: u64, target: &AnyObject) {
        self.cancel();
        let target = std::ptr::from_ref(target) as usize;
        let block = StackBlock::new(move |_| {
            let generation = NSNumber::new_u64(generation);
            // SAFETY: The target is the main-thread app delegate, retained by the application
            // for the lifetime of this timer. The selector and NSNumber argument match the
            // delegate method declaration.
            unsafe {
                let target = &*(target as *const AnyObject);
                let _: () = msg_send![target, autoHideFired: &*generation];
            }
        });
        // SAFETY: The block is a valid Objective-C timer callback and is non-repeating. The
        // interval is finite and the timer is scheduled on the current main run loop.
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_repeats_block(
                after.as_secs_f64(),
                false,
                &block,
            )
        };
        self.timer = Some(timer);
    }

    /// Schedules a main-thread action with no payload, replacing the previous timer.
    pub fn schedule_action(&mut self, after: Duration, target: &AnyObject, action: Sel) {
        self.cancel();
        // SAFETY: target is the retained app delegate, action accepts the NSTimer sender, and
        // this non-repeating timer runs on the main run loop with a finite positive delay.
        self.timer = Some(unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                after.as_secs_f64(),
                target,
                action,
                None,
                false,
            )
        });
    }

    /// Observes status item reordering on the main run loop while a divider is enabled.
    pub fn observe_layout(&mut self, target: &AnyObject, action: Sel) {
        self.cancel();
        // SAFETY: target is the app delegate, retained for the application's lifetime, and
        // action accepts one timer argument. This timer runs on the main thread.
        self.timer = Some(unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                0.5, target, action, None, true,
            )
        });
    }

    /// Invalidates and releases the current timer, if any.
    pub fn cancel(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.cancel();
    }
}
