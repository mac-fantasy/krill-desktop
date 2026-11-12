using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Krill.Desktop.Services;

namespace Krill.Desktop;

public partial class App : Application
{
    private AppController? _controller;
    private Mutex? _instance;
    private EventWaitHandle? _showEvent;
    private RegisteredWaitHandle? _showRegistration;
    private LocalStorage? _storage;
    private bool _ownsMutex;
    private bool _smokeTest;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.GetCultureInfo("zh-CN");
        CultureInfo.DefaultThreadCurrentUICulture = CultureInfo.GetCultureInfo("zh-CN");
        var demo = e.Args.Contains("--demo"); _smokeTest = e.Args.Contains("--smoke-test");
        var directory = _smokeTest
            ? Path.Combine(Argument(e.Args, "--data-dir") ?? Path.Combine(Environment.CurrentDirectory, "artifacts", "verify-data"), "SmokeTest")
            : Argument(e.Args, "--data-dir") ?? Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "KrillDesktop", demo ? "Demo" : "Account");
        _storage = new LocalStorage(directory);
        DispatcherUnhandledException += (_, error) =>
        {
            _storage.Log("ui_unhandled_exception", error.Exception); error.Handled = true;
            if (_smokeTest) WriteSmokeFailure(e.Args, error.Exception);
            if (!_smokeTest) MessageBox.Show("程序遇到异常，请重新启动。账号凭据不会写入诊断日志。", "Krill Desktop", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
        };
        try
        {
            var instanceKey = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(_storage.DirectoryPath)))[..16];
            _instance = new Mutex(true, @"Local\KrillDesktop-" + instanceKey, out _ownsMutex);
            _showEvent = new EventWaitHandle(false, EventResetMode.AutoReset, @"Local\KrillDesktop-Show-" + instanceKey);
            if (!_ownsMutex) { _showEvent.Set(); Shutdown(); return; }
            _controller = new AppController(_storage);
            _showRegistration = ThreadPool.RegisterWaitForSingleObject(_showEvent, (_, _) =>
                Dispatcher.BeginInvoke(() => _controller?.ShowPanel()), null, Timeout.Infinite, false);
            await _controller.StartAsync(demo || _smokeTest, e.Args.Contains("--background"), withTray: !_smokeTest, withLoop: !_smokeTest);
            if (_smokeTest)
            {
                await UiVerifier.RunAsync(_controller, _storage, Argument(e.Args, "--output") ?? Path.Combine(Environment.CurrentDirectory, "artifacts", "ui"));
                Shutdown();
            }
        }
        catch (Exception error)
        {
            _storage.Log("startup_failed", error);
            if (_smokeTest) WriteSmokeFailure(e.Args, error);
            if (!_smokeTest) MessageBox.Show("无法启动 Krill Desktop，请检查应用目录和运行环境。", "Krill Desktop", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
        }
    }

    private static string? Argument(string[] args, string name)
    { var index = Array.IndexOf(args, name); return index >= 0 && index + 1 < args.Length ? args[index + 1] : null; }

    private static void WriteSmokeFailure(string[] args, Exception error)
    {
        var output = Argument(args, "--output") ?? Path.Combine(Environment.CurrentDirectory, "artifacts", "ui");
        Directory.CreateDirectory(output);
        // Smoke mode never loads account credentials or makes account requests.
        File.WriteAllText(Path.Combine(output, "failure.txt"), error.ToString());
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _showRegistration?.Unregister(null); _controller?.Dispose(); _showEvent?.Dispose();
        if (_ownsMutex) _instance?.ReleaseMutex(); _instance?.Dispose();
        base.OnExit(e);
    }
}
