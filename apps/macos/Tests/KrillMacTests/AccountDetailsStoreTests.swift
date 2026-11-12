import AppKit
import Foundation
import KrillCore
import SwiftUI
import XCTest
@testable import KrillMac

final class AccountDetailsStoreTests: XCTestCase {
    @MainActor
    func testResetCardExpiryAndSignOutClearOnlyCurrentAccountData() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("KrillDetails-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = DashboardStore(storage: try LocalStorage(demo: true, directory: directory), demo: true)
        let now = Date()
        store.applyResetCards(DemoData.resetCards(now: now), at: now)
        XCTAssertEqual(store.resetCardsSummary, "1 张可用 · 2 张记录")
        store.advanceResetCardTime(at: now.addingTimeInterval(2 * 86400))
        XCTAssertEqual(store.resetCardsSummary, "0 张可用 · 2 张记录")
        store.signOut()
        XCTAssertTrue(store.resetCards.isEmpty); XCTAssertFalse(store.resetCardsLoaded)
        XCTAssertNil(store.stats); XCTAssertNil(store.subscriptions)
        XCTAssertEqual(store.resetCardsSummary, "—")
    }
    @MainActor
    func testRequestCardSelectionUpdatesMenuAndRendersDetails() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("KrillDetails-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = DashboardStore(storage: try LocalStorage(demo: true, directory: directory), demo: true)
        store.preferences.selectedSubscriptionID = 3
        XCTAssertEqual(store.percent, 77); XCTAssertEqual(store.primaryRemaining, "77 次")
        XCTAssertEqual(store.menuText, "77%")
        guard let path = ProcessInfo.processInfo.environment["KRILL_DETAILS_SCREENSHOTS"] else { return }
        NSApplication.shared.setActivationPolicy(.accessory)
        try FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 1150), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentViewController = NSHostingController(rootView: DashboardView(store: store))
        window.setContentSize(NSSize(width: 500, height: 1000))
        defer { window.close() }
        window.makeKeyAndOrderFront(nil)
        for tab in [DashboardTab.overview, .usage, .plans] {
            store.tab = tab
            try await Task.sleep(for: .milliseconds(250))
            let process = Process(); process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            process.arguments = ["-x", "-l", String(window.windowNumber), path + "/" + tab.rawValue + ".png"]
            try process.run(); process.waitUntilExit(); XCTAssertEqual(process.terminationStatus, 0)
            if tab == .plans || tab == .usage, let content = window.contentView,
               let scroll = descendants(content).compactMap({ $0 as? NSScrollView }).first,
               let document = scroll.documentView {
                document.scrollToVisible(NSRect(x: 0, y: document.bounds.maxY - 1, width: 1, height: 1))
                try await Task.sleep(for: .milliseconds(200))
                let bottomCapture = Process(); bottomCapture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
                bottomCapture.arguments = ["-x", "-l", String(window.windowNumber), path + (tab == .plans ? "/重置卡.png" : "/缓存用量.png")]
                try bottomCapture.run(); bottomCapture.waitUntilExit(); XCTAssertEqual(bottomCapture.terminationStatus, 0)
            }
        }
    }
    @MainActor
    private func descendants(_ view: NSView) -> [NSView] { view.subviews.flatMap { [$0] + descendants($0) } }
}
