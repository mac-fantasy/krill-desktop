import Foundation
import XCTest
@testable import KrillCore

final class ProtocolTests: XCTestCase {
    private func fixture(_ name: String) throws -> Data {
        // Both platforms consume these exact synthetic fixtures from the repository root.
        let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        return try Data(contentsOf: directory.appendingPathComponent("../../../../contracts/fixtures/" + name).standardizedFileURL)
    }
    func testSharedSubscriptionContract() throws {
        let value = try KrillJSON.decoder().decode(APIEnvelope<SubscriptionResponse>.self, from: fixture("subscriptions.json"))
        let data = try XCTUnwrap(value.data)
        XCTAssertTrue(value.success)
        XCTAssertEqual(data.creditBalanceUsd, Decimal(string: "24.80"))
        XCTAssertEqual(data.welfareBalanceUsd, Decimal(string: "3.50"))
        let weekly = QuotaProjection(data.subscriptions[0])
        XCTAssertEqual(weekly.cycle, "本周额度")
        XCTAssertEqual(weekly.limit, 100); XCTAssertEqual(weekly.remaining, Decimal(string: "72.6"))
        XCTAssertEqual(weekly.percent, 72.6); XCTAssertEqual(weekly.forwarded, Decimal(string: "8.20"))
        XCTAssertEqual(weekly.amount(weekly.remaining), "$72.60")
        XCTAssertNotNil(weekly.resetAt)
        XCTAssertEqual(QuotaProjection(data.subscriptions[1]).percent, 75)
        XCTAssertEqual(QuotaProjection(data.subscriptions[2]).percent, 80)
        XCTAssertNil(QuotaProjection(data.subscriptions[3]).percent)
        XCTAssertEqual(QuotaProjection(data.subscriptions[3]).amount(nil), "—")
    }
    func testSharedStatsContract() throws {
        let stats = try XCTUnwrap(KrillJSON.decoder().decode(APIEnvelope<RequestStats>.self, from: fixture("stats.json")).data)
        XCTAssertEqual(stats.totalTokens, 1_284_600); XCTAssertEqual(stats.totalRequests, 128)
        XCTAssertEqual(DisplayFormat.cost(stats.totalCostUsd), "$0.0007")
        XCTAssertEqual(stats.trend.first?.requestCount, 9)
    }
    func testDemoStatsPreserveHourlyTrend() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let stats = DemoData.stats(now: now)
        let start = Calendar.current.startOfDay(for: now)
        XCTAssertEqual(stats.totalRequests, 128)
        XCTAssertEqual(stats.totalCostUsd, Decimal(string: "7.42"))
        XCTAssertEqual(stats.trend.count, 24)
        XCTAssertEqual(stats.trend.first?.bucketStart, start)
        XCTAssertEqual(stats.trend.first?.requestCount, 3)
        XCTAssertEqual(stats.trend.first?.totalCostUsd, Decimal(string: "0.5"))
        XCTAssertEqual(stats.trend.last?.bucketStart, start.addingTimeInterval(23 * 3600))
        XCTAssertEqual(stats.trend.last?.requestCount, 12)
        XCTAssertEqual(stats.trend.last?.totalCostUsd, Decimal(string: "1.4"))
    }
    func testMissingAndZeroQuotaStayUnknownAndPercentIsClamped() {
        var q = Quota(); q.dailyLimitUsd = 0; q.remainingUsd = 0
        func projection(_ quota: Quota) -> QuotaProjection {
            QuotaProjection(Subscription(id: 1, status: "active", plan: Plan(), quota: quota, end: nil, priority: 0))
        }
        XCTAssertNil(projection(q).percent)
        q.dailyLimitUsd = 100; q.remainingUsd = nil; XCTAssertNil(projection(q).percent)
        q.remainingUsd = -10; XCTAssertEqual(projection(q).percent, 0)
        q.remainingUsd = 110; XCTAssertEqual(projection(q).percent, 100)
    }
    func testExactOriginAndLoginNavigation() {
        XCTAssertTrue(KrillOrigin.isTrusted(URL(string: "https://www.krill-code.com:443/app")))
        for text in ["http://www.krill-code.com/app", "https://www.krill-code.com.evil.example", "https://krill-code.com", "https://www.krill-code.com:444", "https://user@www.krill-code.com"] {
            XCTAssertFalse(KrillOrigin.isTrusted(URL(string: text)), text)
        }
        for text in ["https://github.com/login", "https://appleid.apple.com/auth/authorize", "https://accounts.google.com", "https://connect.linux.do/oauth2/authorize", "https://sso.example.org/login"] {
            XCTAssertTrue(KrillOrigin.allowsLoginNavigation(URL(string: text)), text)
            XCTAssertFalse(KrillOrigin.isTrusted(URL(string: text)), text)
        }
        for text in ["http://github.com", "javascript:alert(1)", "file:///tmp/login.html", "https://user:password@github.com", "https://github.com:444", "about:blank"] {
            XCTAssertFalse(KrillOrigin.allowsLoginNavigation(URL(string: text)), text)
        }
        XCTAssertFalse(KrillOrigin.allowsLoginNavigation(nil))
    }
    func testBrowserSessionImportRejectsURLsScriptsAndHeaderInjection() {
        let token = "syntheticxx.payloadxx.signaturexx"
        XCTAssertEqual(BrowserLogin.session(from: " \n" + token + "\r\n"), token)
        XCTAssertEqual(BrowserLogin.session(from: "Bearer " + token), token)
        for text in ["", "null", "undefined", "\"" + token + "\"", "https://www.krill-code.com/#token=" + token,
                     token + "\r\nX-Test: injected", BrowserLogin.exportScript, String(repeating: "a", count: 16401)] {
            XCTAssertNil(BrowserLogin.session(from: text))
        }
    }
    func testSessionShapeAndRequestCredentials() throws {
        let token = "samplexx.payloadxx.signaturexx"
        XCTAssertTrue(KrillOrigin.isPlausibleSession(token))
        XCTAssertFalse(KrillOrigin.isPlausibleSession(token + "\r\nX-Test: injected"))
        let request = try KrillAPI.request(.subscriptions, token: token)
        XCTAssertEqual(request.url?.absoluteString, "https://www.krill-code.com/api/subscription")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer " + token)
        XCTAssertEqual(request.value(forHTTPHeaderField: "Cookie"), "krill_jwt=" + token)
        let publicRequest = try KrillAPI.request(.announcements)
        XCTAssertNil(publicRequest.value(forHTTPHeaderField: "Authorization"))
        XCTAssertNil(publicRequest.value(forHTTPHeaderField: "Cookie"))
    }
    func testLocalDayUsesMidnightOffsetAtDSTTransition() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try XCTUnwrap(TimeZone(identifier: "America/Los_Angeles"))
        let now = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-08T20:00:00Z"))
        let expected = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-08T08:00:00Z"))
        XCTAssertEqual(DisplayFormat.dayRange(now: now, calendar: calendar).from, expected)
    }
    func testAnnouncementIsBoundedPlainText() {
        let text = DisplayFormat.plainText("<script>doBadThing()</script><p>Hello &amp; welcome</p>", limit: 80)
        XCTAssertEqual(text, "Hello & welcome")
        XCTAssertEqual(DisplayFormat.plainText(String(repeating: "a", count: 200)).count, 181)
    }
    func testMalformedAmountsAreRejected() {
        XCTAssertThrowsError(try KrillJSON.decoder().decode(SubscriptionResponse.self, from: Data(#"{"credit_balance_usd":"24.8oops"}"#.utf8)))
    }
}
