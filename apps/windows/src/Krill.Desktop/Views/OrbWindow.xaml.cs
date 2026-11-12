using System.ComponentModel;
using System.Windows.Input;
using System.Windows.Threading;
using Krill.Desktop.ViewModels;
using Krill.Desktop.Services;

namespace Krill.Desktop.Views;

public partial class OrbWindow : Window
{
    private Point _downPoint;
    private bool _tracking;
    private bool _dragged;
    private bool _dragging;
    private bool _snapEnabled;
    private readonly DispatcherTimer _collapseTimer = new() { Interval = TimeSpan.FromMilliseconds(650) };
    internal OrbDockSide DockSide { get; private set; }
    internal bool IsCollapsed { get; private set; }
    internal const double BallSize = 88;
    internal const double BarWidth = 16;
    public bool AllowClose { get; set; }
    public event Action? PanelRequested;
    public event Action? DragStarted;
    public event Action? DragFinished;

    public OrbWindow()
    {
        InitializeComponent();
        SourceInitialized += (_, _) => WindowPlacement.MakeToolWindow(this, noActivate: true);
        _collapseTimer.Tick += (_, _) => { _collapseTimer.Stop(); if (!IsMouseOver && !_tracking && !_dragging && OrbSurface.ContextMenu?.IsOpen != true) CollapseDocked(); };
        DataContextChanged += (_, e) =>
        {
            if (e.OldValue is INotifyPropertyChanged oldVm) oldVm.PropertyChanged -= ModelChanged;
            if (e.NewValue is INotifyPropertyChanged newVm) newVm.PropertyChanged += ModelChanged;
            UpdateBar();
        };
        SizeChanged += (_, _) => UpdateBar();
        OrbSurface.ContextMenu.Closed += (_, _) => ScheduleCollapse();
        IsVisibleChanged += (_, _) => { if (IsVisible) ScheduleCollapse(); else _collapseTimer.Stop(); };
        Closed += (_, _) => _collapseTimer.Stop();
    }
    private void ModelChanged(object? sender, PropertyChangedEventArgs e)
    { if (e.PropertyName == nameof(DashboardViewModel.QuotaPercent)) UpdateBar(); }
    private void UpdateBar()
    {
        var value = DataContext is DashboardViewModel vm ? vm.QuotaPercent : 0;
        BarFill.Height = Math.Max(0, Height - 18) * Math.Clamp(value, 0, 100) / 100;
    }
    internal void ConfigureDocking(bool enabled, bool snapNow)
    {
        _snapEnabled = enabled;
        if (!enabled) { ExpandDocked(); DockSide = OrbDockSide.None; _collapseTimer.Stop(); return; }
        if (snapNow || DockSide == OrbDockSide.None) DockSide = WindowPlacement.FindDockSide(this, snapNow);
        ScheduleCollapse();
    }
    internal void CollapseDocked()
    {
        if (!_snapEnabled || DockSide == OrbDockSide.None || _dragging) return;
        ResizeDocked(collapsed: true);
    }
    internal void ExpandDocked()
    {
        _collapseTimer.Stop();
        if (IsCollapsed) ResizeDocked(collapsed: false);
    }
    private void ResizeDocked(bool collapsed)
    {
        IsCollapsed = collapsed;
        BallVisual.Visibility = collapsed ? Visibility.Collapsed : Visibility.Visible;
        BarVisual.Visibility = collapsed ? Visibility.Visible : Visibility.Collapsed;
        WindowPlacement.ResizeDockedOrb(this, collapsed ? BarWidth : BallSize, DockSide);
        UpdateBar();
    }
    private void ScheduleCollapse()
    { _collapseTimer.Stop(); if (_snapEnabled && DockSide != OrbDockSide.None && !IsCollapsed) _collapseTimer.Start(); }
    private void OrbMouseEnter(object sender, MouseEventArgs e) { if (!_tracking && !_dragging) ExpandDocked(); }
    private void OrbMouseLeave(object sender, MouseEventArgs e) => ScheduleCollapse();
    private void OrbMouseDown(object sender, MouseButtonEventArgs e)
    {
        _downPoint = PointToScreen(e.GetPosition(this)); _tracking = true; _dragged = false;
        OrbSurface.CaptureMouse(); e.Handled = true;
    }
    private void OrbMouseMove(object sender, MouseEventArgs e)
    {
        if (!_tracking || e.LeftButton != MouseButtonState.Pressed) return;
        var position = PointToScreen(e.GetPosition(this));
        if (Math.Abs(position.X - _downPoint.X) < SystemParameters.MinimumHorizontalDragDistance &&
            Math.Abs(position.Y - _downPoint.Y) < SystemParameters.MinimumVerticalDragDistance) return;
        _tracking = false; _dragged = true; _dragging = true; _collapseTimer.Stop();
        ExpandDocked(); OrbSurface.ReleaseMouseCapture(); DragStarted?.Invoke();
        try { DragMove(); } catch (InvalidOperationException) { }
        _dragging = false;
        DragFinished?.Invoke();
    }
    private void OrbMouseUp(object sender, MouseButtonEventArgs e)
    {
        OrbSurface.ReleaseMouseCapture();
        if (_tracking && !_dragged) PanelRequested?.Invoke();
        _tracking = false; e.Handled = true;
        ScheduleCollapse();
    }
    private void OpenPanelClick(object sender, RoutedEventArgs e) => PanelRequested?.Invoke();
    protected override void OnClosing(CancelEventArgs e)
    { if (!AllowClose) { e.Cancel = true; Hide(); } base.OnClosing(e); }
}
