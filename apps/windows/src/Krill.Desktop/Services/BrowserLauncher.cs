using System.Diagnostics;
using Krill.Core;

namespace Krill.Desktop.Services;

public static class BrowserLauncher
{
    public static void Open(string path)
    {
        if (path is not ("/app" or "/app/account" or "/app/shop" or "/app/logs" or "/app/announcements"))
            throw new ArgumentException("Unknown website destination.", nameof(path));
        Process.Start(new ProcessStartInfo(new Uri(KrillApiClient.Origin, path).AbsoluteUri) { UseShellExecute = true });
    }
    public static void OpenRuntimeDownload() => Process.Start(new ProcessStartInfo(
        "https://developer.microsoft.com/microsoft-edge/webview2/") { UseShellExecute = true });
    public static void OpenLogin() => Process.Start(new ProcessStartInfo(BrowserLogin.LoginUrl.AbsoluteUri) { UseShellExecute = true });
}
