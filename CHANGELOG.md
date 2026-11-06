# Changelog

All notable changes to Pinscher are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- macOS menu bar app skeleton.
- Menu bar separator toggle.
- User configuration.
- English and Brazilian Portuguese localization.
- Continuous integration checks.
- Always-hidden separator and an explicit reveal action for arranging icons.
- Experimental secondary bar with bounded, memory-only ScreenCaptureKit snapshots and app-icon fallback.
- Layout tab with validated command-drag requests, verified persistence and explicit saved-layout restoration.

- Hide icons as soon as Pinscher starts (and after every login), with a Settings switch; if macOS has not placed the items yet it retries for about 20 seconds.
- App icon.
- The Layout tab loads when Settings opens, follows the menu bar while it is visible, and shows each item's app icon and name.

### Fixed

- Collapsing works on macOS 27, which ignores status items wider than roughly half the screen.
- Builds signed with a certificate from your keychain keep the Accessibility and Screen Recording permissions between versions (an ad-hoc signature changed on every build).

- Reject non-finite divider positions and renew auto-hide after revealing all icons.
- Restore temporary reveals even when auto-hide is disabled.
- Preserve existing preferences when a save would exceed the file size limit.
