import CryptoKit
import Foundation
import KrillCore
import XCTest
@testable import KrillMac

final class UpdateServiceTests: XCTestCase {
    private func run(_ executable: String, _ arguments: [String]) throws {
        let process = Process(); process.executableURL = URL(fileURLWithPath: executable); process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
        try process.run(); process.waitUntilExit(); XCTAssertEqual(process.terminationStatus, 0)
    }
    func testPrepareVerifiesDigestVersionAndSignedBundleBeforeReplacement() throws {
        // Use a signed copy of the system's no-op executable; this test never launches an app.
        let fm = FileManager.default
        let root = fm.temporaryDirectory.appendingPathComponent("Krill update '\(UUID().uuidString)").resolvingSymlinksInPath()
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        let current = root.appendingPathComponent("Installed.app")
        try fm.createDirectory(at: current, withIntermediateDirectories: true)
        let payload = root.appendingPathComponent("Krill Desktop.app")
        let contents = payload.appendingPathComponent("Contents")
        try fm.createDirectory(at: contents.appendingPathComponent("MacOS"), withIntermediateDirectories: true)
        try fm.copyItem(at: URL(fileURLWithPath: "/usr/bin/true"), to: contents.appendingPathComponent("MacOS/KrillMac"))
        let plist: [String: Any] = ["CFBundleIdentifier": "com.krill.desktop.macos", "CFBundleExecutable": "KrillMac",
                                    "CFBundlePackageType": "APPL", "CFBundleShortVersionString": "1.10.2", "CFBundleVersion": "1.10.2"]
        try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0).write(to: contents.appendingPathComponent("Info.plist"))
        try run("/usr/bin/codesign", ["--force", "--sign", "-", payload.path])
        let archive = root.appendingPathComponent("update.zip")
        try run("/usr/bin/ditto", ["-c", "-k", "--keepParent", payload.path, archive.path])
        let data = try Data(contentsOf: archive)
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        func asset(digest: String, size: Int = data.count) throws -> GitHubRelease.Asset {
            try JSONDecoder().decode(GitHubRelease.Asset.self, from: JSONSerialization.data(withJSONObject: [
                "name": "update.zip", "browser_download_url": "https://github.com/update.zip", "size": size, "digest": "sha256:" + digest
            ]))
        }
        XCTAssertThrowsError(try UpdateService.prepare(archive: archive, asset: asset(digest: String(repeating: "0", count: 64)), version: "1.10.2", appURL: current))
        XCTAssertThrowsError(try UpdateService.prepare(archive: archive, asset: asset(digest: digest, size: 1), version: "1.10.2", appURL: current))
        XCTAssertThrowsError(try UpdateService.prepare(archive: archive, asset: asset(digest: digest), version: "1.10.3", appURL: current))
        let staging = try UpdateService.prepare(archive: archive, asset: asset(digest: digest), version: "1.10.2", appURL: current)
        XCTAssertTrue(fm.fileExists(atPath: staging.appendingPathComponent("Krill Desktop.app/Contents/MacOS/KrillMac").path))
        XCTAssertTrue(fm.fileExists(atPath: current.path))
        XCTAssertEqual(try fm.contentsOfDirectory(atPath: current.path), [])
    }
}
