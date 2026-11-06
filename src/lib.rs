//! Pinscher: a small macOS menu bar manager.
//!
//! - `core`: pure logic with no `AppKit` calls, fully unit tested.
//! - `platform`: thin adapters over macOS APIs (`AppKit`, `CoreGraphics`, ...).
//! - `app`: the application delegate that wires core and platform together.

mod app;
pub mod core;
pub mod platform;

/// Bundle identifier, also used as the unified logging subsystem.
pub const BUNDLE_ID: &str = "io.github.djbrunoramon.pinscher";

/// Starts the application and blocks on the `AppKit` run loop.
///
/// # Panics
/// Panics when called off the main thread.
pub fn run() {
    platform::logging::init();
    app::run();
}
