using System.Text.Json;
using System.Windows.Controls;
using System.Windows.Threading;
using Krill.Core;
using Krill.Desktop.Services;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;

namespace Krill.Desktop.Views;

public partial class LoginWindow : Window
{
    private readonly Func<string, CancellationToken, Task> _acceptSession;
    private readonly LocalStorage _storage;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly DispatcherTimer _poll = new() { Interval = TimeSpan.FromSeconds(1.5) };
    private readonly Dictionary<WebView2, Window> _popups = [];
    private bool _checking;
    private bool _closed;
    private bool _initializing;
    private bool _embedded;
    private string? _rejectedToken;

    public LoginWindow(LocalStorage storage, Func<string, CancellationToken, Task> acceptSession)
    {
        InitializeComponent(); _storage = storage; _acceptSession = acceptSession;
        _poll.Tick += async (_, _) => await CheckSessionsAsync();
        Closed += (_, _) =>
        {
            _closed = true; _poll.Stop(); _lifetime.Cancel(); SessionInput.Clear();
            foreach (var popup in _popups.Values.ToArray()) popup.Close();
            Browser.Dispose(); _lifetime.Dispose();
        };
    }

    private async Task InitializeAsync()
    {
        if (_initializing || Browser.CoreWebView2 is not null || _closed) return;
        _initializing = true;
        try
        {
            // Embedded login stays ephemeral; the default-browser route uses the user's real browser session.
            Browser.CreationProperties = new CoreWebView2CreationProperties
            { UserDataFolder = Path.Combine(_storage.DirectoryPath, "Browser"), IsInPrivateModeEnabled = true };
            await Browser.EnsureCoreWebView2Async();
            if (_closed) return;
            ConfigureBrowser(Browser);
            Browser.CoreWebView2!.Navigate(BrowserLogin.LoginUrl.AbsoluteUri);
            StatusText.Text = "请选择官网任意登录方式；若服务商限制应用内登录，可改用浏览器登录。";
            RuntimeButton.Visibility = Visibility.Collapsed;
            _poll.Start();
        }
        catch (WebView2RuntimeNotFoundException)
        { if (!_closed) { StatusText.Text = "应用内登录需要 WebView2 Runtime，也可直接改用浏览器登录。"; RuntimeButton.Visibility = Visibility.Visible; } }
        catch (Exception e)
        { _storage.Log("login_initialization_failed", e); if (!_closed) StatusText.Text = "登录窗口初始化失败，可改用浏览器登录或关闭后重试。"; }
        finally { _initializing = false; }
    }

    private void ConfigureBrowser(WebView2 browser)
    {
        var core = browser.CoreWebView2;
        core.Settings.AreHostObjectsAllowed = false;
        core.Settings.IsWebMessageEnabled = false;
        core.Settings.IsPasswordAutosaveEnabled = false;
        core.Settings.AreDevToolsEnabled = false;
        core.PermissionRequested += (_, e) => e.State = CoreWebView2PermissionState.Deny;
        core.DownloadStarting += (_, e) => e.Cancel = true;
        core.NavigationStarting += (_, e) =>
        {
            if (!IsAllowedNavigation(e.Uri) && !(e.Uri == "about:blank" && _popups.ContainsKey(browser)))
            { e.Cancel = true; StatusText.Text = "此跳转不是有效的 HTTPS 页面；可改用浏览器完成登录。"; }
        };
        core.NewWindowRequested += async (_, e) => await OpenPopupAsync(browser, e);
        core.WindowCloseRequested += (_, _) => { if (_popups.TryGetValue(browser, out var popup)) popup.Close(); };
        core.SourceChanged += (_, _) =>
        {
            var origin = Uri.TryCreate(core.Source, UriKind.Absolute, out var uri) ? uri.GetLeftPart(UriPartial.Authority) : "";
            if (_popups.TryGetValue(browser, out var popup)) popup.Title = "Krill 登录 · " + origin;
            else LocationText.Text = origin;
        };
        core.NavigationCompleted += async (_, e) =>
        {
            if (_closed || !_embedded) return;
            if (!e.IsSuccess && e.WebErrorStatus != CoreWebView2WebErrorStatus.OperationCanceled)
                StatusText.Text = "页面加载失败，可关闭后重试，或改用浏览器登录。";
            else if (e.IsSuccess) await CheckSessionAsync(browser);
        };
    }

    private async Task OpenPopupAsync(WebView2 opener, CoreWebView2NewWindowRequestedEventArgs e)
    {
        e.Handled = true;
        if (_closed || _popups.Count >= 5 || !(e.Uri == "about:blank" || IsAllowedNavigation(e.Uri))) return;
        using var deferral = e.GetDeferral();
        var child = new WebView2();
        var popup = new Window
        {
            Title = "Krill · 第三方登录", Width = 760, Height = 720, MinWidth = 520, MinHeight = 500,
            Owner = this, Content = child, WindowStartupLocation = WindowStartupLocation.CenterOwner
        };
        _popups.Add(child, popup);
        popup.Closed += (_, _) => { _popups.Remove(child); child.Dispose(); };
        try
        {
            // Same environment/profile and a real NewWindow retain cookies, opener and window.close().
            popup.Show();
            var core = opener.CoreWebView2;
            var options = core.Environment.CreateCoreWebView2ControllerOptions();
            options.ProfileName = core.Profile.ProfileName;
            options.IsInPrivateModeEnabled = core.Profile.IsInPrivateModeEnabled;
            await child.EnsureCoreWebView2Async(core.Environment, options);
            if (_closed || !_popups.ContainsKey(child)) return;
            ConfigureBrowser(child);
            e.NewWindow = child.CoreWebView2;
        }
        catch (Exception error)
        {
            _storage.Log("login_popup_failed", error);
            if (!_closed) StatusText.Text = "无法打开授权窗口，请改用浏览器登录。";
            popup.Close();
        }
    }

    internal static bool IsAllowedNavigation(string value) => BrowserLogin.AllowsNavigation(value);

    private async Task CheckSessionsAsync()
    {
        foreach (var browser in new[] { Browser }.Concat(_popups.Keys).ToArray())
            await CheckSessionAsync(browser);
    }

    private bool ContainsBrowser(WebView2 browser) => browser == Browser || _popups.ContainsKey(browser);

    private async Task CheckSessionAsync(WebView2 browser)
    {
        if (!_embedded || _checking || _closed || !ContainsBrowser(browser) || browser.CoreWebView2 is null || !KrillApiClient.IsTrustedPage(browser.Source)) return;
        _checking = true;
        try
        {
            var result = await browser.ExecuteScriptAsync("(() => location.origin === 'https://www.krill-code.com' ? {origin: location.origin, token: localStorage.getItem('krill_jwt')} : null)()");
            if (_closed || !ContainsBrowser(browser) || !KrillApiClient.IsTrustedPage(browser.Source)) return;
            using var document = JsonDocument.Parse(result);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("origin", out var origin) || origin.GetString() != "https://www.krill-code.com" ||
                !root.TryGetProperty("token", out var value) || value.ValueKind != JsonValueKind.String) return;
            var token = value.GetString();
            if (!KrillApiClient.IsPlausibleSession(token) || token == _rejectedToken) return;
            await ValidateSessionAsync(token!);
        }
        catch (OperationCanceledException) { }
        catch (Exception e)
        { _storage.Log("login_session_check_failed", e); if (!_closed) StatusText.Text = "暂时无法确认登录，请稍后重试。"; }
        finally { _checking = false; }
    }

    private async Task ValidateSessionAsync(string token)
    {
        StatusText.Text = "正在确认账号…";
        try
        {
            await _acceptSession(token, _lifetime.Token);
            if (!_closed) Close();
        }
        catch (KrillApiException e)
        {
            if (e.Failure == ApiFailure.SessionExpired) _rejectedToken = token;
            if (!_closed) StatusText.Text = e.FriendlyMessage;
        }
    }

    private async void ImportSessionClick(object sender, RoutedEventArgs e)
    {
        if (_checking || _closed) return;
        var token = BrowserLogin.SessionFromText(SessionInput.Password);
        if (token is null) { StatusText.Text = "会话格式不正确，请粘贴脚本弹出的完整 Krill 会话。"; return; }
        SessionInput.Clear(); _checking = true;
        try { await ValidateSessionAsync(token); }
        catch (OperationCanceledException) { }
        catch (Exception error)
        { _storage.Log("login_session_import_failed", error); if (!_closed) StatusText.Text = "暂时无法确认登录或保存凭据，请稍后重试。"; }
        finally { _checking = false; }
    }
    private void OpenBrowserClick(object sender, RoutedEventArgs e)
    {
        try { BrowserLauncher.OpenLogin(); }
        catch (Exception error) { _storage.Log("login_browser_launch_failed", error); StatusText.Text = "无法打开默认浏览器，请手动访问 https://www.krill-code.com/login。"; }
    }
    private void CopyExportScriptClick(object sender, RoutedEventArgs e)
    {
        try { Clipboard.SetText(BrowserLogin.ExportScript); StatusText.Text = "脚本已复制，请仅在 Krill 官网的控制台运行，然后导入弹出的会话。"; }
        catch (Exception error) { _storage.Log("login_script_copy_failed", error); StatusText.Text = "剪贴板暂时不可用，请稍后重试。"; }
    }
    private async void ToggleModeClick(object sender, RoutedEventArgs e)
    {
        if (_checking || _closed) return;
        _embedded = !_embedded;
        BrowserGuide.Visibility = _embedded ? Visibility.Collapsed : Visibility.Visible;
        EmbeddedContainer.Visibility = _embedded ? Visibility.Visible : Visibility.Collapsed;
        ModeButton.Content = _embedded ? "改用浏览器登录" : "改用应用内登录";
        if (_embedded) await InitializeAsync();
        else
        {
            foreach (var popup in _popups.Values.ToArray()) popup.Close();
            RuntimeButton.Visibility = Visibility.Collapsed;
            StatusText.Text = "在默认浏览器登录后导入会话，即可复用已有登录状态。";
        }
    }
    private void RuntimeClick(object sender, RoutedEventArgs e)
    { try { BrowserLauncher.OpenRuntimeDownload(); } catch (Exception error) { _storage.Log("runtime_link_failed", error); } }
}
