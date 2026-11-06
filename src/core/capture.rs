//! Bounds and identity checks for in-memory status-item captures.

use crate::core::layout::{MenuBarItem, Rect};

pub const MAX_ENTRIES: usize = 256;
pub const MAX_PIXELS: usize = 512 * 128;

/// Whether an item's entire frame is a small, finite rectangle within the menu bar.
#[must_use]
pub fn eligible(frame: Rect, menu_bar: Rect) -> bool {
    valid_rect(frame)
        && valid_rect(menu_bar)
        && frame.width <= 256.0
        && frame.height <= 64.0
        && menu_bar.height <= 64.0
        && inside(frame, menu_bar)
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

fn inside(frame: Rect, outer: Rect) -> bool {
    frame.x >= outer.x
        && frame.y >= outer.y
        && frame.x + frame.width <= outer.x + outer.width
        && frame.y + frame.height <= outer.y + outer.height
}

/// Converts a frame into display-local coordinates, rejecting frames crossing a display edge.
#[must_use]
pub fn source_rect(frame: Rect, display: Rect) -> Option<Rect> {
    (valid_rect(frame) && valid_rect(display) && inside(frame, display)).then_some(Rect {
        x: frame.x - display.x,
        y: frame.y - display.y,
        ..frame
    })
}

/// Bounded 2× SDR output dimensions, independent of the display resolution.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "positive finite dimensions are bounded before conversion"
)]
pub fn pixel_dimensions(frame: Rect) -> Option<(usize, usize)> {
    if !valid_rect(frame) || frame.width > 256.0 || frame.height > 64.0 {
        return None;
    }
    let width = (frame.width * 2.0).ceil() as usize;
    let height = (frame.height * 2.0).ceil() as usize;
    (width.checked_mul(height)? <= MAX_PIXELS).then_some((width, height))
}

/// Results are accepted only if both the identity and frame stayed unchanged.
#[must_use]
pub fn stable(requested: &MenuBarItem, current: &MenuBarItem) -> bool {
    requested == current
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: Rect = Rect {
        x: -100.0,
        y: 10.0,
        width: 1000.0,
        height: 30.0,
    };
    const FRAME: Rect = Rect {
        x: 100.0,
        y: 10.0,
        width: 30.0,
        height: 24.0,
    };

    #[test]
    fn accepts_small_frame_inside_menu_bar() {
        assert!(eligible(FRAME, BAR));
        assert!(eligible(Rect { x: BAR.x, ..FRAME }, BAR));
    }

    #[test]
    fn rejects_partially_outside_and_oversized_frames() {
        for frame in [
            Rect { x: -101.0, ..FRAME },
            Rect { x: 890.0, ..FRAME },
            Rect { y: 9.0, ..FRAME },
            Rect { y: 39.0, ..FRAME },
            Rect {
                width: 257.0,
                ..FRAME
            },
            Rect {
                height: 65.0,
                ..FRAME
            },
            Rect {
                width: 0.0,
                ..FRAME
            },
            Rect {
                height: -1.0,
                ..FRAME
            },
        ] {
            assert!(!eligible(frame, BAR));
        }
    }

    #[test]
    fn rejects_nonfinite_geometry_and_non_bar_regions() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for frame in [
                Rect { x: value, ..FRAME },
                Rect { y: value, ..FRAME },
                Rect {
                    width: value,
                    ..FRAME
                },
                Rect {
                    height: value,
                    ..FRAME
                },
            ] {
                assert!(!eligible(frame, BAR));
                assert!(!eligible(FRAME, frame));
            }
        }
        assert!(!eligible(
            FRAME,
            Rect {
                height: 1000.0,
                ..BAR
            }
        ));
        assert!(!eligible(FRAME, Rect { width: 0.0, ..BAR }));
    }

    #[test]
    fn translates_negative_display_origins_and_rejects_crossing_edges() {
        assert_eq!(
            source_rect(FRAME, BAR),
            Some(Rect {
                x: 200.0,
                y: 0.0,
                ..FRAME
            })
        );
        assert_eq!(source_rect(Rect { x: -101.0, ..FRAME }, BAR), None);
    }

    #[test]
    fn output_pixels_are_bounded() {
        assert_eq!(pixel_dimensions(FRAME), Some((60, 48)));
        assert_eq!(
            pixel_dimensions(Rect {
                width: 256.0,
                height: 64.0,
                ..FRAME
            }),
            Some((512, 128))
        );
        assert_eq!(
            pixel_dimensions(Rect {
                width: f64::INFINITY,
                ..FRAME
            }),
            None
        );
    }

    #[test]
    fn requires_stable_identity_and_frame() {
        let item = MenuBarItem {
            owner_pid: 42,
            owner: "com.example.app".into(),
            title: "Status".into(),
            frame: FRAME,
        };
        assert!(stable(&item, &item));
        for changed in [
            MenuBarItem {
                owner_pid: 43,
                ..item.clone()
            },
            MenuBarItem {
                owner: "other.app".into(),
                ..item.clone()
            },
            MenuBarItem {
                title: "Other".into(),
                ..item.clone()
            },
            MenuBarItem {
                frame: Rect { x: 101.0, ..FRAME },
                ..item.clone()
            },
        ] {
            assert!(!stable(&item, &changed));
        }
    }
}
