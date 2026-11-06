#!/usr/bin/env bash
# Creates distributable archives of Pinscher.app in dist/.
#
# By default it builds the app first (scripts/bundle.sh, ad-hoc signed). Set PINSCHER_SKIP_BUILD=1
# to package an existing dist/Pinscher.app instead; CI uses this so that dependency build scripts
# never run in a step that can see signing or notarization secrets.
#
# With PINSCHER_SIGN_IDENTITY set to a Developer ID, the app is re-signed with it. Notarization runs
# when, in addition, PINSCHER_NOTARY_PROFILE (a `notarytool store-credentials` profile) or all of
# APPLE_ID, APPLE_TEAM_ID and APPLE_APP_PASSWORD are set.
set -euo pipefail
set +x

cd "$(dirname "$0")/.."

if [[ "${PINSCHER_SKIP_BUILD:-}" != "1" ]]; then
  ./scripts/bundle.sh
fi

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP=dist/Pinscher.app
ZIP=dist/Pinscher-${VERSION}.zip
DMG=dist/Pinscher-${VERSION}.dmg
IDENTITY=${PINSCHER_SIGN_IDENTITY:--}
NOTARY_PROFILE=${PINSCHER_NOTARY_PROFILE:-}

[[ -d "$APP" ]] || { echo "missing $APP; run scripts/bundle.sh first" >&2; exit 1; }
rm -f "$ZIP" "$DMG" dist/SHA256SUMS

if [[ "$IDENTITY" != "-" ]]; then
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
  codesign --verify --strict "$APP"
fi

notarize=false
if [[ "$IDENTITY" != "-" ]]; then
  if [[ -n "$NOTARY_PROFILE" ]]; then
    NOTARY_ARGS=(--keychain-profile "$NOTARY_PROFILE")
    notarize=true
  elif [[ -n "${APPLE_ID:-}" && -n "${APPLE_TEAM_ID:-}" && -n "${APPLE_APP_PASSWORD:-}" ]]; then
    NOTARY_ARGS=(--apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD")
    notarize=true
  fi
fi

if [[ "$notarize" == true ]]; then
  # Notarize the app through a zip, staple the ticket, then archive the stapled app.
  SUBMISSION=$(mktemp -d "${TMPDIR:-/tmp}/pinscher-notary.XXXXXX")
  trap 'rm -rf "$SUBMISSION"' EXIT
  ditto -c -k --keepParent "$APP" "$SUBMISSION/Pinscher.zip"
  xcrun notarytool submit "$SUBMISSION/Pinscher.zip" --wait "${NOTARY_ARGS[@]}"
  xcrun stapler staple "$APP"
else
  echo "skipping notarization (no credentials)"
fi

ditto -c -k --keepParent "$APP" "$ZIP"

DMG_STAGING=$(mktemp -d "${TMPDIR:-/tmp}/pinscher-dmg.XXXXXX")
ditto "$APP" "$DMG_STAGING/Pinscher.app"
ln -s /Applications "$DMG_STAGING/Applications"
hdiutil create -volname Pinscher -srcfolder "$DMG_STAGING" -format UDZO "$DMG" >/dev/null
rm -rf "$DMG_STAGING"

if [[ "$IDENTITY" != "-" ]]; then
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"
fi
if [[ "$notarize" == true ]]; then
  xcrun notarytool submit "$DMG" --wait "${NOTARY_ARGS[@]}"
  xcrun stapler staple "$DMG"
fi

(cd dist && shasum -a 256 "$(basename "$ZIP")" "$(basename "$DMG")" > SHA256SUMS)
echo "Packaged $ZIP and $DMG"
