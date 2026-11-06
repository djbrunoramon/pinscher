#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) Pinscher.app in dist/.
# Signing identity, in order:
#   1. PINSCHER_SIGN_IDENTITY, when set ("-" forces ad-hoc).
#   2. A "Developer ID Application" or "Apple Development" certificate found in your keychain.
#      Their identity stays the same between builds, so macOS keeps the Accessibility and
#      Screen Recording permissions you granted. (An ad-hoc signature is the binary's hash,
#      which changes on every build, so macOS forgets the permissions each time.)
#   3. Ad-hoc, for machines without a certificate (CI, first-time contributors).
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=$(cargo metadata --no-deps --format-version 1 | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
BUILD=${PINSCHER_BUILD_NUMBER:-$(git rev-list --count HEAD 2>/dev/null || echo 1)}

# Picks the first Developer ID, else the first Apple Development, certificate that is usable.
detect_identity() {
  local kind line
  for kind in "Developer ID Application" "Apple Development"; do
    line=$(security find-identity -v -p codesigning 2>/dev/null | grep "\"$kind" | head -1 || true)
    if [[ -n "$line" ]]; then
      sed -E 's/^[[:space:]]*[0-9]+\)[[:space:]]+([0-9A-F]{40}).*/\1/' <<<"$line"
      return
    fi
  done
  echo "-"
}
IDENTITY=${PINSCHER_SIGN_IDENTITY:-$(detect_identity)}
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
cp packaging/Pinscher.icns packaging/PinscherTemplate.png "packaging/PinscherTemplate@2x.png" "$APP/Contents/Resources/"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

# Only Developer ID signatures are timestamped (that is what notarization needs); a development
# certificate or ad-hoc signature must not depend on Apple's timestamp server being reachable.
if [[ "$IDENTITY" != "-" ]] && security find-identity -v -p codesigning | grep "$IDENTITY" | grep -q "Developer ID Application"; then
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
else
  codesign --force --options runtime --timestamp=none --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"

if [[ "$IDENTITY" == "-" ]]; then
  SIGNED_WITH="ad-hoc (permissions will be forgotten on every build; see header of this script)"
else
  SIGNED_WITH=$(codesign -dvv "$APP" 2>&1 | sed -n 's/^Authority=//p' | head -1)
fi
echo "Built $APP ($VERSION build $BUILD, signed with: $SIGNED_WITH)"
