#!/bin/bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
configuration="${1:-release}"
case "$configuration" in debug|release) ;; *) echo "Usage: bash scripts/build-macos.sh [debug|release]" >&2; exit 2 ;; esac
cd "$repo_root"
version="$(python3 scripts/release_version.py "${KRILL_VERSION:-$(cat VERSION)}")"
build_args=(--locked --package krill-desktop --target aarch64-apple-darwin)
profile=debug
if [[ "$configuration" == release ]]; then build_args+=(--release); profile=release; fi
KRILL_VERSION="$version" MACOSX_DEPLOYMENT_TARGET=15.0 cargo build "${build_args[@]}"
app_bundle="$repo_root/dist/Krill.Desktop-$version-macos-arm64-$configuration/Krill Desktop.app"
rm -rf "$app_bundle"
mkdir -p "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Resources"
cp "target/aarch64-apple-darwin/$profile/KrillMac" "$app_bundle/Contents/MacOS/KrillMac"
cp resources/macos/Info.plist "$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app_bundle/Contents/Info.plist"
cp resources/macos/AppIcon.icns "$app_bundle/Contents/Resources/AppIcon.icns"
cp resources/update/install-update.sh "$app_bundle/Contents/Resources/install-update.sh"
/usr/bin/plutil -lint "$app_bundle/Contents/Info.plist"
if [[ -n "${KRILL_SIGN_IDENTITY:-}" ]]; then
  /usr/bin/codesign --force --options runtime --timestamp --sign "$KRILL_SIGN_IDENTITY" "$app_bundle"
else
  /usr/bin/codesign --force --sign - "$app_bundle"
fi
/usr/bin/codesign --verify --strict "$app_bundle"
if [[ "$configuration" == release ]]; then
  /usr/bin/ditto -c -k --sequesterRsrc --keepParent "$app_bundle" "$repo_root/dist/Krill.Desktop-$version-macos-arm64.zip"
fi
echo "$app_bundle"
