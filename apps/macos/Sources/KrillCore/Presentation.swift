import Foundation

public struct QuotaProjection: Sendable {
    public let cycle: String
    public let limit: Decimal?
    public let used: Decimal?
    public let remaining: Decimal?
    public let forwarded: Decimal?
    public let resetAt: Date?
    public let isMoney: Bool
    public let unit: String
    public var percent: Double? {
        guard let limit, limit > 0, let remaining else { return nil }
        return min(100, max(0, NSDecimalNumber(decimal: remaining / limit * 100).doubleValue))
    }
    public init(_ subscription: Subscription, requestCountQuota: RequestCountQuota? = nil) {
        let q = subscription.quota
        if subscription.plan.billingType == "request_count",
           let window = requestCountQuota?.windows.filter({ $0.percent != nil }).min(by: { ($0.percent ?? 100) < ($1.percent ?? 100) }) {
            cycle = "按次额度 · " + window.label; unit = "次"; isMoney = false; forwarded = nil; resetAt = nil
            limit = window.limit.map { Decimal($0) }; used = window.used.map { Decimal($0) }; remaining = window.remaining
            return
        }
        resetAt = q.windowResetAt
        if subscription.plan.billingType == "request_count" {
            cycle = "按次额度"; unit = "次"; isMoney = false; forwarded = nil
            limit = q.limitRequests.map { Decimal($0) }; used = q.usedRequests.map { Decimal($0) }
            remaining = q.remainingRequests.map { Decimal($0) }
        } else if subscription.plan.totalCredits != nil || q.limitCredits != nil {
            cycle = "积分额度"; unit = "积分"; isMoney = false; forwarded = nil
            limit = q.limitCredits ?? subscription.plan.totalCredits; used = q.usedCredits; remaining = q.remainingCredits
        } else {
            cycle = ["usd_daily": "本日额度", "usd_weekly": "本周额度", "usd_monthly": "本月额度"][subscription.plan.billingType ?? ""] ?? "当前周期额度"
            unit = "USD"; isMoney = true; limit = q.dailyLimitUsd; used = q.usedUsd
            remaining = q.remainingUsd; forwarded = q.forwardedRemainingUsd
        }
    }
    public func amount(_ value: Decimal?) -> String {
        isMoney ? DisplayFormat.money(value) : value.map { DisplayFormat.number($0, digits: 0) + " " + unit } ?? "—"
    }
}

public enum DisplayFormat {
    public static func number(_ value: Decimal, digits: Int = 2) -> String {
        let f = NumberFormatter(); f.locale = Locale(identifier: "en_US_POSIX")
        f.numberStyle = .decimal; f.minimumFractionDigits = digits; f.maximumFractionDigits = digits
        return f.string(from: NSDecimalNumber(decimal: value)) ?? "—"
    }
    public static func money(_ value: Decimal?) -> String { value.map { "$" + number($0) } ?? "—" }
    public static func cost(_ value: Decimal?) -> String { value.map { "$" + number($0, digits: $0 > 0 && $0 < 1 ? 4 : 2) } ?? "—" }
    public static func compact(_ value: Int64) -> String {
        if value >= 1_000_000 { return number(Decimal(value) / 1_000_000, digits: 2) + "M" }
        if value >= 10_000 { return number(Decimal(value) / 1_000, digits: 1) + "k" }
        return number(Decimal(value), digits: 0)
    }
    public static func date(_ value: Date?) -> String {
        guard let value else { return "—" }
        let f = DateFormatter(); f.locale = Locale(identifier: "zh_CN"); f.dateFormat = "MM.dd HH:mm"
        return f.string(from: value)
    }
    public static func plainText(_ value: String?, limit: Int = 180) -> String {
        guard var value, !value.isEmpty else { return "点击前往官网查看完整内容。" }
        value = value.replacingOccurrences(of: #"(?is)<(script|style)\b[^>]*>.*?</\1>"#, with: " ", options: .regularExpression)
        value = value.replacingOccurrences(of: "<[^>]*>", with: " ", options: .regularExpression)
        // Native text only; never render announcement HTML or execute its content.
        for (entity, replacement) in [("&nbsp;", " "), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"), ("&amp;", "&")] {
            value = value.replacingOccurrences(of: entity, with: replacement)
        }
        value = value.replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression).trimmingCharacters(in: .whitespacesAndNewlines)
        return String(value.prefix(limit)) + (value.count > limit ? "…" : "")
    }
    public static func dayRange(now: Date, calendar: Calendar = .current) -> (from: Date, to: Date) {
        (calendar.startOfDay(for: now), now)
    }
}
