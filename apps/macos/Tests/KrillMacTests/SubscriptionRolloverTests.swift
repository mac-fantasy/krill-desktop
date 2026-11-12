import Foundation
import KrillCore
import XCTest
@testable import KrillMac

final class SubscriptionRolloverTests: XCTestCase {
    @MainActor
    func testRolloverPersistsSelectionAndUpdatesMenuAtExpiryWithoutNetwork() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("KrillRollover-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let storage = try LocalStorage(demo: true, directory: directory)
        let store = DashboardStore(storage: storage, demo: true)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("../../../../contracts/fixtures/subscription-selection.json").standardizedFileURL)
        var response = try XCTUnwrap(KrillJSON.decoder().decode(APIEnvelope<SubscriptionResponse>.self, from: data).data)
        let now = try XCTUnwrap(ISO8601DateFormatter().date(from: "2099-10-20T00:00:00Z"))
        store.preferences.selectedSubscriptionID = 201
        store.applySubscriptions(response, at: now)
        XCTAssertEqual(store.selectedPlan?.id, 205)
        XCTAssertEqual(storage.loadPreferences().selectedSubscriptionID, 205)
        XCTAssertEqual(store.primaryRemaining, "$80.00")
        XCTAssertEqual(store.menuText, "80%")
        XCTAssertFalse(store.activePlans.contains { [201, 202, 203, 204, 207, 210].contains($0.id) })
        // A quota reset on the previous card must not undo the automatic switch.
        let old = try XCTUnwrap(response.subscriptions.firstIndex { $0.id == 201 })
        response.subscriptions[old].quota.remainingUsd = 100
        store.applySubscriptions(response, at: now.addingTimeInterval(1))
        XCTAssertEqual(store.selectedPlan?.id, 205)
        store.advanceSubscriptionTime(at: now.addingTimeInterval(10))
        XCTAssertEqual(store.selectedPlan?.id, 201)
        XCTAssertEqual(storage.loadPreferences().selectedSubscriptionID, 201)
        XCTAssertEqual(store.primaryRemaining, "$100.00")
        XCTAssertEqual(store.plans.first { $0.id == 205 }?.availability(at: store.subscriptionDate), .expired)
    }

    @MainActor
    func testOmittedExpiredMonthlyCardStillSwitchesToMonthlyBeforeDaily() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("KrillRollover-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = DashboardStore(storage: try LocalStorage(demo: true, directory: directory), demo: true)
        let now = Date()
        var quota = Quota(); quota.dailyLimitUsd = 100; quota.remainingUsd = 50
        let old = Subscription(id: 301, status: "active", plan: Plan(billingType: "usd_monthly"), quota: quota, end: now.addingTimeInterval(10), priority: 0)
        let monthly = Subscription(id: 302, status: "active", plan: Plan(billingType: "usd_monthly"), quota: quota, end: nil, priority: 10)
        let daily = Subscription(id: 303, status: "active", plan: Plan(billingType: "usd_daily"), quota: quota, end: nil, priority: 1)
        store.preferences.selectedSubscriptionID = old.id
        store.applySubscriptions(SubscriptionResponse(subscriptions: [old, monthly, daily], creditBalanceUsd: nil, welfareBalanceUsd: nil), at: now)
        store.applySubscriptions(SubscriptionResponse(subscriptions: [monthly, daily], creditBalanceUsd: nil, welfareBalanceUsd: nil), at: now.addingTimeInterval(10))
        XCTAssertEqual(store.selectedPlan?.id, monthly.id)
    }
}
