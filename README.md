[Leia em português](README.pt-BR.md)

# Pinscher

Pinscher is a macOS menu bar manager that lets you hide menu bar items you do not need to see all the time.

## Status

Early development. Behavior and configuration may change between releases.

## Features

- **Available:** Show or hide menu bar items with a click.
- **Available:** Move the separator with ⌘-drag to choose which items stay hidden to its left.
- **Available:** Toggle visibility with ⌥⌘P and configure auto-hide and launch at login in Preferences.
- **Planned:** A second bar, an always-hidden mode, and reordering items in Preferences.

## Requirements

- macOS 14 or later
- Apple silicon or Intel Mac

## Install

Download a release from [GitHub Releases](https://github.com/djbrunoramon/pinscher/releases) and move `Pinscher.app` to Applications.

Release bundles are ad-hoc signed. If Gatekeeper blocks a build you trust, Control-click (or right-click) the app and choose **Open**. You can also remove quarantine from a trusted downloaded app in Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/Pinscher.app
```

## Use

- Click the Pinscher menu bar icon to show or hide the items left of its separator.
- Right-click the icon or press ⌃ while clicking to open the menu.
- Press ⌥⌘P to toggle visibility.
- Hold ⌘ and drag a menu bar item to choose which items remain hidden to the left of the separator. The separator must be left of the Pinscher icon; otherwise Pinscher shows a warning and does not hide items.
- Enable **Launch at Login** from the menu. This works when Pinscher runs from its app bundle and may require approval in System Settings.
- Preferences include auto-hide settings and the global shortcut.

## Permissions and privacy

The basic show/hide feature does not request Screen Recording or Accessibility permission. Advanced features that inspect or control other menu bar items may require Screen Recording to capture their appearance and Accessibility to interact with them. Those features are planned and are not part of the current basic behavior.

Pinscher makes no network requests and has no telemetry. Any captures used by advanced features are kept in memory; Pinscher does not save captured images.

## Build from source

Install Rust with rustup; it reads the pinned toolchain in `rust-toolchain.toml`. Then run:

```sh
cargo build
cargo test
./scripts/bundle.sh
```

The bundle script creates a universal, ad-hoc signed `dist/Pinscher.app`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, project boundaries, and the pull request process. Please report bugs using [GitHub Issues](https://github.com/djbrunoramon/pinscher/issues).

## License

Pinscher is licensed under the [MIT License](LICENSE).
