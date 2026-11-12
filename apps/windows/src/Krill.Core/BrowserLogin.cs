namespace Krill.Core;

public static class BrowserLogin
{
    public static readonly Uri LoginUrl = new(KrillApiClient.Origin, "/login");
    public const string ExportScript = """
        (() => {
          if (location.origin !== 'https://www.krill-code.com') { alert('请在 Krill 官网运行'); return; }
          const token = localStorage.getItem('krill_jwt');
          if (!token) { alert('请先完成 Krill 登录'); return; }
          prompt('复制 Krill 会话并粘贴到 Krill Desktop（请勿分享给他人）', token);
        })()
        """;

    public static string? SessionFromText(string text)
    {
        if (text.Length > 16400) return null;
        var value = text.Trim();
        if (value.StartsWith("Bearer ", StringComparison.Ordinal)) value = value[7..];
        return KrillApiClient.IsPlausibleSession(value) ? value : null;
    }

    public static bool AllowsNavigation(string value) => Uri.TryCreate(value, UriKind.Absolute, out var uri) &&
        uri.Scheme == Uri.UriSchemeHttps && uri.Port == 443 && !string.IsNullOrEmpty(uri.Host) && string.IsNullOrEmpty(uri.UserInfo);
}
