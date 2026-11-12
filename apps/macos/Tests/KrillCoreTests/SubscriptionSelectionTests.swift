import Foundation
import XCTest
@testable import KrillCore

final class SubscriptionSelectionTests: XCTestCase {
    private func fixture() throws -> (SubscriptionResponse, Date) {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("../../../../contracts/fixtures/subscription-selection.json").standardizedFileURL)
        let response = try XCTUnwrap(KrillJSON.decoder().decode(APIEnvelope<SubscriptionResponse>.self, from: data).data)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let date = try XCTUnwrap(ISO8601DateFormatter().date(from: try XCTUnwrap(json["evaluation_time"] as? String)))
        return (response, date)
    }
    func testMonthlyRolloverPrefersEffectiveMonthlyCardAndKeepsUsableSelection() throws {
        let (response, now) = try fixture(); let plans = response.subscriptions
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 201, at: now)?.id, 205)
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 203, at: now)?.id, 205)
        XCTAssertEqual(SubscriptionSelection.select(Array(plans.reversed()), preferredID: 201, at: now)?.id, 205)
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 100, at: now)?.id, 100)
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 206, at: now)?.id, 206)
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: nil, at: now)?.id, 100)
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 999, preferredWasMonthly: true, at: now)?.id, 205)
        XCTAssertEqual(SubscriptionSelection.nextTransition(in: plans, after: now), now.addingTimeInterval(10))
        XCTAssertEqual(SubscriptionSelection.select(plans, preferredID: 205, at: now.addingTimeInterval(10))?.id, 202)
    }
    func testUnusableCardsAreDistinguishedWithoutTreatingUnknownOrCarryoverAsEmpty() throws {
        let (response, now) = try fixture()
        let expected: [Int64: SubscriptionAvailability] = [100: .available, 201: .exhausted, 202: .notStarted, 203: .expired,
            204: .frozen, 205: .available, 206: .available, 207: .exhausted, 208: .available, 209: .available, 210: .exhausted]
        for plan in response.subscriptions { XCTAssertEqual(plan.availability(at: now), expected[plan.id], plan.plan.name) }
        for id: Int64 in [208, 209] { XCTAssertEqual(SubscriptionSelection.select(response.subscriptions, preferredID: id, at: now)?.id, id) }
        let unavailable = response.subscriptions.filter { [201, 202, 203, 204, 207, 210].contains($0.id) }
        XCTAssertNil(SubscriptionSelection.select(unavailable, preferredID: 201, at: now))
        XCTAssertEqual(SubscriptionSelection.select(unavailable + [response.subscriptions[0]], preferredID: 201, at: now)?.id, 100)
        var daily = response.subscriptions[0]; daily.quota.remainingUsd = 0
        XCTAssertEqual(daily.availability(at: now), .available)
        daily.plan.durationDays = 30
        XCTAssertTrue(daily.isMonthly); XCTAssertEqual(daily.availability(at: now), .exhausted)
    }
}
