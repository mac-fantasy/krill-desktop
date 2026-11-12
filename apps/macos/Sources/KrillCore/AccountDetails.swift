import Foundation

public struct ChannelCacheRate: Decodable, Sendable {
    public var channelName: String
    public var cacheRate: Decimal?
    enum CodingKeys: String, CodingKey { case channelName, cacheRate }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        channelName = try c.decodeIfPresent(String.self, forKey: .channelName) ?? "未知渠道"
        cacheRate = try c.decimal(.cacheRate)
    }
}

public struct RequestCountQuota: Decodable, Sendable {
    public var used5h: Int64?
    public var limit5h: Int64?
    public var usedWeekly: Int64?
    public var limitWeekly: Int64?
    public var usedMonthly: Int64?
    public var limitMonthly: Int64?
    public var cycleStart: String?
    public var cycleEnd: String?
    enum CodingKeys: String, CodingKey { case used5h = "used5H", limit5h = "limit5H", usedWeekly, limitWeekly, usedMonthly, limitMonthly, cycleStart, cycleEnd }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        used5h = try c.integer(.used5h); limit5h = try c.integer(.limit5h)
        usedWeekly = try c.integer(.usedWeekly); limitWeekly = try c.integer(.limitWeekly)
        usedMonthly = try c.integer(.usedMonthly); limitMonthly = try c.integer(.limitMonthly)
        cycleStart = try c.decodeIfPresent(String.self, forKey: .cycleStart); cycleEnd = try c.decodeIfPresent(String.self, forKey: .cycleEnd)
    }
}

public struct ResetCardsResponse: Decodable, Sendable {
    public var items: [ResetCard]
    public init(items: [ResetCard]) { self.items = items }
}

public struct ResetCard: Decodable, Sendable, Identifiable {
    public var id: Int64
    public var planName: String?
    public var status: Int64?
    public var expired: Bool?
    public var capUsd: Decimal?
    public var grantedUsd: Decimal?
    public var expiresAt: Date?
    public var usedAt: Date?
    public var note: String?
    public var usableSubscriptions: [ResetCardSubscription]?
    enum CodingKeys: String, CodingKey { case id, planName, status, expired, capUsd, grantedUsd, expiresAt, usedAt, note, usableSubscriptions }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.integer(.id) ?? 0; planName = try c.decodeIfPresent(String.self, forKey: .planName)
        status = try c.integer(.status); expired = try c.decodeIfPresent(Bool.self, forKey: .expired)
        capUsd = try c.decimal(.capUsd); grantedUsd = try c.decimal(.grantedUsd)
        expiresAt = try c.decodeIfPresent(Date.self, forKey: .expiresAt); usedAt = try c.decodeIfPresent(Date.self, forKey: .usedAt)
        note = try c.decodeIfPresent(String.self, forKey: .note)
        usableSubscriptions = try c.decodeIfPresent([ResetCardSubscription].self, forKey: .usableSubscriptions)
    }
}

public struct ResetCardSubscription: Decodable, Sendable {
    public var subscriptionId: Int64
    enum CodingKeys: String, CodingKey { case subscriptionId }
    public init(from decoder: Decoder) throws {
        subscriptionId = try decoder.container(keyedBy: CodingKeys.self).integer(.subscriptionId) ?? 0
    }
}
