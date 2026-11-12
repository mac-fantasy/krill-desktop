using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Krill.Core;

public enum ApiFailure { SessionExpired, Forbidden, RateLimited, Network, Service, InvalidResponse }

public sealed class KrillApiException(ApiFailure failure) : Exception(failure.ToString())
{
    public ApiFailure Failure { get; } = failure;
    public string FriendlyMessage => Failure switch
    {
        ApiFailure.SessionExpired => "登录已失效，请重新连接账号。",
        ApiFailure.Forbidden => "当前账号暂时无权读取此项数据。",
        ApiFailure.RateLimited => "请求较频繁，稍后会自动重试。",
        ApiFailure.Network => "暂时无法连接，已保留上次成功读取的数据。",
        ApiFailure.InvalidResponse => "网站返回的数据格式有变化，请稍后重试。",
        _ => "网站暂时不可用，稍后会自动重试。"
    };
}

public sealed partial class KrillApiClient(HttpClient http)
{
    public static readonly Uri Origin = new("https://www.krill-code.com");

    // The HAR contains both a Bearer header and this first-party cookie. Keep both
    // until the server's independent authentication requirements are verified.
    private async Task<T> SendAsync<T>(HttpMethod method, string path, string? token,
        object? body = null, CancellationToken cancellationToken = default)
    {
        if (!path.StartsWith("/api/", StringComparison.Ordinal) || path.Contains('?') || path.Contains('#'))
            throw new ArgumentException("Only fixed Krill API paths are allowed.", nameof(path));

        using var request = new HttpRequestMessage(method, new Uri(Origin, path));
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        request.Headers.Add("X-Language", "zh-CN");
        request.Headers.AcceptLanguage.ParseAdd("zh-CN");
        request.Headers.UserAgent.ParseAdd("KrillDesktop/0.1.0");
        if (token is not null)
        {
            if (!IsPlausibleSession(token)) throw new KrillApiException(ApiFailure.SessionExpired);
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
            request.Headers.Add("Cookie", $"krill_jwt={Uri.EscapeDataString(token)}");
        }
        if (body is not null) request.Content = JsonContent.Create(body, options: KrillJson.Options);

        try
        {
            using var response = await http.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            if (response.StatusCode == HttpStatusCode.Unauthorized) throw new KrillApiException(ApiFailure.SessionExpired);
            if (response.StatusCode == HttpStatusCode.Forbidden) throw new KrillApiException(ApiFailure.Forbidden);
            if ((int)response.StatusCode == 429) throw new KrillApiException(ApiFailure.RateLimited);
            if (!response.IsSuccessStatusCode) throw new KrillApiException(ApiFailure.Service);
            // A bounded read also protects the desktop process from unexpected HTML or oversized data.
            await response.Content.LoadIntoBufferAsync(8 * 1024 * 1024, cancellationToken);
            var result = await response.Content.ReadFromJsonAsync<ApiEnvelope<T>>(KrillJson.Options, cancellationToken);
            if (result is null) throw new KrillApiException(ApiFailure.InvalidResponse);
            if (result.Code == 401) throw new KrillApiException(ApiFailure.SessionExpired);
            if (!result.Success || result.Data is null) throw new KrillApiException(ApiFailure.InvalidResponse);
            return result.Data;
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        { throw new KrillApiException(ApiFailure.Network); }
        catch (HttpRequestException) { throw new KrillApiException(ApiFailure.Network); }
        catch (JsonException) { throw new KrillApiException(ApiFailure.InvalidResponse); }
    }

    public Task<UserProfile> GetProfileAsync(string token, CancellationToken ct = default) =>
        SendAsync<UserProfile>(HttpMethod.Get, "/api/auth/me", token, cancellationToken: ct);

    public Task<SubscriptionResponse> GetSubscriptionsAsync(string token, CancellationToken ct = default) =>
        SendAsync<SubscriptionResponse>(HttpMethod.Get, "/api/subscription", token, cancellationToken: ct);

    public Task<ResetCardsResponse> GetResetCardsAsync(string token, CancellationToken ct = default) =>
        SendAsync<ResetCardsResponse>(HttpMethod.Get, "/api/reset-cards", token, cancellationToken: ct);

    public Task<RequestStats> GetStatsAsync(string token, DateTimeOffset from, DateTimeOffset to, CancellationToken ct = default) =>
        SendAsync<RequestStats>(HttpMethod.Post, "/api/request-logs/stats", token,
            new { StartTime = from.ToUniversalTime().ToString("O"), EndTime = to.ToUniversalTime().ToString("O") }, ct);

    public Task<AnnouncementsResponse> GetAnnouncementsAsync(CancellationToken ct = default) =>
        SendAsync<AnnouncementsResponse>(HttpMethod.Get, "/api/public/announcements", null, cancellationToken: ct);

    public static bool IsPlausibleSession(string? value) => value is { Length: >= 16 and <= 16384 } && SessionPattern().IsMatch(value);

    public static bool IsTrustedPage(Uri? uri) => uri is not null && uri.Scheme == Uri.UriSchemeHttps &&
        uri.Host.Equals(Origin.Host, StringComparison.OrdinalIgnoreCase) && uri.Port == 443 && string.IsNullOrEmpty(uri.UserInfo);

    [GeneratedRegex(@"\A[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\z", RegexOptions.CultureInvariant)]
    private static partial Regex SessionPattern();
}
