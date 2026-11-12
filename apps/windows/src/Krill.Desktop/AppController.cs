using System.ComponentModel;
using Krill.Core;
using Krill.Desktop.Services;
using Krill.Desktop.ViewModels;
using Krill.Desktop.Views;

namespace Krill.Desktop;

public sealed class AppController : IDisposable
{
    private readonly LocalStorage _storage;
    private readonly ClientSettings _settings;
    private readonly HttpClient _http;
    private readonly KrillApiClient _api;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource _session = new();
    private readonly SemaphoreSlim _refreshGate = new(1, 1);
    private TrayService? _tray;
    private LoginWindow? _login;
    private string? _token;
    private UserProfile? _profile;
    private int _generation;
    private int _failures;
    private bool _ready;
    private bool _savingPreferences;
    private bool _disposed;
    private DateTimeOffset _lastAnnouncements;
    private DateTimeOffset _lastResetCards;

    public DashboardViewModel ViewModel { get; }
    public OrbWindow Orb { get; }
    public PanelWindow Panel { get; }

    public AppController(LocalStorage storage)
    {
        _storage = storage; _settings = storage.LoadSettings();
        _http = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false, UseCookies = false })
        { Timeout = TimeSpan.FromSeconds(20) };
        _api = new KrillApiClient(_http);
        ViewModel = new DashboardViewModel(_settings, new DashboardActions(ShowLoginAsync, () => RefreshAsync(),
            OpenWebsite, SignOut, Quit, SavePreferences, MarkRead, UseDemo));
        Orb = new OrbWindow { DataContext = ViewModel };
        Panel = new PanelWindow { DataContext = ViewModel };
        Orb.PanelRequested += TogglePanel;
        Orb.DragStarted += () => Panel.Hide();
        Orb.DragFinished += SaveOrbPosition;
        ViewModel.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(ViewModel.OrbText)) _tray?.Update(ViewModel.OrbText);
            if (e.PropertyName == nameof(ViewModel.SelectedTab) && ViewModel.HasData && !ViewModel.IsDemo &&
                ViewModel.SelectedTab is 2 or 3) _ = RefreshAsync();
        };
        _ready = true;
    }

    public async Task StartAsync(bool demo, bool background, bool withTray = true, bool withLoop = true)
    {
        Orb.Show(); WindowPlacement.InitializeOrb(Orb, _settings);
        Orb.ConfigureDocking(ViewModel.SnapToEdge, snapNow: false);
        if (withTray) _tray = new TrayService(ShowPanel, ToggleOrb, () => _ = RefreshAsync(), () => OpenWebsite("/app"),
            () => { ViewModel.SelectedTab = 4; ShowPanel(); ViewModel.Updater.CheckCommand.Execute(null); }, Quit);
        if (demo) UseDemo();
        else
        {
            _token = _storage.LoadSession();
            if (!KrillApiClient.IsPlausibleSession(_token)) _token = null;
        }
        if (!background) ShowPanel();
        if (_token is not null) await RefreshAsync(includeDetails: !background);
        if (withLoop) _ = PollAsync(_lifetime.Token);
        if (withLoop && !demo) ViewModel.Updater.Start();
    }

    public void ShowPanel()
    {
        if (_disposed) return;
        if (!Orb.IsVisible) Orb.Show();
        WindowPlacement.PlacePanel(Panel, Orb);
        Panel.Show(); Panel.Activate();
        Panel.ClearInitialFocus();
        // Re-evaluate after WPF has received the destination monitor's DPI change.
        Panel.Dispatcher.BeginInvoke(() => { if (Panel.IsVisible) WindowPlacement.PlacePanel(Panel, Orb); });
        if (!ViewModel.IsDemo && _token is not null) _ = RefreshAsync();
    }

    private void TogglePanel() { if (Panel.IsVisible) Panel.Hide(); else ShowPanel(); }
    private void ToggleOrb() { if (Orb.IsVisible) { Orb.Hide(); Panel.Hide(); } else Orb.Show(); }

    private void SaveOrbPosition()
    {
        var position = WindowPlacement.ClampOrb(Orb, ViewModel.SnapToEdge);
        Orb.ConfigureDocking(ViewModel.SnapToEdge, snapNow: ViewModel.SnapToEdge);
        position = WindowPlacement.ExpandedOrbPosition(Orb);
        _settings.OrbLeftPixels = position.X; _settings.OrbTopPixels = position.Y;
        TrySaveSettings();
    }

    private async Task ShowLoginAsync()
    {
        if (_login is not null) { _login.Activate(); return; }
        ViewModel.IsLoginOpen = true;
        var generation = _generation;
        _login = new LoginWindow(_storage, async (token, ct) =>
        {
            var profile = await _api.GetProfileAsync(token, ct);
            ct.ThrowIfCancellationRequested();
            if (_generation != generation) throw new OperationCanceledException();
            _storage.SaveSession(token);
            CancelSession();
            _token = token; _profile = profile; _failures = 0;
            ViewModel.ClearAccount(); ViewModel.NeedsLogin = false;
            await RefreshAsync();
        }) { Owner = Panel };
        _login.Closed += (_, _) => { _login = null; ViewModel.IsLoginOpen = false; ShowPanel(); };
        _login.Show();
        await Task.CompletedTask;
    }

    public async Task RefreshAsync(bool includeDetails = true)
    {
        if (_disposed) return;
        if (ViewModel.IsDemo) { UseDemo(); return; }
        var token = _token;
        if (token is null || !await _refreshGate.WaitAsync(0)) return;
        var generation = _generation;
        var ct = _session.Token;
        ViewModel.IsLoading = true;
        try
        {
            var subscriptions = _api.GetSubscriptionsAsync(token, ct);
            var profile = _profile is null ? _api.GetProfileAsync(token, ct) : Task.FromResult(_profile);
            await Task.WhenAll(subscriptions, profile);
            if (generation != _generation) return;
            _profile = await profile;
            ViewModel.ApplyCore(_profile, await subscriptions);
            _failures = 0;
            var optional = new List<Task>();
            if (includeDetails && (_lastResetCards == default || DateTimeOffset.UtcNow - _lastResetCards >= TimeSpan.FromSeconds(30)))
                optional.Add(LoadResetCardsAsync(token, generation, ct));
            if (includeDetails) optional.Add(LoadStatsAsync(token, generation, ct));
            if (_lastAnnouncements == default || DateTimeOffset.UtcNow - _lastAnnouncements > TimeSpan.FromMinutes(10))
                optional.Add(LoadAnnouncementsAsync(generation, ct));
            await Task.WhenAll(optional);
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
        catch (KrillApiException e)
        {
            if (generation == _generation) HandleFailure(e);
        }
        catch (Exception e)
        {
            _storage.Log("refresh_failed", e);
            if (generation == _generation) { _failures++; ViewModel.ReportFailure("暂时无法读取数据，稍后会自动重试。"); }
        }
        finally
        {
            if (generation == _generation) ViewModel.IsLoading = false;
            _refreshGate.Release();
        }
    }

    private async Task LoadStatsAsync(string token, int generation, CancellationToken ct)
    {
        try
        {
            var range = DisplayFormat.LocalDayRange(DateTimeOffset.Now, TimeZoneInfo.Local);
            var stats = await _api.GetStatsAsync(token, range.From, range.To, ct);
            if (generation == _generation) ViewModel.ApplyStats(stats);
        }
        catch (KrillApiException e)
        {
            if (generation != _generation) return;
            if (e.Failure == ApiFailure.SessionExpired) HandleFailure(e);
            else ViewModel.StatsNote = "今日统计暂时无法更新，已保留上次结果。";
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
    }

    private async Task LoadResetCardsAsync(string token, int generation, CancellationToken ct)
    {
        try
        {
            var result = await _api.GetResetCardsAsync(token, ct);
            if (generation != _generation) return;
            ViewModel.ApplyResetCards(result); _lastResetCards = DateTimeOffset.UtcNow;
        }
        catch (KrillApiException e)
        {
            if (generation != _generation) return;
            if (e.Failure == ApiFailure.SessionExpired) HandleFailure(e);
            else ViewModel.ReportResetCardsFailure();
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
        catch (Exception e)
        { _storage.Log("reset_cards_refresh_failed", e); if (generation == _generation) ViewModel.ReportResetCardsFailure(); }
    }

    private async Task LoadAnnouncementsAsync(int generation, CancellationToken ct)
    {
        try
        {
            var result = await _api.GetAnnouncementsAsync(ct);
            if (generation != _generation) return;
            ViewModel.ApplyAnnouncements(result); _lastAnnouncements = DateTimeOffset.UtcNow;
        }
        catch (KrillApiException)
        { if (generation == _generation) ViewModel.AnnouncementNote = "公告暂时无法更新，可稍后刷新或在官网查看。"; }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
    }

    private void HandleFailure(KrillApiException failure)
    {
        _failures++;
        var expired = failure.Failure == ApiFailure.SessionExpired;
        if (expired)
        {
            _token = null; ViewModel.IsLoading = false; CancelSession();
            try { _storage.DeleteSession(); } catch (Exception e) when (e is IOException or UnauthorizedAccessException) { _storage.Log("expired_session_cleanup_failed", e); }
        }
        ViewModel.ReportFailure(failure.FriendlyMessage, expired);
    }

    private async Task PollAsync(CancellationToken ct)
    {
        try
        {
            var lastRefresh = DateTimeOffset.UtcNow;
            while (!ct.IsCancellationRequested)
            {
                await Task.Delay(TimeSpan.FromSeconds(1), ct);
                ViewModel.AdvanceSubscriptionTime();
                var seconds = Panel.IsVisible ? Math.Min(ViewModel.RefreshSeconds, 30) : ViewModel.RefreshSeconds;
                seconds = Math.Min(300, seconds * (1 << Math.Min(_failures, 3)));
                if (DateTimeOffset.UtcNow - lastRefresh >= TimeSpan.FromSeconds(seconds))
                {
                    lastRefresh = DateTimeOffset.UtcNow;
                    if (_token is not null && !ViewModel.IsDemo) await RefreshAsync(Panel.IsVisible);
                }
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
        catch (Exception e) { _storage.Log("polling_failed", e); }
    }

    public void UseDemo()
    {
        CancelSession(); _token = null; _profile = null; _failures = 0;
        var now = DateTimeOffset.Now;
        ViewModel.ApplyCore(DemoData.Profile, DemoData.Subscriptions(now), demo: true);
        ViewModel.ApplyStats(DemoData.Stats(now)); ViewModel.ApplyAnnouncements(DemoData.Announcements(now));
        ViewModel.ApplyResetCards(DemoData.ResetCards(now));
        _tray?.Update(ViewModel.OrbText);
    }

    private void CancelSession()
    {
        _generation++; _session.Cancel(); _session.Dispose();
        _session = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _lastAnnouncements = default; _lastResetCards = default;
    }

    private void SignOut()
    {
        CancelSession(); _token = null; _profile = null; _failures = 0;
        _login?.Close(); ViewModel.ClearAccount();
        try { _storage.DeleteSession(); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        { _storage.Log("signout_cleanup_failed", e); ViewModel.ReportFailure("本机凭据未能清除，请检查应用数据目录的访问权限。", expired: true); }
    }

    private void SavePreferences()
    {
        if (!_ready || _savingPreferences || _disposed) return;
        _savingPreferences = true;
        try
        {
            if (_settings.StartWithWindows != ViewModel.StartWithWindows)
            {
                try { StartupRegistration.SetEnabled(ViewModel.StartWithWindows); _settings.StartWithWindows = ViewModel.StartWithWindows; }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException)
                { _storage.Log("startup_preference_failed", e); ViewModel.StartWithWindows = _settings.StartWithWindows; ViewModel.StatusDetail = "未能修改开机启动设置，请检查当前用户权限。"; }
            }
            if (_settings.SnapToEdge != ViewModel.SnapToEdge)
                Orb.ConfigureDocking(ViewModel.SnapToEdge, snapNow: false);
            _settings.AlwaysOnTop = ViewModel.AlwaysOnTop; _settings.SnapToEdge = ViewModel.SnapToEdge;
            _settings.PanelPinned = ViewModel.PanelPinned; _settings.RefreshSeconds = ViewModel.RefreshSeconds;
            TrySaveSettings();
        }
        finally { _savingPreferences = false; }
    }

    private void TrySaveSettings()
    {
        try { _storage.SaveSettings(_settings); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        { _storage.Log("settings_save_failed", e); ViewModel.StatusDetail = "本次设置暂时无法保存。"; }
    }
    private void MarkRead(long id) { ViewModel.MarkRead(id); TrySaveSettings(); }
    private void OpenWebsite(string path)
    {
        try { BrowserLauncher.Open(path); }
        catch (Exception e) when (e is Win32Exception or InvalidOperationException)
        { _storage.Log("website_open_failed", e); ViewModel.StatusDetail = "无法打开默认浏览器，请检查系统的浏览器设置。"; }
    }
    private void Quit() => Application.Current.Shutdown();

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true; _lifetime.Cancel(); _session.Cancel();
        ViewModel.Updater.Dispose();
        _tray?.Dispose(); _login?.Close();
        Orb.AllowClose = true; Panel.AllowClose = true;
        Orb.Close(); Panel.Close(); _http.Dispose();
        _session.Dispose(); _lifetime.Dispose();
    }
}
