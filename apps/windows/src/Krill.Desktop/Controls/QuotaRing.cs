using System.Windows.Media;

namespace Krill.Desktop.Controls;

public sealed class QuotaRing : FrameworkElement
{
    public static readonly DependencyProperty ValueProperty = DependencyProperty.Register(nameof(Value), typeof(double), typeof(QuotaRing),
        new FrameworkPropertyMetadata(0d, FrameworkPropertyMetadataOptions.AffectsRender));
    public static readonly DependencyProperty StrokeProperty = DependencyProperty.Register(nameof(Stroke), typeof(Brush), typeof(QuotaRing),
        new FrameworkPropertyMetadata(Brushes.Aquamarine, FrameworkPropertyMetadataOptions.AffectsRender));
    public static readonly DependencyProperty ThicknessProperty = DependencyProperty.Register(nameof(Thickness), typeof(double), typeof(QuotaRing),
        new FrameworkPropertyMetadata(4d, FrameworkPropertyMetadataOptions.AffectsRender));
    public double Value { get => (double)GetValue(ValueProperty); set => SetValue(ValueProperty, value); }
    public Brush Stroke { get => (Brush)GetValue(StrokeProperty); set => SetValue(StrokeProperty, value); }
    public double Thickness { get => (double)GetValue(ThicknessProperty); set => SetValue(ThicknessProperty, value); }

    protected override void OnRender(DrawingContext dc)
    {
        base.OnRender(dc);
        var center = new Point(ActualWidth / 2, ActualHeight / 2);
        var radius = Math.Max(0, Math.Min(ActualWidth, ActualHeight) / 2 - Thickness / 2 - 1);
        var track = new Pen(new SolidColorBrush(Color.FromRgb(37, 55, 64)), Thickness);
        dc.DrawEllipse(null, track, center, radius, radius);
        var value = double.IsFinite(Value) ? Math.Clamp(Value, 0, 100) : 0;
        var pen = new Pen(Stroke, Thickness) { StartLineCap = PenLineCap.Round, EndLineCap = PenLineCap.Round };
        if (value >= 99.999) { dc.DrawEllipse(null, pen, center, radius, radius); return; }
        if (value <= 0) return;
        var angle = value / 100 * Math.PI * 2 - Math.PI / 2;
        var geometry = new StreamGeometry();
        using (var context = geometry.Open())
        {
            context.BeginFigure(new Point(center.X, center.Y - radius), false, false);
            context.ArcTo(new Point(center.X + Math.Cos(angle) * radius, center.Y + Math.Sin(angle) * radius),
                new Size(radius, radius), 0, value > 50, SweepDirection.Clockwise, true, false);
        }
        geometry.Freeze(); dc.DrawGeometry(null, pen, geometry);
    }
}
