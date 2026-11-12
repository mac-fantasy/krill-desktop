namespace Krill.Core;

public sealed record RequestQuotaWindow(string Label, long? Used, long? Limit)
{
    public decimal? Remaining => Limit > 0 && Used >= 0 ? Math.Max(0, (decimal)Limit.Value - Used.Value) : null;
    public double? Percent => Remaining is { } remaining && Limit > 0 ? (double)(remaining / Limit.Value * 100) : null;
    public string Detail => $"已用 {AccountDetails.Count(Used)} / {(Limit > 0 ? AccountDetails.Count(Limit) : "—")} 次";
}

public enum ResetCardState { Unused, Used, Revoked, Expired, Unknown }

public static class AccountDetails
{
    public static RequestQuotaWindow[] Windows(RequestCountQuota? quota, Plan? fallback = null) =>
    [
        new("5 小时", quota?.Used5h, quota is null ? fallback?.RateLimit5h : quota.Limit5h),
        new("本周", quota?.UsedWeekly, quota is null ? fallback?.RateLimitWeekly : quota.LimitWeekly),
        new("本月", quota?.UsedMonthly, quota is null ? fallback?.RateLimitMonthly : quota.LimitMonthly)
    ];
    public static string Cycle(RequestCountQuota? quota) =>
        $"周期 {DisplayFormat.PlainText(quota?.CycleStart ?? "—", 40)} ~ {DisplayFormat.PlainText(quota?.CycleEnd ?? "—", 40)}";

    public static decimal? CachePercent(RequestStats stats)
    {
        decimal read, eligible;
        if (stats.CacheRateReadTokens is { } dedicatedRead && stats.CacheRateEligibleTokens is { } dedicatedEligible)
        { read = dedicatedRead; eligible = dedicatedEligible; }
        else if (stats.CacheReadInputTokens is >= 0 && stats.CacheCreationInputTokens is >= 0 && stats.InputTokens >= 0)
        { read = stats.CacheReadInputTokens.Value; eligible = (decimal)stats.InputTokens.Value + stats.CacheCreationInputTokens.Value + read; }
        else return null;
        return eligible > 0 && read >= 0 && read <= eligible ? read / eligible * 100 : null;
    }
    public static string Percentage(decimal? value) => value is >= 0 and <= 100 ? DisplayFormat.Number(value.Value, 1) + "%" : "—";
    public static string Count(long? value) => value >= 0 ? DisplayFormat.Count(value.Value) : "—";
    public static string Rate(decimal? value) => value >= 0 ? DisplayFormat.Number(value.Value, 1) : "—";
    public static ResetCardState State(ResetCard card, DateTimeOffset now)
    {
        if (card.Status == 1) return ResetCardState.Used;
        if (card.Status == 2) return ResetCardState.Revoked;
        if (card.Expired == true || card.ExpiresAt <= now) return ResetCardState.Expired;
        return card.Status == 0 ? ResetCardState.Unused : ResetCardState.Unknown;
    }
    public static bool CanUse(ResetCard card, DateTimeOffset now) => State(card, now) == ResetCardState.Unused && card.UsableSubscriptions?.Count > 0;
    public static string StateLabel(ResetCardState state) => state switch
    {
        ResetCardState.Unused => "未使用", ResetCardState.Used => "已使用", ResetCardState.Revoked => "已作废",
        ResetCardState.Expired => "已过期", _ => "状态未知"
    };
    public static string AvailabilityText(ResetCard card, DateTimeOffset now) => State(card, now) != ResetCardState.Unused
        ? StateLabel(State(card, now)) : card.UsableSubscriptions is null ? "未使用 · 适用订阅未知"
        : card.UsableSubscriptions.Count == 0 ? "未使用 · 暂无适用订阅" : $"可用 · {card.UsableSubscriptions.Count} 个订阅";
}
