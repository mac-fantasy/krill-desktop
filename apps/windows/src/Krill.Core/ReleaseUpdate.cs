using System.Net;
using System.Net.Http.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace Krill.Core;

public sealed partial record ReleaseVersion(int Major, int Minor, int Patch) : IComparable<ReleaseVersion>
{
    [GeneratedRegex(@"\Av?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z")]
    private static partial Regex VersionPattern();
    public static ReleaseVersion? Parse(string text)
    {
        var match = VersionPattern().Match(text);
        if (!match.Success) return null;
        var numbers = new int[3];
        for (var i = 0; i < 3; i++)
            if (!int.TryParse(match.Groups[i + 1].Value, out numbers[i]) || numbers[i] > 65535) return null;
        return new(numbers[0], numbers[1], numbers[2]);
    }
    public int CompareTo(ReleaseVersion? other) => other is null ? 1 : (Major, Minor, Patch).CompareTo((other.Major, other.Minor, other.Patch));
    public override string ToString() => $"{Major}.{Minor}.{Patch}";
}

public sealed record ReleaseAsset(
    string Name, [property: JsonPropertyName("browser_download_url")] string BrowserDownloadUrl, string? Digest, long Size)
{
    public string? Sha256 => Digest is { Length: 71 } && Digest.StartsWith("sha256:", StringComparison.Ordinal) &&
        Digest[7..].All(char.IsAsciiHexDigit) ? Digest[7..].ToLowerInvariant() : null;
}

public sealed record GitHubRelease([property: JsonPropertyName("tag_name")] string TagName, bool Draft, bool Prerelease, ReleaseAsset[] Assets)
{
    public const string Repository = "mac-fantasy/krill-desktop";
    public const string LatestUrl = $"https://api.github.com/repos/{Repository}/releases/latest";
    public ReleaseAsset? UpdateAfter(ReleaseVersion current, string platform)
    {
        var version = ReleaseVersion.Parse(TagName);
        if (Draft || Prerelease || version is null || TagName != $"v{version}" || version.CompareTo(current) <= 0) return null;
        var name = $"Krill.Desktop-{version}-{platform}.zip";
        return Assets?.FirstOrDefault(asset => asset.Name == name && asset.Sha256 is not null &&
            asset.Size is > 0 and <= 1_073_741_824 &&
            asset.BrowserDownloadUrl == $"https://github.com/{Repository}/releases/download/{TagName}/{name}");
    }
    public static async Task<GitHubRelease?> LatestAsync(HttpClient http, CancellationToken ct)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, LatestUrl);
        request.Headers.UserAgent.ParseAdd("KrillDesktop-Updater/1.0");
        request.Headers.Accept.ParseAdd("application/vnd.github+json");
        request.Headers.Add("X-GitHub-Api-Version", "2022-11-28");
        using var response = await http.SendAsync(request, ct);
        if (response.StatusCode == HttpStatusCode.NotFound) return null;
        response.EnsureSuccessStatusCode();
        return await response.Content.ReadFromJsonAsync<GitHubRelease>(cancellationToken: ct);
    }
}
