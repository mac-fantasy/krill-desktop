import Foundation

public enum SubscriptionAvailability: Sendable, Equatable {
    case available, notStarted, expired, exhausted, frozen, inactive
    public var label: String {
        switch self {
        case .available: "使用中"
        case .notStarted: "未生效"
        case .expired: "已到期"
        case .exhausted: "额度已用尽"
        case .frozen: "已冻结"
        case .inactive: "已结束 / 不可用"
        }
    }
}

public extension Subscription {
    // Monthly billing, or a calendar-month-duration card with daily/weekly/request quotas.
    var isMonthly: Bool { plan.billingType == "usd_monthly" || plan.durationDays.map { (28...31).contains($0) } == true }

    func availability(at now: Date = Date()) -> SubscriptionAvailability {
        if status == "expired" || subscriptionEndAt.map({ $0 <= now }) == true { return .expired }
        if status == "frozen" { return .frozen }
        if status == "exhausted" { return .exhausted }
        if subscriptionStartAt.map({ $0 > now }) == true { return .notStarted }
        guard isActive(at: now) else { return .inactive }
        let quota = QuotaProjection(self)
        if isMonthly, let remaining = quota.remaining, remaining <= 0,
           !(quota.isMoney && (quota.forwarded ?? 0) > 0) { return .exhausted }
        return .available
    }
}

public enum SubscriptionSelection {
    public static func ordered(_ subscriptions: [Subscription]) -> [Subscription] {
        subscriptions.sorted { $0.consumePriority == $1.consumePriority ? $0.id < $1.id : $0.consumePriority < $1.consumePriority }
    }

    public static func select(_ subscriptions: [Subscription], preferredID: Int64?, preferredWasMonthly: Bool = false,
                              at now: Date = Date()) -> Subscription? {
        let available = ordered(subscriptions).filter { $0.availability(at: now) == .available }
        if let preferred = available.first(where: { $0.id == preferredID }) { return preferred }
        if preferredWasMonthly || subscriptions.first(where: { $0.id == preferredID })?.isMonthly == true,
           let monthly = available.first(where: { $0.isMonthly }) { return monthly }
        return available.first
    }

    public static func nextTransition(in subscriptions: [Subscription], after now: Date) -> Date? {
        subscriptions.flatMap { [$0.subscriptionStartAt, $0.subscriptionEndAt].compactMap { $0 } }.filter { $0 > now }.min()
    }
}
