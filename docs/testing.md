# Testing

## Automated checks

Run these from the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/check-arch.sh
```

## Manual smoke test

On macOS 14 or later:

1. Run `./scripts/bundle.sh`, then launch `dist/Pinscher.app`.
2. Confirm the Pinscher icon appears in the menu bar.
3. Click the icon and confirm the items to the left of the separator hide; click again and confirm they return.
4. Hold ⌘ and drag a menu bar item to the left of the separator, then toggle and confirm it is hidden.
5. Move the separator to the right of the Pinscher icon and toggle. Confirm Pinscher shows a warning instead of hiding items.
6. Right-click the Pinscher icon and confirm its menu opens.
7. Press ⌥⌘P and confirm visibility toggles.
8. From the menu, enable **Launch at Login**. Confirm the app is listed in System Settings › General › Login Items and starts at login. This check requires running from the app bundle and may require approval.
9. In Settings, enable auto-hide and confirm the bar collapses after the configured delay; disable it and confirm it stays expanded.
10. Enable the always-hidden section. Confirm its divider appears left of the regular separator; use the menu's reveal action and ⌘-drag to put an item left of it. Expand normally: the item should remain hidden. Disable the section and confirm its divider disappears. Re-enable and verify its saved position.
11. Move either divider into an unsafe order. Confirm Pinscher stays reachable; the always-hidden divider should remain thin when its order cannot be established.
12. With advanced features off and permissions missing, restart the app: no permission prompt or synthetic move should occur. Basic toggle and settings must remain usable.
13. Enable the second bar and grant Accessibility and Screen Recording in System Settings. Enable it again if a prompt is still awaiting approval. Expand normally for at least five seconds, then hide and ⌥-click Pinscher: previously visible items should have snapshots; unseen icons should use app-icon fallbacks. No image files should be created by Pinscher.
14. Activate a second-bar item. Confirm the real item is revealed and its menu opens. Verify previous visibility returns after the configured delay with auto-hide both on and off; an expanded prior state should restart its ordinary auto-hide timer when enabled. Test with an always-hidden item and with an item lacking screen space.
15. In Layout, click Refresh and Arrange. With Accessibility allowed, confirm three sections appear. Move an item between sections, including when the separator touches Pinscher. Confirm the real item moved and only then check persistence across restart. Scroll down: the list must not jump to the top on background ticks.
16. Click Restore Saved Layout and confirm the remembered sections are reapplied only on request. Missing/ambiguous items should be skipped. With always-hidden disabled but a saved item targeting that section, verify a failure message rather than false success.
17. Revoke Accessibility or Screen Recording while advanced features are active. Confirm the panel closes, cached images become unavailable and no further moves start. Repeat with multiple displays, including one left of or above the primary display.

Steps 13–17 require manual validation with the app's own permissions granted. They have not been confirmed by compilation or unit tests alone.

## Logs

To inspect unified logs while testing, run:

```sh
/usr/bin/log stream --level info --predicate 'subsystem == "io.github.djbrunoramon.pinscher"'
```

Do not share log lines containing personal information. Do not capture or share other apps' private content.
