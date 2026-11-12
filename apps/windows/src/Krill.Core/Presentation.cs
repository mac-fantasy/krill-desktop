using System.Globalization;
using System.Net;
using System.Text.RegularExpressions;

namespace Krill.Core;

public sealed record QuotaProjection(string Cycle, decimal? Limit, decimal? Used, decimal? Remaining,
    decimal? Forwarded, DateTimeOffset? ResetAt, bool IsMoney, string Unit)
{
    public double? Percent => Limit > 0 && Remaining.HasValue
        ? (double)Math.Clamp(Remaining.Value / Limit.Value * 100m, 0m, 100m) : null;

    public static QuotaProjection From(Subscription subscription, RequestCountQuota? requestCountQuota = null)
    {
        var q = subscription.Quota;
        var type = subscription.Plan.BillingType;
        if (type == "request_count")
        {
            var window = AccountDetails.Windows(requestCountQuota).Where(x => x.Percent is not null).OrderBy(x => x.Percent).FirstOrDefault();
            if (window is not null) return new("按次额度 · " + window.Label, window.Limit, window.Used, window.Remaining, null, null, false, "次");
            return new("按次额度", q.LimitRequests, q.UsedRequests, q.RemainingRequests, null, q.WindowResetAt, false, "次");
        }
        if (subscription.Plan.TotalCredits.HasValue || q.LimitCredits.HasValue)
            return new("积分额度", q.LimitCredits ?? subscription.Plan.TotalCredits, q.UsedCredits, q.RemainingCredits,
                null, q.WindowResetAt, false, "积分");
        return new(type switch { "usd_daily" => "本日额度", "usd_weekly" => "本周额度", "usd_monthly" => "本月额度", _ => "当前周期额度" },
            q.DailyLimitUsd, q.UsedUsd, q.RemainingUsd, q.ForwardedRemainingUsd, q.WindowResetAt, true, "USD");
    }
}

public static partial class DisplayFormat
{
    public static string Number(decimal value, int digits = 2) => value.ToString("N" + digits, CultureInfo.InvariantCulture);
    public static string Money(decimal? value) => value.HasValue ? "$" + value.Value.ToString("N2", CultureInfo.InvariantCulture) : "—";
    public static string Cost(decimal? value) => value.HasValue ? "$" + value.Value.ToString(value.Value is > 0 and < 1 ? "0.0000" : "N2", CultureInfo.InvariantCulture) : "—";
    public static string Count(long value) => value.ToString("N0", CultureInfo.InvariantCulture);
    public static string Compact(long value) => value >= 1_000_000 ? (value / 1_000_000d).ToString("0.##", CultureInfo.InvariantCulture) + "M"
        : value >= 10_000 ? (value / 1000d).ToString("0.#", CultureInfo.InvariantCulture) + "k" : Count(value);
    public static string Amount(QuotaProjection q, decimal? value) => q.IsMoney ? Money(value)
        : value?.ToString("N0", CultureInfo.InvariantCulture) + (value.HasValue ? " " + q.Unit : "—");
    public static string LocalDate(DateTimeOffset? value) => value?.ToLocalTime().ToString("MM.dd HH:mm") ?? "—";

    public static string PlainText(string? text, int maxLength = 160)
    {
        if (string.IsNullOrWhiteSpace(text)) return "点击前往官网查看完整内容。";
        text = ScriptTags().Replace(text, " ");
        text = HtmlTags().Replace(text, " ");
        text = WhiteSpace().Replace(WebUtility.HtmlDecode(text), " ").Trim();
        return text.Length <= maxLength ? text : text[..maxLength] + "…";
    }

    public static (DateTimeOffset From, DateTimeOffset To) LocalDayRange(DateTimeOffset now, TimeZoneInfo zone)
    {
        var day = TimeZoneInfo.ConvertTime(now, zone).Date;
        return (new DateTimeOffset(day, zone.GetUtcOffset(day)).ToUniversalTime(), now.ToUniversalTime());
    }

    [GeneratedRegex(@"<(script|style)\b[^>]*>.*?</\1>", RegexOptions.IgnoreCase | RegexOptions.Singleline)]
    private static partial Regex ScriptTags();
    [GeneratedRegex(@"<[^>]*>")]
    private static partial Regex HtmlTags();
    [GeneratedRegex(@"\s+")]
    private static partial Regex WhiteSpace();
}
