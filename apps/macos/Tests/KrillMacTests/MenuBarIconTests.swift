import AppKit
import XCTest
@testable import KrillMac

final class MenuBarIconTests: XCTestCase {
    @MainActor
    func testProgressFillsClockwiseAndKeepsLogo() async throws {
        let empty = try alpha(percent: 0)
        let quarter = try alpha(percent: 25)
        let full = try alpha(percent: 100)
        XCTAssertGreaterThan(quarter[22 * 44 + 40], empty[22 * 44 + 40])
        XCTAssertEqual(quarter[22 * 44 + 3], empty[22 * 44 + 3])
        XCTAssertGreaterThan(full[22 * 44 + 3], quarter[22 * 44 + 3])
        let center = (10..<34).flatMap { y in (12..<32).map { x in y * 44 + x } }
        XCTAssertGreaterThan(center.filter { empty[$0] > 0.5 }.count, 50)
        XCTAssertEqual(center.map { empty[$0] }, center.map { full[$0] })
    }

    @MainActor
    func testUnknownAndOutOfRangeProgress() async throws {
        let empty = try alpha(percent: 0)
        for percent in [Double?.none, .some(.nan), .some(.infinity), .some(-.infinity), .some(-10)] {
            XCTAssertEqual(try alpha(percent: percent), empty)
        }
        XCTAssertEqual(try alpha(percent: 120), try alpha(percent: 100))
    }

    @MainActor
    func testLoginAndStaleBadgesStayVisible() async throws {
        XCTAssertEqual(try alpha(percent: 100, needsLogin: true), try alpha(percent: nil, needsLogin: true))
        XCTAssertNotEqual(try alpha(percent: nil, needsLogin: true), try alpha(percent: nil))
        XCTAssertNotEqual(try alpha(percent: 100, isStale: true), try alpha(percent: 100))
        XCTAssertNotEqual(try alpha(percent: nil, needsLogin: true), try alpha(percent: nil, isStale: true))
    }

    @MainActor
    private func alpha(percent: Double?, needsLogin: Bool = false, isStale: Bool = false) throws -> [CGFloat] {
        let image = MenuBarIcon.image(percent: percent, needsLogin: needsLogin, isStale: isStale)
        XCTAssertTrue(image.isTemplate)
        XCTAssertEqual(image.size, NSSize(width: 22, height: 22))
        let context = try XCTUnwrap(CGContext(data: nil, width: 44, height: 44, bitsPerComponent: 8, bytesPerRow: 0,
                                            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.scaleBy(x: 2, y: 2)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
        image.draw(in: NSRect(origin: .zero, size: image.size))
        NSGraphicsContext.restoreGraphicsState()
        let bitmap = NSBitmapImageRep(cgImage: try XCTUnwrap(context.makeImage()))
        return try (0..<44).flatMap { y in
            try (0..<44).map { x in try XCTUnwrap(bitmap.colorAt(x: x, y: y)).alphaComponent }
        }
    }
}
