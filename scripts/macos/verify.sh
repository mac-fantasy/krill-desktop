#!/bin/bash
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then echo "Verification requires macOS and Xcode." >&2; exit 1; fi
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
package_dir="$repo_root/apps/macos"
python3 -m unittest discover -s "$repo_root/scripts/tests" -v
swift --version
/usr/bin/plutil -lint "$package_dir/Support/Info.plist"
swift test --package-path "$package_dir"
swift build --package-path "$package_dir" -c release --product KrillMac
binary_dir="$(swift build --package-path "$package_dir" -c release --show-bin-path)"
symbols="$(/usr/bin/nm -u "$binary_dir/KrillMac" | xcrun swift-demangle)"
# An empty SwiftUI Settings scene can create a blank window when the accessory app activates.
if [[ "$symbols" == *"SwiftUI.Settings"* || "$symbols" == *"SwiftUI.App.main()"* || "$symbols" == *"SwiftUI.NSApplicationDelegateAdaptor"* ]]; then
  echo "FAIL: the menu-bar application must use the AppKit entry point without SwiftUI window scenes." >&2
  exit 1
fi
echo "PASS Swift tests, macOS compilation, and AppKit-only application entry point. GUI/login still require manual verification."
