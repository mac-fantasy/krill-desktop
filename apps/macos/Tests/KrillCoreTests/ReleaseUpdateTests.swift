import Foundation
import XCTest
@testable import KrillCore

final class ReleaseUpdateTests: XCTestCase {
    private func fixture(_ mutate: (inout [String: Any]) -> Void = { _ in }) throws -> GitHubRelease {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../contracts/updates/latest.json")
        var object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        mutate(&object)
        return try JSONDecoder().decode(GitHubRelease.self, from: JSONSerialization.data(withJSONObject: object))
    }
    func testNumericVersionsAndInvalidTags() throws {
        XCTAssertLessThan(try XCTUnwrap(ReleaseVersion("1.9.99")), try XCTUnwrap(ReleaseVersion("v1.10.2")))
        for invalid in ["1.2", "1.2.3.4", "1.2.3-beta.1", "01.2.3", "1.02.3", "-1.2.3", "1.2.65536", "1.2.٣", "1.2.3\n"] {
            XCTAssertNil(ReleaseVersion(invalid), invalid)
        }
    }
    func testSharedReleaseSelectsOnlyMatchingPlatformAndNewerVersion() throws {
        let release = try fixture()
        let current = try XCTUnwrap(ReleaseVersion("1.9.0"))
        XCTAssertEqual(release.update(after: current, platform: "macos-universal")?.size, 2048)
        XCTAssertEqual(release.update(after: current, platform: "win-x64")?.size, 1024)
        XCTAssertNil(release.update(after: current, platform: "macos-arm64"))
        XCTAssertNil(release.update(after: try XCTUnwrap(ReleaseVersion("1.10.2")), platform: "macos-universal"))
        XCTAssertNil(release.update(after: try XCTUnwrap(ReleaseVersion("2.0.0")), platform: "macos-universal"))
    }
    func testDraftsPrereleasesAndUnsignedOrUntrustedAssetsAreRejected() throws {
        let current = try XCTUnwrap(ReleaseVersion("1.0.0"))
        for field in ["draft", "prerelease"] {
            XCTAssertNil(try fixture { $0[field] = true }.update(after: current, platform: "macos-universal"))
        }
        for changes: [String: Any] in [
            ["digest": NSNull()], ["digest": "sha256:bad"], ["digest": "sha256:" + String(repeating: "z", count: 64)],
            ["size": 0], ["size": 1_073_741_825],
            ["browser_download_url": "https://evil.example/update.zip"],
            ["browser_download_url": "https://github.com/lljxww/krill-desktop/releases/download/v1.10.2/Krill.Desktop-1.10.2-macos-universal.zip"],
            ["browser_download_url": "http://github.com/mac-fantasy/krill-desktop/releases/download/v1.10.2/Krill.Desktop-1.10.2-macos-universal.zip"]
        ] {
            let release = try fixture { object in
                var assets = object["assets"] as! [[String: Any]]
                assets[1].merge(changes) { _, new in new }; object["assets"] = assets
            }
            XCTAssertNil(release.update(after: current, platform: "macos-universal"), "\(changes)")
        }
    }
}
