//! On-demand status-item screenshots, held only in a bounded memory cache.
//!
//! This adapter never asks for permission or reveals items. `ScreenCaptureKit` callbacks send
//! bounded results back to the owner; the owner rechecks the live item before accepting pixels.

use crate::core::capture::{self, MAX_ENTRIES, MAX_PIXELS};
use crate::core::layout::{MenuBarItem, Rect};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::NSImage;
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGImage, CGPreflightScreenCaptureAccess};
use objc2_foundation::{NSArray, NSError, NSSize};
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
};
use std::fmt;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

const MIN_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_TIMEOUT: Duration = Duration::from_secs(30);

struct Cached {
    item: MenuBarItem,
    image: CFRetained<CGImage>,
}

struct ResultImage {
    item: MenuBarItem,
    image: Option<CFRetained<CGImage>>,
}

struct InFlight {
    receiver: Receiver<ResultImage>,
    active: Arc<AtomicBool>,
    started: Instant,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

/// Captures only small visible item frames, never an entire screen or an offscreen item.
#[derive(Default)]
pub struct CaptureCache {
    entries: Vec<Cached>,
    in_flight: Option<InFlight>,
    last_request: Option<Instant>,
}

// Do not make item titles, positions or image objects part of diagnostics.
impl fmt::Debug for CaptureCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CaptureCache")
            .field("entries", &self.entries.len())
            .field("in_flight", &self.in_flight.is_some())
            .finish_non_exhaustive()
    }
}

impl CaptureCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts at most one bounded asynchronous batch, no more often than every five seconds.
    /// Calls only the preflight permission check; a denied check clears all previous images.
    pub fn request(&mut self, items: &[MenuBarItem], menu_bar: Rect) {
        self.collect(items);
        if !CGPreflightScreenCaptureAccess() {
            self.clear();
            return;
        }
        if self.in_flight.is_some() {
            return;
        }
        let now = Instant::now();
        if self
            .last_request
            .is_some_and(|last| now.duration_since(last) < MIN_INTERVAL)
        {
            return;
        }
        let requested: Vec<_> = items
            .iter()
            .filter(|item| {
                item.owner_pid > 0
                    && capture::eligible(item.frame, menu_bar)
                    && items
                        .iter()
                        .filter(|other| same_identity(item, other))
                        .count()
                        == 1
            })
            .take(MAX_ENTRIES)
            .cloned()
            .collect();
        if requested.is_empty() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(MAX_ENTRIES);
        let active = Arc::new(AtomicBool::new(true));
        self.in_flight = Some(InFlight {
            receiver,
            active: Arc::clone(&active),
            started: now,
        });
        self.last_request = Some(now);
        let callback = RcBlock::new(
            move |content: *mut SCShareableContent, error: *mut NSError| {
                if !active.load(Ordering::Acquire) || !CGPreflightScreenCaptureAccess() {
                    return;
                }
                // SAFETY: ScreenCaptureKit supplies a valid borrowed content object, or null on
                // error, and retains it for this callback. It never escapes the callback.
                let content = unsafe { content.as_ref() };
                let Some(content) = content.filter(|_| error.is_null()) else {
                    return;
                };
                // SAFETY: content is valid for this callback. The returned array retains displays.
                let displays = unsafe { content.displays() };
                for item in &requested {
                    if !active.load(Ordering::Acquire) || !CGPreflightScreenCaptureAccess() {
                        break;
                    }
                    let display = displays.iter().find_map(|display| {
                        // SAFETY: the display belongs to the retained SCShareableContent array.
                        let frame = unsafe { display.frame() };
                        capture::source_rect(item.frame, from_cg_rect(frame))
                            .map(|source| (display, source))
                    });
                    if let Some((display, source)) = display {
                        // SAFETY: display is a valid SCDisplay. An empty exclusion list captures
                        // the display; sourceRect below limits the output to this one item.
                        let filter = unsafe {
                            SCContentFilter::initWithDisplay_excludingWindows(
                                SCContentFilter::alloc(),
                                &display,
                                &NSArray::new(),
                            )
                        };
                        capture_item(
                            item.clone(),
                            source,
                            &filter,
                            sender.clone(),
                            Arc::clone(&active),
                        );
                    }
                }
            },
        );
        // SAFETY: macOS 14+ supports this API; ScreenCaptureKit copies the valid callback.
        // Preflight was checked above, so enumeration never intentionally prompts for access.
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&callback) };
    }

    /// Drains completed callbacks and accepts only identities and frames still in the live list.
    /// Existing snapshots may be reused after a move; newly completed stale captures are refused.
    pub fn collect(&mut self, items: &[MenuBarItem]) {
        if !CGPreflightScreenCaptureAccess() {
            self.clear();
            return;
        }
        self.entries.retain(|entry| {
            items
                .iter()
                .filter(|item| same_identity(&entry.item, item))
                .count()
                == 1
        });
        let mut completed = false;
        if let Some(batch) = &self.in_flight {
            if batch.started.elapsed() >= BATCH_TIMEOUT {
                completed = true;
            } else {
                loop {
                    match batch.receiver.try_recv() {
                        Ok(result) => {
                            let Some(image) = result.image else {
                                continue;
                            };
                            if items
                                .iter()
                                .filter(|item| same_identity(&result.item, item))
                                .count()
                                != 1
                                || !items.iter().any(|item| capture::stable(&result.item, item))
                            {
                                continue;
                            }
                            self.entries
                                .retain(|entry| !same_identity(&entry.item, &result.item));
                            if self.entries.len() == MAX_ENTRIES {
                                self.entries.remove(0);
                            }
                            self.entries.push(Cached {
                                item: result.item,
                                image,
                            });
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            completed = true;
                            break;
                        }
                    }
                }
            }
        }
        if completed {
            self.in_flight = None;
        }
    }

    /// Returns a retained snapshot with its original point size, if capture is still allowed.
    /// The current frame may differ: an item can now be hidden while its cached pixels remain.
    #[must_use]
    pub fn image(
        &mut self,
        item: &MenuBarItem,
        _mtm: MainThreadMarker,
    ) -> Option<Retained<NSImage>> {
        if !CGPreflightScreenCaptureAccess() {
            self.clear();
            return None;
        }
        let entry = self
            .entries
            .iter()
            .find(|entry| same_identity(&entry.item, item))?;
        Some(NSImage::initWithCGImage_size(
            NSImage::alloc(),
            &entry.image,
            NSSize::new(entry.item.frame.width, entry.item.frame.height),
        ))
    }

    /// Forgets pixels and disconnects callbacks, preventing late results from restoring them.
    pub fn clear(&mut self) {
        self.in_flight = None;
        self.entries.clear();
    }
}

fn same_identity(a: &MenuBarItem, b: &MenuBarItem) -> bool {
    a.owner_pid == b.owner_pid && a.owner == b.owner && a.title == b.title
}

fn from_cg_rect(rect: CGRect) -> Rect {
    Rect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    }
}

fn capture_item(
    item: MenuBarItem,
    source: Rect,
    filter: &SCContentFilter,
    sender: SyncSender<ResultImage>,
    active: Arc<AtomicBool>,
) {
    if !active.load(Ordering::Acquire) || !CGPreflightScreenCaptureAccess() {
        return;
    }
    let Some((width, height)) = capture::pixel_dimensions(item.frame) else {
        return;
    };
    // SAFETY: SCStreamConfiguration is available on macOS 14+. Dimensions and sourceRect have
    // been checked by core, the rectangle lies entirely within a display, and default dynamic
    // range is SDR. Only macOS 14-compatible setters are used.
    let config = unsafe {
        let config = SCStreamConfiguration::new();
        config.setSourceRect(CGRect::new(
            CGPoint::new(source.x, source.y),
            CGSize::new(source.width, source.height),
        ));
        config.setWidth(width);
        config.setHeight(height);
        config.setShowsCursor(false);
        config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
        config
    };
    let callback = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        if !active.load(Ordering::Acquire) || !CGPreflightScreenCaptureAccess() {
            return;
        }
        let image = if error.is_null() {
            NonNull::new(image).and_then(|image| {
                // SAFETY: the completion handler receives a borrowed valid CGImage. Retaining
                // it extends its lifetime past the callback; CGImage is Send + Sync.
                let image = unsafe { CFRetained::retain(image) };
                let pixels =
                    CGImage::width(Some(&image)).checked_mul(CGImage::height(Some(&image)))?;
                (pixels > 0
                    && pixels <= MAX_PIXELS
                    && CGImage::width(Some(&image)) == width
                    && CGImage::height(Some(&image)) == height)
                    .then_some(image)
            })
        } else {
            None
        };
        let _ = sender.try_send(ResultImage {
            item: item.clone(),
            image,
        });
    });
    // SAFETY: captureImageWithFilter is available since macOS 14. The framework retains/copies
    // the filter, configuration and callback for the asynchronous capture.
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            &config,
            Some(&callback),
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_cancels_and_disconnects_late_callbacks_without_resetting_throttle() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(true));
        let started = Instant::now();
        let mut cache = CaptureCache {
            entries: Vec::new(),
            in_flight: Some(InFlight {
                receiver,
                active: Arc::clone(&active),
                started,
            }),
            last_request: Some(started),
        };
        cache.clear();
        assert!(!active.load(Ordering::Acquire));
        assert!(cache.in_flight.is_none());
        assert!(cache.entries.is_empty());
        assert_eq!(cache.last_request, Some(started));
        assert!(matches!(
            sender.try_send(ResultImage {
                item: MenuBarItem::default(),
                image: None
            }),
            Err(mpsc::TrySendError::Disconnected(_))
        ));
    }
}
