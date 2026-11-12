import Foundation

public enum DemoData {
    public static var profile: UserProfile { UserProfile(email: "explorer@example.com", name: "Krill Explorer") }
    public static func subscriptions(now: Date = Date()) -> SubscriptionResponse {
        var weekly = Quota(); weekly.dailyLimitUsd = 100; weekly.usedUsd = Decimal(string: "27.40")
        weekly.remainingUsd = Decimal(string: "72.60"); weekly.forwardedRemainingUsd = Decimal(string: "8.20")
        weekly.windowResetAt = now.addingTimeInterval(4 * 86400)
        var daily = Quota(); daily.dailyLimitUsd = 20; daily.usedUsd = Decimal(string: "3.20")
        daily.remainingUsd = Decimal(string: "16.80"); daily.windowResetAt = now.addingTimeInterval(6 * 3600)
        let shared = try! KrillJSON.decoder().decode(RequestCountQuota.self, from: Data(#"{"used_5h":23,"limit_5h":100,"used_weekly":180,"limit_weekly":1000,"used_monthly":780,"limit_monthly":4000,"cycle_start":"2026-10-01","cycle_end":"2026-11-01"}"#.utf8))
        var requestPlan = Plan(name: "狂蹬卡 · 按次套餐", billingType: "request_count")
        requestPlan.rateLimit5h = 100; requestPlan.rateLimitWeekly = 1000; requestPlan.rateLimitMonthly = 4000
        return SubscriptionResponse(subscriptions: [
            Subscription(id: 1, status: "active", plan: Plan(name: "Codex Pro · 周套餐", billingType: "usd_weekly"), quota: weekly, end: now.addingTimeInterval(23 * 86400), priority: 1),
            Subscription(id: 2, status: "active", plan: Plan(name: "Claude · 日套餐", billingType: "usd_daily"), quota: daily, end: now.addingTimeInterval(12 * 86400), priority: 2),
            Subscription(id: 3, status: "active", plan: requestPlan, quota: Quota(), end: now.addingTimeInterval(20 * 86400), priority: 3)
        ], creditBalanceUsd: Decimal(string: "24.80"), welfareBalanceUsd: Decimal(string: "3.50"), requestCountQuota: shared)
    }
    public static func stats(now: Date = Date()) -> RequestStats {
        let start = Calendar.current.startOfDay(for: now)
        let trend: [TrendBucket] = (0..<24).map { (hour: Int) -> TrendBucket in
            let requests = Int64((hour * 7 + 3) % 19)
            let cost = Decimal((hour * 13 + 5) % 29) / Decimal(10)
            return TrendBucket(start: start.addingTimeInterval(Double(hour) * 3600), requests: requests, cost: cost)
        }
        var result = RequestStats(requests: 128, successes: 126, tokens: 1_284_600, input: 1_046_200, output: 238_400,
                            cost: Decimal(string: "7.42")!, trend: trend)
        result.failedRequests = 2; result.cacheReadInputTokens = 640000; result.cacheCreationInputTokens = 120000
        result.cacheRateEligibleTokens = 1000000; result.cacheRateReadTokens = 640000; result.reasoningTokens = 48500
        result.rpm = Decimal(string: "1.8"); result.tpm = 17842
        return result
    }
    public static func resetCards(now: Date = Date()) -> ResetCardsResponse {
        let text = #"{"items":[{"id":1,"plan_name":"Codex Pro · 周套餐","status":0,"expired":false,"cap_usd":"30","usable_subscriptions":[{"subscription_id":1}],"note":"虚构演示重置卡"},{"id":2,"plan_name":"历史重置卡","status":1,"cap_usd":"20","granted_usd":"12.50","usable_subscriptions":[]}]}"#
        var response = try! KrillJSON.decoder().decode(ResetCardsResponse.self, from: Data(text.utf8))
        response.items[0].expiresAt = now.addingTimeInterval(2 * 86400)
        response.items[1].usedAt = now.addingTimeInterval(-86400)
        return response
    }
    public static func announcements(now: Date = Date()) -> [Announcement] {
        [Announcement(id: 3, title: "欢迎使用 Krill Desktop", summary: "额度、余额与套餐，一眼就能看见。此处为虚构演示内容。", date: now.addingTimeInterval(-7200)),
         Announcement(id: 2, title: "让桌面留一点空间给重要的事", summary: "菜单栏进度环随所选套餐更新。点击图标即可查看详情。", date: now.addingTimeInterval(-86400)),
         Announcement(id: 1, title: "你的数据，仅保存在本机", summary: "会话存入 macOS 钥匙串；演示模式不会请求个人账号接口。", date: now.addingTimeInterval(-3 * 86400))]
    }
}
