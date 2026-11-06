//! Logging setup: unified logging (Console.app) plus stderr in debug builds.
//!
//! Never log user paths, other apps' names or config values; log keys and error kinds only.

use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs the global tracing subscriber. Safe to call once at startup.
pub fn init() {
    let filter = std::env::var("PINSCHER_LOG")
        .ok()
        .and_then(|level| level.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::INFO);
    let oslog = tracing_oslog::OsLogger::new(crate::BUNDLE_ID, "default");
    let stderr = cfg!(debug_assertions).then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(false)
            .boxed()
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(oslog)
        .with(stderr)
        .try_init();
}
