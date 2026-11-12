import Foundation
import XCTest
@testable import KrillCore

final class AccountDetailsTests: XCTestCase {
    private func fixture<T: Decodable & Sendable>(_ name: String, as type: T.Type) throws -> T {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("../../../../contracts/fixtures/" + name).standardizedFileURL)
        return try XCTUnwrap(KrillJSON.decoder().decode(APIEnvelope<T>.self, from: data).data)
    }
    func testStatsDecodeCacheChannelsRequestsAndRates() throws {
        let stats = try fixture("stats.json", as: RequestStats.self)
        XCTAssertEqual(stats.failedRequests, 2); XCTAssertEqual(stats.cachePercent, 64)
        XCTAssertEqual(stats.cacheReadInputTokens, 640000); XCTAssertEqual(stats.cacheCreationInputTokens, 120000)
        XCTAssertEqual(stats.reasoningTokens, 48500); XCTAssertEqual(stats.rpm, Decimal(string: "1.8"))
        XCTAssertEqual(stats.tpm, 17842); XCTAssertEqual(stats.channelCacheRates.first?.cacheRate, Decimal(string: "0.64"))
        XCTAssertEqual(DisplayFormat.percentage(stats.cachePercent), "64.0%")
    }
    func testCacheRateUsesDedicatedDenominatorAndFallsBackOnlyWithCompleteCounters() throws {
        func stats(_ json: String) throws -> RequestStats { try KrillJSON.decoder().decode(RequestStats.self, from: Data(json.utf8)) }
        let fallback = try stats(#"{"input_tokens":50,"cache_creation_input_tokens":10,"cache_read_input_tokens":60}"#)
        XCTAssertEqual(fallback.cachePercent, 50)
        let dedicated = try stats(#"{"input_tokens":50,"cache_creation_input_tokens":10,"cache_read_input_tokens":60,"cache_rate_read_tokens":10,"cache_rate_eligible_tokens":100}"#)
        XCTAssertEqual(dedicated.cachePercent, 10)
        for json in [#"{}"#, #"{"cache_read_input_tokens":60,"cache_creation_input_tokens":10}"#,
            #"{"cache_rate_read_tokens":0,"cache_rate_eligible_tokens":0}"#,
            #"{"cache_rate_read_tokens":101,"cache_rate_eligible_tokens":100}"#,
            #"{"cache_rate_read_tokens":-1,"cache_rate_eligible_tokens":100}"#,
            #"{"input_tokens":50,"cache_read_input_tokens":60}"#] {
            XCTAssertNil(try stats(json).cachePercent, json)
        }
        XCTAssertEqual(try stats(#"{"cache_rate_read_tokens":0,"cache_rate_eligible_tokens":100}"#).cachePercent, 0)
        XCTAssertEqual(DisplayFormat.percentage(nil), "—")
    }
    func testRequestCardUsesSharedWindowsAndMostConstrainedRemaining() throws {
        let response = try fixture("request-count.json", as: SubscriptionResponse.self)
        let shared = try XCTUnwrap(response.requestCountQuota)
        XCTAssertEqual(shared.used5h, 95); XCTAssertEqual(shared.limit5h, 100)
        XCTAssertEqual(response.subscriptions[0].plan.rateLimit5h, 100)
        XCTAssertEqual(shared.windows.map { $0.percent }, [5, 70, 80.5])
        let quota = QuotaProjection(response.subscriptions[0], requestCountQuota: shared)
        XCTAssertEqual(quota.cycle, "按次额度 · 5 小时"); XCTAssertEqual(quota.remaining, 5); XCTAssertEqual(quota.percent, 5)
        XCTAssertEqual(shared.cycle, "周期 2099-10-01 ~ 2099-11-01")
        // Plan limits alone do not imply any known consumption or remaining quota.
        XCTAssertNil(QuotaProjection(response.subscriptions[0]).remaining)
        var incomplete = shared; incomplete.used5h = nil
        XCTAssertEqual(QuotaProjection(response.subscriptions[0], requestCountQuota: incomplete).percent, 70)
        incomplete.used5h = -1
        XCTAssertNil(incomplete.windows[0].percent)
    }
    func testResetCardsKeepHistoryAndExpireAtExactBoundary() throws {
        let response = try fixture("reset-cards.json", as: ResetCardsResponse.self)
        let now = try XCTUnwrap(ISO8601DateFormatter().date(from: "2099-10-20T00:00:00Z"))
        XCTAssertEqual(response.items.map { $0.state(at: now) }, [.unused, .used, .revoked, .expired, .unused, .unknown, .expired])
        XCTAssertEqual(response.items.filter { $0.canUse(at: now) }.map { $0.id }, [601])
        XCTAssertEqual(response.items[0].capUsd, 30); XCTAssertEqual(response.items[1].grantedUsd, Decimal(string: "12.50"))
        XCTAssertEqual(response.items[0].availabilityText(at: now), "可用 · 1 个订阅")
        XCTAssertEqual(response.items[4].availabilityText(at: now), "未使用 · 暂无适用订阅")
        XCTAssertFalse(response.items[0].canUse(at: now.addingTimeInterval(86400)))
    }
    func testResetCardRequestIsReadOnlyAndCarriesKrillCredentials() throws {
        let request = try KrillAPI.request(.resetCards, token: "syntheticxx.payloadxx.signaturexx")
        XCTAssertEqual(request.url?.absoluteString, "https://www.krill-code.com/api/reset-cards")
        XCTAssertEqual(request.httpMethod, "GET"); XCTAssertNil(request.httpBody)
        XCTAssertNotNil(request.value(forHTTPHeaderField: "Authorization"))
    }
}
