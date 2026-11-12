import Foundation

public struct RequestQuotaWindow: Sendable, Identifiable {
    public let label: String
    public let used: Int64?
    public let limit: Int64?
    public var id: String { label }
    public var remaining: Decimal? {
        guard let limit, limit > 0, let used, used >= 0 else { return nil }
        return max(0, Decimal(limit) - Decimal(used))
    }
    public var percent: Double? {
        guard let remaining, let limit else { return nil }
        return NSDecimalNumber(decimal: remaining / Decimal(limit) * 100).doubleValue
    }
    public var detail: String {
        let usedText = used.flatMap { $0 >= 0 ? DisplayFormat.number(Decimal($0), digits: 0) : nil } ?? "—"
        let limitText = limit.flatMap { $0 > 0 ? DisplayFormat.number(Decimal($0), digits: 0) : nil } ?? "—"
        return "已用 \(usedText) / \(limitText) 次"
    }
    public init(label: String, used: Int64?, limit: Int64?) { self.label = label; self.used = used; self.limit = limit }
}

public extension RequestCountQuota {
    var windows: [RequestQuotaWindow] { [
        RequestQuotaWindow(label: "5 小时", used: used5h, limit: limit5h),
        RequestQuotaWindow(label: "本周", used: usedWeekly, limit: limitWeekly),
        RequestQuotaWindow(label: "本月", used: usedMonthly, limit: limitMonthly)
    ] }
    var cycle: String { "周期 \(DisplayFormat.plainText(cycleStart ?? "—", limit: 40)) ~ \(DisplayFormat.plainText(cycleEnd ?? "—", limit: 40))" }
}

public extension RequestStats {
    var cachePercent: Decimal? {
        let read: Decimal, eligible: Decimal
        if let dedicatedRead = cacheRateReadTokens, let dedicatedEligible = cacheRateEligibleTokens {
            read = Decimal(dedicatedRead); eligible = Decimal(dedicatedEligible)
        } else if let cached = cacheReadInputTokens, let written = cacheCreationInputTokens, let inputTokens, cached >= 0, written >= 0, inputTokens >= 0 {
            read = Decimal(cached); eligible = Decimal(inputTokens) + Decimal(written) + read
        } else { return nil }
        guard read >= 0, eligible > 0, read <= eligible else { return nil }
        return read / eligible * 100
    }
}

public enum ResetCardState: Sendable, Equatable {
    case unused, used, revoked, expired, unknown
    public var label: String {
        switch self { case .unused: "未使用"; case .used: "已使用"; case .revoked: "已作废"; case .expired: "已过期"; case .unknown: "状态未知" }
    }
}

public extension ResetCard {
    func state(at now: Date = Date()) -> ResetCardState {
        if status == 1 { return .used }; if status == 2 { return .revoked }
        if expired == true || expiresAt.map({ $0 <= now }) == true { return .expired }
        return status == 0 ? .unused : .unknown
    }
    func canUse(at now: Date = Date()) -> Bool { state(at: now) == .unused && (usableSubscriptions?.isEmpty == false) }
    func availabilityText(at now: Date = Date()) -> String {
        guard state(at: now) == .unused else { return state(at: now).label }
        guard let usableSubscriptions else { return "未使用 · 适用订阅未知" }
        return usableSubscriptions.isEmpty ? "未使用 · 暂无适用订阅" : "可用 · \(usableSubscriptions.count) 个订阅"
    }
}

public extension DisplayFormat {
    static func percentage(_ value: Decimal?) -> String {
        value.flatMap { (0...100).contains($0) ? number($0, digits: 1) + "%" : nil } ?? "—"
    }
    static func optionalCount(_ value: Int64?) -> String { value.flatMap { $0 >= 0 ? number(Decimal($0), digits: 0) : nil } ?? "—" }
    static func rate(_ value: Decimal?) -> String { value.flatMap { $0 >= 0 ? number($0, digits: 1) : nil } ?? "—" }
}
