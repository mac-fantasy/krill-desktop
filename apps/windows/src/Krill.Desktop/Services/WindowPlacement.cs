using System.Runtime.InteropServices;
using System.Windows.Interop;
using System.Windows.Media;
using Forms = System.Windows.Forms;

namespace Krill.Desktop.Services;

internal enum OrbDockSide { None, Left, Right }

internal static class WindowPlacement
{
    [StructLayout(LayoutKind.Sequential)] internal struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] private static extern bool GetWindowRect(nint hwnd, out Rect rect);
    [DllImport("user32.dll")] private static extern bool SetWindowPos(nint hwnd, nint after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] private static extern nint GetWindowLongPtr(nint hwnd, int index);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")] private static extern nint SetWindowLongPtr(nint hwnd, int index, nint value);

    public static void MakeToolWindow(Window window, bool noActivate)
    {
        var hwnd = new WindowInteropHelper(window).Handle;
        var style = GetWindowLongPtr(hwnd, -20).ToInt64() | 0x80;
        style &= ~0x40000L;
        if (noActivate) style |= 0x08000000L;
        SetWindowLongPtr(hwnd, -20, (nint)style);
    }

    private static Rect Bounds(Window window)
    {
        GetWindowRect(new WindowInteropHelper(window).Handle, out var rect);
        return rect;
    }

    internal static OrbDockSide FindDockSide(Window window, bool force)
    {
        var b = Bounds(window);
        var area = Forms.Screen.FromHandle(new WindowInteropHelper(window).Handle).WorkingArea;
        var left = Math.Abs(b.Left - area.Left); var right = Math.Abs(b.Right - area.Right);
        var threshold = 12 * VisualTreeHelper.GetDpi(window).DpiScaleX;
        if (!force && Math.Min(left, right) > threshold) return OrbDockSide.None;
        return left <= right ? OrbDockSide.Left : OrbDockSide.Right;
    }

    internal static void ResizeDockedOrb(Window window, double width, OrbDockSide side)
    {
        var b = Bounds(window);
        var area = Forms.Screen.FromHandle(new WindowInteropHelper(window).Handle).WorkingArea;
        var dpi = VisualTreeHelper.GetDpi(window);
        window.Width = width; window.Height = Views.OrbWindow.BallSize;
        var pixelsWide = (int)Math.Round(width * dpi.DpiScaleX);
        var pixelsHigh = (int)Math.Round(window.Height * dpi.DpiScaleY);
        var x = side == OrbDockSide.Left ? area.Left + 2 : area.Right - pixelsWide - 2;
        var y = Math.Clamp(b.Top, area.Top + 2, Math.Max(area.Top + 2, area.Bottom - pixelsHigh - 2));
        SetWindowPos(new WindowInteropHelper(window).Handle, 0, x, y, pixelsWide, pixelsHigh, 0x14);
    }

    internal static (int X, int Y) ExpandedOrbPosition(Views.OrbWindow window)
    {
        var b = Bounds(window);
        var width = (int)Math.Round(Views.OrbWindow.BallSize * VisualTreeHelper.GetDpi(window).DpiScaleX);
        return (window.IsCollapsed && window.DockSide == OrbDockSide.Right ? b.Right - width : b.Left, b.Top);
    }

    public static void InitializeOrb(Window orb, ClientSettings settings)
    {
        var area = Forms.Screen.PrimaryScreen?.WorkingArea ?? Forms.Screen.AllScreens[0].WorkingArea;
        var x = settings.OrbLeftPixels ?? area.Right - 108;
        var y = settings.OrbTopPixels ?? area.Top + area.Height / 2 - 44;
        SetWindowPos(new WindowInteropHelper(orb).Handle, 0, x, y, 0, 0, 0x15);
        ClampOrb(orb, snap: false);
    }

    public static (int X, int Y) ClampOrb(Window orb, bool snap)
    {
        var bounds = Bounds(orb);
        var area = Forms.Screen.FromRectangle(new System.Drawing.Rectangle(bounds.Left, bounds.Top,
            bounds.Right - bounds.Left, bounds.Bottom - bounds.Top)).WorkingArea;
        var width = bounds.Right - bounds.Left; var height = bounds.Bottom - bounds.Top;
        var x = Math.Clamp(bounds.Left, area.Left + 4, Math.Max(area.Left + 4, area.Right - width - 4));
        var y = Math.Clamp(bounds.Top, area.Top + 4, Math.Max(area.Top + 4, area.Bottom - height - 4));
        if (snap) x = x + width / 2 < area.Left + area.Width / 2 ? area.Left + 4 : area.Right - width - 4;
        SetWindowPos(new WindowInteropHelper(orb).Handle, 0, x, y, 0, 0, 0x15);
        return (x, y);
    }

    public static void PlacePanel(Window panel, Window orb)
    {
        var orbBounds = Bounds(orb);
        var screen = Forms.Screen.FromRectangle(new System.Drawing.Rectangle(orbBounds.Left, orbBounds.Top,
            orbBounds.Right - orbBounds.Left, orbBounds.Bottom - orbBounds.Top));
        var area = screen.WorkingArea;
        var scale = VisualTreeHelper.GetDpi(orb).DpiScaleX;
        var width = Math.Min((int)Math.Round(520 * scale), area.Width - 16);
        var height = Math.Min((int)Math.Round(800 * scale), area.Height - 16);
        var gap = (int)(8 * scale);
        var x = orbBounds.Left - width - gap;
        if (x < area.Left) x = orbBounds.Right + gap;
        x = Math.Clamp(x, area.Left + 8, Math.Max(area.Left + 8, area.Right - width - 8));
        var y = Math.Clamp(orbBounds.Top - 140, area.Top + 8, Math.Max(area.Top + 8, area.Bottom - height - 8));
        SetWindowPos(new WindowInteropHelper(panel).EnsureHandle(), 0, x, y, width, height, 0x14);
    }
}
