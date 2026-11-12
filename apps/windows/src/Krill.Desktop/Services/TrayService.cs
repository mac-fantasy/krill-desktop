using System.Drawing;
using Forms = System.Windows.Forms;

namespace Krill.Desktop.Services;

public sealed class TrayService : IDisposable
{
    private readonly Forms.NotifyIcon _icon;
    private readonly Icon _image;
    private readonly Forms.ContextMenuStrip _menu;
    public TrayService(Action showPanel, Action toggleOrb, Action refresh, Action website, Action checkUpdate, Action exit)
    {
        using var resource = Application.GetResourceStream(new Uri("pack://application:,,,/Assets/krill.ico")).Stream;
        using var loaded = new Icon(resource);
        _image = (Icon)loaded.Clone();
        _menu = new Forms.ContextMenuStrip();
        Add("打开详情面板", showPanel); Add("显示 / 隐藏悬浮球", toggleOrb); Add("立即刷新", refresh); Add("打开官网", website);
        Add("检查更新", checkUpdate);
        _menu.Items.Add(new Forms.ToolStripSeparator()); Add("退出 Krill Desktop", exit);
        _icon = new Forms.NotifyIcon { Icon = _image, Text = "Krill Desktop", ContextMenuStrip = _menu, Visible = true };
        _icon.DoubleClick += (_, _) => Application.Current.Dispatcher.InvokeAsync(showPanel);
    }
    private void Add(string label, Action action) => _menu.Items.Add(label, null, (_, _) => Application.Current.Dispatcher.InvokeAsync(action));
    public void Update(string value)
    { var text = "Krill Desktop · " + value; _icon.Text = text.Length > 63 ? text[..63] : text; }
    public void Dispose() { _icon.Visible = false; _icon.Dispose(); _menu.Dispose(); _image.Dispose(); }
}
