using System.ComponentModel;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using Krill.Desktop.ViewModels;

namespace Krill.Desktop.Views;

public partial class PanelWindow : Window
{
    public bool AllowClose { get; set; }
    public PanelWindow() => InitializeComponent();
    public void ClearInitialFocus() => Dispatcher.BeginInvoke(DispatcherPriority.Input, () =>
    {
        if (!IsVisible) return;
        FocusManager.SetFocusedElement(this, this);
        Keyboard.Focus(this);
    });
    private void HideClick(object sender, RoutedEventArgs e) => Hide();
    private void PinClick(object sender, RoutedEventArgs e)
    { if (DataContext is DashboardViewModel vm) vm.PanelPinned = !vm.PanelPinned; }
    private void PanelKeyDown(object sender, KeyEventArgs e)
    { if (e.Key == Key.Escape) { Hide(); e.Handled = true; } }
    private void HeaderMouseDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left) return;
        var node = e.OriginalSource as DependencyObject;
        while (node is not null)
        { if (node is ButtonBase) return; node = VisualTreeHelper.GetParent(node); }
        try { DragMove(); } catch (InvalidOperationException) { }
    }
    private void PanelDeactivated(object? sender, EventArgs e) => Dispatcher.BeginInvoke(DispatcherPriority.Background, () =>
    {
        if (!IsActive && DataContext is DashboardViewModel { PanelPinned: false, IsLoginOpen: false }) Hide();
    });
    protected override void OnClosing(CancelEventArgs e)
    { if (!AllowClose) { e.Cancel = true; Hide(); } base.OnClosing(e); }
}
