import Foundation

public enum KrillJSON {
    public static func decoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .custom { value in
            let text = try value.singleValueContainer().decode(String.self)
            let fractional = ISO8601DateFormatter()
            fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            if let date = fractional.date(from: text) ?? ISO8601DateFormatter().date(from: text) { return date }
            throw DecodingError.dataCorrupted(.init(codingPath: value.codingPath, debugDescription: "Invalid ISO8601 date"))
        }
        return decoder
    }
}

extension KeyedDecodingContainer {
    func decimal(_ key: Key) throws -> Decimal? {
        guard contains(key) else { return nil }
        if try decodeNil(forKey: key) { return nil }
        if let number = try? decode(Decimal.self, forKey: key) { return number }
        let text = try decode(String.self, forKey: key)
        guard text.range(of: #"^-?[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$"#, options: .regularExpression) != nil,
              let number = Decimal(string: text, locale: Locale(identifier: "en_US_POSIX")), !number.isNaN else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "Invalid decimal")
        }
        return number
    }
    func integer(_ key: Key) throws -> Int64? {
        guard contains(key) else { return nil }
        if try decodeNil(forKey: key) { return nil }
        if let number = try? decode(Int64.self, forKey: key) { return number }
        let text = try decode(String.self, forKey: key)
        guard let number = Int64(text) else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "Invalid integer")
        }
        return number
    }
}

public struct APIEnvelope<T: Decodable & Sendable>: Decodable, Sendable {
    public let success: Bool
    public let code: Int?
    public let data: T?
}

public struct UserProfile: Decodable, Sendable {
    public var email: String?
    public var username: String?
    public var name: String?
    public var nickname: String?
    public var displayName: String?
    public var friendlyName: String {
        [displayName, nickname, name, username, email?.components(separatedBy: "@").first]
            .compactMap { $0 }.first { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } ?? "Krill 用户"
    }
}

public struct SubscriptionResponse: Decodable, Sendable {
    public var requestCountQuota: RequestCountQuota?
    public var subscriptions: [Subscription]
    public var creditBalanceUsd: Decimal?
    public var welfareBalanceUsd: Decimal?
    enum CodingKeys: String, CodingKey { case subscriptions, creditBalanceUsd, welfareBalanceUsd, requestCountQuota }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        subscriptions = try c.decodeIfPresent([Subscription].self, forKey: .subscriptions) ?? []
        creditBalanceUsd = try c.decimal(.creditBalanceUsd)
        welfareBalanceUsd = try c.decimal(.welfareBalanceUsd)
        requestCountQuota = try c.decodeIfPresent(RequestCountQuota.self, forKey: .requestCountQuota)
    }
    public init(subscriptions: [Subscription], creditBalanceUsd: Decimal?, welfareBalanceUsd: Decimal?, requestCountQuota: RequestCountQuota? = nil) {
        self.subscriptions = subscriptions; self.creditBalanceUsd = creditBalanceUsd; self.welfareBalanceUsd = welfareBalanceUsd
        self.requestCountQuota = requestCountQuota
    }
}

public struct Subscription: Decodable, Identifiable, Sendable {
    public var subscriptionId: Int64
    public var status: String?
    public var plan: Plan
    public var quota: Quota
    public var subscriptionStartAt: Date?
    public var subscriptionEndAt: Date?
    public var consumePriority: Int64
    public var id: Int64 { subscriptionId }
    public func isActive(at now: Date = Date()) -> Bool {
        status == "active" && (subscriptionStartAt.map { $0 <= now } ?? true) && (subscriptionEndAt.map { $0 > now } ?? true)
    }
    enum CodingKeys: String, CodingKey { case subscriptionId, status, plan, quota, subscriptionStartAt, subscriptionEndAt, consumePriority }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        subscriptionId = try c.integer(.subscriptionId) ?? 0
        status = try c.decodeIfPresent(String.self, forKey: .status)
        plan = try c.decodeIfPresent(Plan.self, forKey: .plan) ?? Plan()
        quota = try c.decodeIfPresent(Quota.self, forKey: .quota) ?? Quota()
        subscriptionStartAt = try c.decodeIfPresent(Date.self, forKey: .subscriptionStartAt)
        subscriptionEndAt = try c.decodeIfPresent(Date.self, forKey: .subscriptionEndAt)
        consumePriority = try c.integer(.consumePriority) ?? 0
    }
    public init(id: Int64, status: String, plan: Plan, quota: Quota, end: Date?, priority: Int64, start: Date? = nil) {
        subscriptionId = id; self.status = status; self.plan = plan; self.quota = quota
        subscriptionStartAt = start; subscriptionEndAt = end; consumePriority = priority
    }
}

public struct Plan: Decodable, Sendable {
    public var rateLimit5h: Int64?
    public var rateLimitWeekly: Int64?
    public var rateLimitMonthly: Int64?
    public var name = "Krill 套餐"
    public var billingType: String?
    public var totalCredits: Decimal?
    public var durationDays: Int64?
    enum CodingKeys: String, CodingKey { case name, billingType, totalCredits, durationDays, rateLimit5h = "rateLimit5H", rateLimitWeekly, rateLimitMonthly }
    public init(name: String = "Krill 套餐", billingType: String? = nil, totalCredits: Decimal? = nil, durationDays: Int64? = nil) {
        self.name = name; self.billingType = billingType; self.totalCredits = totalCredits; self.durationDays = durationDays
    }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decodeIfPresent(String.self, forKey: .name) ?? "Krill 套餐"
        billingType = try c.decodeIfPresent(String.self, forKey: .billingType)
        totalCredits = try c.decimal(.totalCredits)
        durationDays = try c.integer(.durationDays)
        rateLimit5h = try c.integer(.rateLimit5h); rateLimitWeekly = try c.integer(.rateLimitWeekly); rateLimitMonthly = try c.integer(.rateLimitMonthly)
    }
}

public struct Quota: Decodable, Sendable {
    public var dailyLimitUsd: Decimal?
    public var usedUsd: Decimal?
    public var remainingUsd: Decimal?
    public var forwardedRemainingUsd: Decimal?
    public var windowResetAt: Date?
    public var limitCredits: Decimal?
    public var usedCredits: Decimal?
    public var remainingCredits: Decimal?
    public var limitRequests: Int64?
    public var usedRequests: Int64?
    public var remainingRequests: Int64?
    enum CodingKeys: String, CodingKey {
        case dailyLimitUsd, usedUsd, remainingUsd, forwardedRemainingUsd, windowResetAt
        case limitCredits, usedCredits, remainingCredits, limitRequests, usedRequests, remainingRequests
    }
    public init() {}
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        dailyLimitUsd = try c.decimal(.dailyLimitUsd); usedUsd = try c.decimal(.usedUsd)
        remainingUsd = try c.decimal(.remainingUsd); forwardedRemainingUsd = try c.decimal(.forwardedRemainingUsd)
        windowResetAt = try c.decodeIfPresent(Date.self, forKey: .windowResetAt)
        limitCredits = try c.decimal(.limitCredits); usedCredits = try c.decimal(.usedCredits)
        remainingCredits = try c.decimal(.remainingCredits)
        limitRequests = try c.integer(.limitRequests); usedRequests = try c.integer(.usedRequests)
        remainingRequests = try c.integer(.remainingRequests)
    }
}

public struct RequestStats: Decodable, Sendable {
    public var failedRequests: Int64?
    public var cacheReadInputTokens: Int64?
    public var cacheCreationInputTokens: Int64?
    public var cacheRateEligibleTokens: Int64?
    public var cacheRateReadTokens: Int64?
    public var reasoningTokens: Int64?
    public var rpm: Decimal?
    public var tpm: Decimal?
    public var channelCacheRates: [ChannelCacheRate] = []
    public var totalRequests: Int64
    public var successRequests: Int64
    public var totalTokens: Int64
    public var inputTokens: Int64?
    public var outputTokens: Int64
    public var totalCostUsd: Decimal?
    public var trend: [TrendBucket]
    enum CodingKeys: String, CodingKey {
        case totalRequests, successRequests, failedRequests, totalTokens, inputTokens, outputTokens, totalCostUsd, trend
        case cacheReadInputTokens, cacheCreationInputTokens, cacheRateEligibleTokens, cacheRateReadTokens, reasoningTokens, rpm, tpm, channelCacheRates
    }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        totalRequests = try c.integer(.totalRequests) ?? 0; successRequests = try c.integer(.successRequests) ?? 0
        totalTokens = try c.integer(.totalTokens) ?? 0; inputTokens = try c.integer(.inputTokens)
        outputTokens = try c.integer(.outputTokens) ?? 0; totalCostUsd = try c.decimal(.totalCostUsd)
        trend = try c.decodeIfPresent([TrendBucket].self, forKey: .trend) ?? []
        failedRequests = try c.integer(.failedRequests)
        cacheReadInputTokens = try c.integer(.cacheReadInputTokens); cacheCreationInputTokens = try c.integer(.cacheCreationInputTokens)
        cacheRateEligibleTokens = try c.integer(.cacheRateEligibleTokens); cacheRateReadTokens = try c.integer(.cacheRateReadTokens)
        reasoningTokens = try c.integer(.reasoningTokens); rpm = try c.decimal(.rpm); tpm = try c.decimal(.tpm)
        channelCacheRates = try c.decodeIfPresent([ChannelCacheRate].self, forKey: .channelCacheRates) ?? []
    }
    public init(requests: Int64, successes: Int64, tokens: Int64, input: Int64, output: Int64, cost: Decimal, trend: [TrendBucket]) {
        totalRequests = requests; successRequests = successes; totalTokens = tokens
        inputTokens = input; outputTokens = output; totalCostUsd = cost; self.trend = trend
    }
}

public struct TrendBucket: Decodable, Sendable {
    public var bucketStart: Date
    public var requestCount: Int64
    public var totalCostUsd: Decimal?
    enum CodingKeys: String, CodingKey { case bucketStart, requestCount, totalCostUsd }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        bucketStart = try c.decode(Date.self, forKey: .bucketStart)
        requestCount = try c.integer(.requestCount) ?? 0; totalCostUsd = try c.decimal(.totalCostUsd)
    }
    public init(start: Date, requests: Int64, cost: Decimal) { bucketStart = start; requestCount = requests; totalCostUsd = cost }
}

public struct AnnouncementsResponse: Decodable, Sendable {
    public var items: [Announcement]
    public init(items: [Announcement]) { self.items = items }
}

public struct Announcement: Decodable, Identifiable, Sendable {
    public var id: Int64
    public var title: String
    public var summary: String?
    public var content: String?
    public var createdAt: Date?
    enum CodingKeys: String, CodingKey { case id, title, summary, content, createdAt }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.integer(.id) ?? 0; title = try c.decodeIfPresent(String.self, forKey: .title) ?? "公告"
        summary = try c.decodeIfPresent(String.self, forKey: .summary)
        content = try c.decodeIfPresent(String.self, forKey: .content)
        createdAt = try c.decodeIfPresent(Date.self, forKey: .createdAt)
    }
    public init(id: Int64, title: String, summary: String, date: Date) {
        self.id = id; self.title = title; self.summary = summary; createdAt = date
    }
}
