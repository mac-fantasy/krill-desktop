using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media;
using System.Windows.Shapes;
using Krill.Core;

namespace Krill.Desktop.Controls;

public sealed class UsageTrendChart : StackPanel
{
    public static readonly DependencyProperty DataProperty = DependencyProperty.Register(nameof(Data), typeof(TrendChartData), typeof(UsageTrendChart),
        new PropertyMetadata(null, (owner, _) => ((UsageTrendChart)owner).Rebuild()));
    public TrendChartData? Data { get => (TrendChartData?)GetValue(DataProperty); set => SetValue(DataProperty, value); }

    private const string Hint = "悬停图表查看时段、费用与请求数";
    private static readonly Brush Mint = new SolidColorBrush(Color.FromRgb(124, 227, 205));
    private static readonly Brush Muted = new SolidColorBrush(Color.FromRgb(143, 164, 173));
    private static readonly Brush BorderBrush = new SolidColorBrush(Color.FromRgb(52, 71, 80));
    private readonly TextBlock readout = new() { FontSize = 11, Height = 32, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 8) };
    private readonly Grid plot = new() { Height = 104, Background = Brushes.Transparent };
    private readonly UniformGrid slots = new() { Rows = 1 };
    private readonly Canvas axis = new() { Height = 26, IsHitTestVisible = false };
    private readonly Border empty = new() { Height = 104, Child = new TextBlock { Text = "暂无时段统计", Foreground = Muted,
        HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center } };
    private readonly List<(Border Bar, Line Guide)> bars = [];
    private DateTimeOffset? hoveredStart;

    public UsageTrendChart()
    {
        AutomationProperties.SetAutomationId(readout, "usageTrendReadout");
        AutomationProperties.SetAutomationId(plot, "usageTrendPlot");
        Children.Add(readout); Children.Add(plot); Children.Add(axis); Children.Add(empty);
        plot.Children.Add(slots);
        plot.Children.Add(new Border { Height = 1, Background = BorderBrush, VerticalAlignment = VerticalAlignment.Bottom, IsHitTestVisible = false });
        plot.MouseLeave += (_, _) => Select(null);
        IsVisibleChanged += (_, _) => { if (!IsVisible) Select(null); };
        SizeChanged += (_, _) => LayoutAxis();
        Rebuild();
    }

    private void Rebuild()
    {
        slots.Children.Clear(); bars.Clear();
        var data = Data;
        var hasData = data?.Buckets.Count > 0;
        readout.Visibility = plot.Visibility = axis.Visibility = hasData ? Visibility.Visible : Visibility.Collapsed;
        empty.Visibility = hasData ? Visibility.Collapsed : Visibility.Visible;
        if (data is not null)
        {
            for (var i = 0; i < data.Buckets.Count; i++)
            {
                var index = i;
                // Each full-height slot includes the whitespace above and between bars.
                var slot = new Grid { Background = Brushes.Transparent };
                AutomationProperties.SetAutomationId(slot, $"usageTrendBucket{i}");
                AutomationProperties.SetName(slot, data.Tooltip(i));
                var bar = new Border { Background = data.Buckets[i].TotalCostUsd is null ? Muted : Mint,
                    Height = data.BarHeight(i), VerticalAlignment = VerticalAlignment.Bottom,
                    CornerRadius = new CornerRadius(2, 2, 0, 0), Margin = new Thickness(2, 0, 2, 0), IsHitTestVisible = false };
                var guide = new Line { X1 = 0, X2 = 0, Y1 = 0, Y2 = 104, Stroke = Mint, Opacity = 0.6,
                    StrokeDashArray = new DoubleCollection { 3, 3 }, HorizontalAlignment = HorizontalAlignment.Center,
                    IsHitTestVisible = false };
                slot.Children.Add(bar); slot.Children.Add(guide);
                slot.MouseEnter += (_, _) => Select(index);
                slots.Children.Add(slot); bars.Add((bar, guide));
            }
        }
        var previous = data is null || hoveredStart is null ? -1 : data.Buckets.ToList().FindIndex(x => x.BucketStart == hoveredStart);
        Select(previous >= 0 ? previous : null);
        LayoutAxis();
    }

    private void Select(int? index)
    {
        hoveredStart = index.HasValue ? Data!.Buckets[index.Value].BucketStart : null;
        readout.Text = index.HasValue ? Data!.Tooltip(index.Value) : Hint;
        readout.Foreground = index.HasValue ? Mint : Muted;
        for (var i = 0; i < bars.Count; i++)
        {
            bars[i].Bar.Opacity = i == index ? 1 : 0.75;
            bars[i].Guide.Visibility = i == index ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    private void LayoutAxis()
    {
        axis.Children.Clear();
        if (Data is not { Buckets.Count: > 0 } data || ActualWidth <= 0) return;
        foreach (var index in data.AxisIndices)
        {
            var x = (index + 0.5) * ActualWidth / data.Buckets.Count;
            var tick = new Border { Width = 1, Height = 4, Background = BorderBrush };
            Canvas.SetLeft(tick, x); axis.Children.Add(tick);
            var label = new TextBlock { Text = data.TimeLabel(index), FontSize = 9, Foreground = Muted, Width = 36, TextAlignment = TextAlignment.Center };
            Canvas.SetLeft(label, Math.Clamp(x - 18, 0, Math.Max(0, ActualWidth - 36)));
            Canvas.SetTop(label, 8); axis.Children.Add(label);
        }
    }
}
