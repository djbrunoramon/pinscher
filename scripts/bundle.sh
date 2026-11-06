#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) Pinscher.app in dist/.
# Signing is ad-hoc by default. Set PINSCHER_SIGN_IDENTITY to a Developer ID to sign for distribution.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=$(cargo metadata --no-deps --format-version 1 | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
BUILD=${PINSCHER_BUILD_NUMBER:-$(git rev-list --count HEAD 2>/dev/null || echo 1)}
IDENTITY=${PINSCHER_SIGN_IDENTITY:--}
APP=dist/Pinscher.app

export MACOSX_DEPLOYMENT_TARGET=14.0
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --locked --target "$target"
done

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create \
  target/aarch64-apple-darwin/release/pinscher \
  target/x86_64-apple-darwin/release/pinscher \
  -output "$APP/Contents/MacOS/pinscher"
sed -e "s/__VERSION__/$VERSION/" -e "s/__BUILD__/$BUILD/" packaging/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

if [[ "$IDENTITY" == "-" ]]; then
  codesign --force --options runtime --timestamp=none --sign "$IDENTITY" "$APP"
else
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"

echo "Built $APP ($VERSION build $BUILD, signed with: $IDENTITY)"
