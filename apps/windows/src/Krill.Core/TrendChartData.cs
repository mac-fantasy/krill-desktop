using System.Globalization;

namespace Krill.Core;

public sealed class TrendChartData
{
    public IReadOnlyList<TrendBucket> Buckets { get; }
    private readonly decimal maximum;

    public TrendChartData(IEnumerable<TrendBucket> buckets)
    {
        Buckets = buckets.OrderBy(x => x.BucketStart).TakeLast(48).ToArray();
        maximum = Buckets.Select(x => x.TotalCostUsd ?? 0).DefaultIfEmpty(0).Max();
    }

    public IEnumerable<int> AxisIndices => Enumerable.Range(0, Math.Min(5, Buckets.Count))
        .Select(i => Buckets.Count == 1 ? 0 : (int)Math.Round(i * (Buckets.Count - 1d) / (Math.Min(5, Buckets.Count) - 1), MidpointRounding.AwayFromZero));

    public double BarHeight(int index) => maximum > 0 && Buckets[index].TotalCostUsd is > 0
        ? Math.Clamp((double)(Buckets[index].TotalCostUsd!.Value / maximum) * 88, 3, 88) : 3;

    public string TimeLabel(int index, TimeZoneInfo? timeZone = null) =>
        TimeZoneInfo.ConvertTime(Buckets[index].BucketStart, timeZone ?? TimeZoneInfo.Local).ToString("HH:mm", CultureInfo.InvariantCulture);

    public string Tooltip(int index) =>
        $"{DisplayFormat.LocalDate(Buckets[index].BucketStart)} · 费用 {DisplayFormat.Cost(Buckets[index].TotalCostUsd)} · {Buckets[index].RequestCount} 次请求";
}
