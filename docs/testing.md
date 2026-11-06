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
9. In Preferences, enable auto-hide and confirm the bar collapses after the configured delay; disable it and confirm it stays expanded.

## Logs

To inspect unified logs while testing, run:

```sh
/usr/bin/log stream --level info --predicate 'subsystem == "io.github.djbrunoramon.pinscher"'
```

Do not share log lines containing personal information. Do not capture or share other apps' private content.
