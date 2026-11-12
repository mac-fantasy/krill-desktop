#!/bin/bash
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then
  echo "This script requires macOS and the Xcode command-line tools." >&2
  exit 1
fi
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
package_dir="$repo_root/apps/macos"
configuration="${1:-release}"
case "$configuration" in debug|release) ;; *) echo "Usage: bash scripts/macos/build.sh [debug|release] [--universal|--arm64]" >&2; exit 2 ;; esac
version="$(python3 "$repo_root/scripts/release_version.py" "${KRILL_VERSION:-$(cat "$repo_root/VERSION")}")"
build_args=(--package-path "$package_dir" -c "$configuration")
flavor="$(uname -m)"
if [[ "${2:-}" == --universal ]]; then
  build_args+=(--arch arm64 --arch x86_64)
  flavor=universal
elif [[ "${2:-}" == --arm64 ]]; then
  build_args+=(--arch arm64)
  flavor=arm64
elif [[ -n "${2:-}" ]]; then
  echo "Unknown option: $2" >&2; exit 2
fi
swift build "${build_args[@]}" --product KrillMac
binary_dir="$(swift build "${build_args[@]}" --show-bin-path)"
output_dir="$repo_root/dist/Krill.Desktop-$version-macos-$flavor-$configuration"
app_bundle="$output_dir/Krill Desktop.app"
rm -rf "$app_bundle"
mkdir -p "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Resources"
cp "$binary_dir/KrillMac" "$app_bundle/Contents/MacOS/KrillMac"
cp "$package_dir/Support/Info.plist" "$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app_bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app_bundle/Contents/Info.plist"
cp "$package_dir/Support/Assets/AppIcon.icns" "$app_bundle/Contents/Resources/AppIcon.icns"
cp "$repo_root/scripts/macos/install-update.sh" "$app_bundle/Contents/Resources/install-update.sh"
/usr/bin/plutil -lint "$app_bundle/Contents/Info.plist"
if [[ -n "${KRILL_SIGN_IDENTITY:-}" ]]; then
  /usr/bin/codesign --force --options runtime --timestamp --sign "$KRILL_SIGN_IDENTITY" "$app_bundle"
else
  /usr/bin/codesign --force --sign - "$app_bundle"
fi
/usr/bin/codesign --verify --strict "$app_bundle"
architectures="$(/usr/bin/lipo -archs "$app_bundle/Contents/MacOS/KrillMac")"
if [[ "$flavor" == universal ]]; then
  [[ " $architectures " == *" arm64 "* && " $architectures " == *" x86_64 "* ]]
elif [[ "$flavor" == arm64 ]]; then
  [[ "$architectures" == arm64 ]]
fi
if [[ "$configuration" == release ]]; then
  archive="$repo_root/dist/Krill.Desktop-$version-macos-$flavor.zip"
  /usr/bin/ditto -c -k --sequesterRsrc --keepParent "$app_bundle" "$archive"
  echo "$archive"
fi
echo "$app_bundle"
echo "Launch: open \"$app_bundle\" --args --demo"
