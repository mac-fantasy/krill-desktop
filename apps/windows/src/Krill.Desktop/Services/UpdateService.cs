using System.Diagnostics;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Krill.Core;

namespace Krill.Desktop.Services;

public sealed partial class UpdateService : ObservableObject, IDisposable
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromMinutes(10) };
    private readonly CancellationTokenSource _lifetime = new();
    private ReleaseAsset? _asset;
    private bool _disposed;
    public string CurrentVersion { get; } = (Assembly.GetExecutingAssembly().GetName().Version?.ToString(3)) ?? "0.0.0";
    public string VersionLabel => $"KRILL DESKTOP  /  {CurrentVersion}";
    [ObservableProperty] private string status = "启动后自动检查更新，每 6 小时检查一次。";
    [ObservableProperty] private bool isBusy;
    [ObservableProperty] private string? availableVersion;
    public bool HasUpdate => AvailableVersion is not null;
    public IAsyncRelayCommand CheckCommand { get; }
    public IAsyncRelayCommand InstallCommand { get; }
    public UpdateService()
    {
        CheckCommand = new AsyncRelayCommand(CheckAsync, () => !IsBusy);
        InstallCommand = new AsyncRelayCommand(InstallAsync, () => !IsBusy && HasUpdate);
    }
    partial void OnIsBusyChanged(bool value) { CheckCommand.NotifyCanExecuteChanged(); InstallCommand.NotifyCanExecuteChanged(); }
    partial void OnAvailableVersionChanged(string? value) { OnPropertyChanged(nameof(HasUpdate)); InstallCommand.NotifyCanExecuteChanged(); }
    public void Start() => _ = PollAsync();
    private async Task PollAsync()
    {
        try
        {
            while (!_lifetime.IsCancellationRequested)
            {
                await CheckAsync();
                await Task.Delay(TimeSpan.FromHours(6), _lifetime.Token);
            }
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { }
    }
    private async Task CheckAsync()
    {
        if (IsBusy || _disposed) return;
        IsBusy = true; Status = "正在检查 GitHub Release…";
        try
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
            timeout.CancelAfter(TimeSpan.FromSeconds(30));
            var release = await GitHubRelease.LatestAsync(_http, timeout.Token);
            _asset = release?.UpdateAfter(ReleaseVersion.Parse(CurrentVersion)!, "win-x64");
            AvailableVersion = _asset is null ? null : ReleaseVersion.Parse(release!.TagName)!.ToString();
            Status = release is null ? "暂无可访问的正式版本，请确认发布仓库已公开。" : HasUpdate ? $"发现新版本 {AvailableVersion}，可下载并重启更新。" :
                ReleaseVersion.Parse(release.TagName)?.CompareTo(ReleaseVersion.Parse(CurrentVersion)) > 0 && !release.Draft && !release.Prerelease
                ? "新版本尚未提供可校验的 Windows 安装包，请稍后重试。" : $"当前已是最新版本（{CurrentVersion}）。";
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { }
        catch { Status = "检查更新失败，请检查网络或 GitHub 请求额度后重试。"; }
        finally { IsBusy = false; }
    }
    private async Task InstallAsync()
    {
        if (IsBusy || _asset is not { } asset || AvailableVersion is not { } version || _disposed) return;
        IsBusy = true; Status = $"正在下载 {version}…";
        var staging = Path.Combine(Path.GetTempPath(), "KrillUpdate-" + Guid.NewGuid().ToString("N"));
        var handedOff = false;
        try
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
            timeout.CancelAfter(TimeSpan.FromMinutes(10));
            var ct = timeout.Token;
            var destination = Path.GetFullPath(AppContext.BaseDirectory);
            // Probe write access before the application exits.
            var probe = Path.Combine(destination, ".krill-write-" + Guid.NewGuid().ToString("N"));
            await File.WriteAllTextAsync(probe, "", ct); File.Delete(probe);
            Directory.CreateDirectory(staging);
            var archive = Path.Combine(staging, "update.zip");
            using (var response = await _http.GetAsync(asset.BrowserDownloadUrl, HttpCompletionOption.ResponseHeadersRead, ct))
            {
                response.EnsureSuccessStatusCode();
                await using var input = await response.Content.ReadAsStreamAsync(ct);
                await using var output = File.Create(archive);
                var buffer = new byte[1_048_576]; long total = 0;
                int count;
                while ((count = await input.ReadAsync(buffer, ct)) > 0)
                {
                    total += count;
                    if (total > asset.Size) throw new InvalidDataException("安装包大小不匹配。");
                    await output.WriteAsync(buffer.AsMemory(0, count), ct);
                }
                if (total != asset.Size) throw new InvalidDataException("安装包下载不完整。");
            }
            Status = "正在校验并准备更新…";
            await using (var stream = File.OpenRead(archive))
            {
                var digest = Convert.ToHexStringLower(await SHA256.HashDataAsync(stream, ct));
                if (digest != asset.Sha256) throw new InvalidDataException("安装包 SHA-256 校验失败。");
            }
            var payload = Path.Combine(staging, "payload");
            await Task.Run(() => ZipFile.ExtractToDirectory(archive, payload), ct);
            var executable = Path.Combine(payload, "Krill.Desktop.exe");
            var assembly = Path.Combine(payload, "Krill.Desktop.dll");
            if (!File.Exists(executable) || !File.Exists(assembly) || AssemblyName.GetAssemblyName(assembly).Version?.ToString(3) != version)
                throw new InvalidDataException("安装包中的应用或版本不匹配。");
            // Configuration is JSON, so paths and original arguments never become PowerShell code.
            var configuration = Path.Combine(staging, "update.json");
            await File.WriteAllTextAsync(configuration, JsonSerializer.Serialize(new
            {
                ProcessId = Environment.ProcessId, Destination = destination, Payload = payload,
                Arguments = Environment.GetCommandLineArgs().Skip(1).ToArray()
            }), ct);
            var script = Path.Combine(staging, "install-update.ps1");
            using (var resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("Krill.Desktop.Services.install-update.ps1")!)
            using (var reader = new StreamReader(resource))
            {
                // Windows PowerShell 5.1 requires a BOM to read UTF-8 messages correctly.
                await File.WriteAllTextAsync(script, await reader.ReadToEndAsync(ct), new UTF8Encoding(true), ct);
            }
            ct.ThrowIfCancellationRequested();
            var start = new ProcessStartInfo("powershell.exe") { UseShellExecute = false, CreateNoWindow = true };
            foreach (var argument in new[] { "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", script, "-Configuration", configuration })
                start.ArgumentList.Add(argument);
            using var helper = Process.Start(start) ?? throw new IOException("无法启动更新组件。");
            handedOff = true;
            Application.Current.Shutdown();
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { }
        catch (Exception error)
        {
            Status = error is InvalidDataException ? error.Message + " 请重新下载。" : "更新失败，请确认应用目录可写并检查网络后重试。";
        }
        finally
        {
            if (!handedOff) { try { Directory.Delete(staging, true); } catch (IOException) { } catch (UnauthorizedAccessException) { } }
            IsBusy = false;
        }
    }
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true; _lifetime.Cancel(); _http.Dispose();
    }
}
