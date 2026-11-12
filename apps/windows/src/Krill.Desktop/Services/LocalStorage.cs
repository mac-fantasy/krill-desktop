using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Win32;

namespace Krill.Desktop.Services;

public sealed class ClientSettings
{
    public bool AlwaysOnTop { get; set; } = true;
    public bool SnapToEdge { get; set; } = true;
    public bool PanelPinned { get; set; }
    public bool StartWithWindows { get; set; }
    public int RefreshSeconds { get; set; } = 60;
    public int? OrbLeftPixels { get; set; }
    public int? OrbTopPixels { get; set; }
    public long? SelectedSubscriptionId { get; set; }
    public long LastReadAnnouncementId { get; set; }
}

public sealed class LocalStorage(string directory)
{
    public string DirectoryPath { get; } = Path.GetFullPath(directory);
    private string SettingsPath => Path.Combine(DirectoryPath, "settings.json");
    private string SessionPath => Path.Combine(DirectoryPath, "session.dat");
    private static readonly byte[] Entropy = Encoding.UTF8.GetBytes("Krill.Desktop/session/v1");

    public ClientSettings LoadSettings()
    {
        try
        {
            var settings = JsonSerializer.Deserialize<ClientSettings>(File.ReadAllText(SettingsPath),
                new JsonSerializerOptions { PropertyNameCaseInsensitive = true }) ?? new();
            if (settings.RefreshSeconds is not (30 or 60 or 120 or 300)) settings.RefreshSeconds = 60;
            return settings;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { return new(); }
    }

    public void SaveSettings(ClientSettings settings) => WriteAtomic(SettingsPath,
        JsonSerializer.SerializeToUtf8Bytes(settings, new JsonSerializerOptions { WriteIndented = true }));

    public string? LoadSession()
    {
        try { return Encoding.UTF8.GetString(ProtectedData.Unprotect(File.ReadAllBytes(SessionPath), Entropy, DataProtectionScope.CurrentUser)); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or CryptographicException) { return null; }
    }

    public void SaveSession(string token) => WriteAtomic(SessionPath,
        ProtectedData.Protect(Encoding.UTF8.GetBytes(token), Entropy, DataProtectionScope.CurrentUser));

    public void DeleteSession()
    {
        if (File.Exists(SessionPath)) File.Delete(SessionPath);
    }

    private void WriteAtomic(string path, byte[] content)
    {
        Directory.CreateDirectory(DirectoryPath);
        var temporary = path + ".tmp";
        File.WriteAllBytes(temporary, content);
        File.Move(temporary, path, overwrite: true);
    }

    // Do not record exception messages, request bodies, URLs, headers, or tokens.
    public void Log(string eventName, Exception? exception = null)
    {
        try
        {
            Directory.CreateDirectory(DirectoryPath);
            var path = Path.Combine(DirectoryPath, "events.log");
            if (File.Exists(path) && new FileInfo(path).Length > 256_000) File.Delete(path);
            File.AppendAllText(path, $"{DateTimeOffset.UtcNow:O} {eventName} {exception?.GetType().Name}\n");
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
    }
}

public static class StartupRegistration
{
    private const string KeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    public static void SetEnabled(bool enabled)
    {
        using var key = Registry.CurrentUser.CreateSubKey(KeyPath);
        if (enabled)
        {
            var executable = Environment.ProcessPath ?? throw new IOException("Executable path unavailable.");
            key.SetValue("KrillDesktop", $"\"{executable}\" --background");
        }
        else key.DeleteValue("KrillDesktop", throwOnMissingValue: false);
    }
}
