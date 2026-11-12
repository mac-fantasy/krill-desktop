namespace Krill.Core;

public enum SubscriptionAvailability { Available, NotStarted, Expired, Exhausted, Frozen, Inactive }

public static class SubscriptionSelection
{
    public static bool IsMonthly(Subscription subscription) => subscription.Plan.BillingType == "usd_monthly" ||
        subscription.Plan.DurationDays is >= 28 and <= 31;

    public static SubscriptionAvailability Availability(Subscription subscription, DateTimeOffset now)
    {
        if (subscription.Status == "expired" || subscription.SubscriptionEndAt <= now) return SubscriptionAvailability.Expired;
        if (subscription.Status == "frozen") return SubscriptionAvailability.Frozen;
        if (subscription.Status == "exhausted") return SubscriptionAvailability.Exhausted;
        if (subscription.SubscriptionStartAt > now) return SubscriptionAvailability.NotStarted;
        if (subscription.Status != "active") return SubscriptionAvailability.Inactive;
        var quota = QuotaProjection.From(subscription);
        if (IsMonthly(subscription) && quota.Remaining <= 0 && !(quota.IsMoney && quota.Forwarded > 0))
            return SubscriptionAvailability.Exhausted;
        return SubscriptionAvailability.Available;
    }

    public static string Label(SubscriptionAvailability availability) => availability switch
    {
        SubscriptionAvailability.Available => "使用中",
        SubscriptionAvailability.NotStarted => "未生效",
        SubscriptionAvailability.Expired => "已到期",
        SubscriptionAvailability.Exhausted => "额度已用尽",
        SubscriptionAvailability.Frozen => "已冻结",
        _ => "已结束 / 不可用"
    };

    public static IOrderedEnumerable<Subscription> Ordered(IEnumerable<Subscription> subscriptions) =>
        subscriptions.OrderBy(x => x.ConsumePriority).ThenBy(x => x.SubscriptionId);

    public static Subscription? Select(IEnumerable<Subscription> subscriptions, long? preferredId, DateTimeOffset now, bool preferredWasMonthly = false)
    {
        var all = Ordered(subscriptions).ToArray();
        var available = all.Where(x => Availability(x, now) == SubscriptionAvailability.Available).ToArray();
        if (available.FirstOrDefault(x => x.SubscriptionId == preferredId) is { } preferred) return preferred;
        if ((preferredWasMonthly || all.Any(x => x.SubscriptionId == preferredId && IsMonthly(x))) &&
            available.FirstOrDefault(IsMonthly) is { } monthly) return monthly;
        return available.FirstOrDefault();
    }

    public static DateTimeOffset? NextTransition(IEnumerable<Subscription> subscriptions, DateTimeOffset now) =>
        subscriptions.SelectMany(x => new[] { x.SubscriptionStartAt, x.SubscriptionEndAt }).Where(x => x > now).Min();
}
