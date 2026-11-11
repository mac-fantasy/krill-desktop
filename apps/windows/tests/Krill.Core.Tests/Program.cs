using System.Globalization;
using System.Net;
using System.Text;
using System.Text.Json;
using Krill.Core;

var tests = new (string Name, Func<Task> Run)[]
{
    ("Usage chart sorts before taking the latest 48 buckets and keeps local time ticks", () => Sync(() =>
    {
        var start = DateTimeOffset.Parse("2026-11-10T23:00:00Z", CultureInfo.InvariantCulture);
        var source = Enumerable.Range(0, 60).Select(i => new TrendBucket { BucketStart = start.AddHours(i), TotalCostUsd = i }).ToArray();
        var data = new TrendChartData(source.Reverse());
        Equal(48, data.Buckets.Count); Equal(source[12], data.Buckets[0]); Equal(source[59], data.Buckets[^1]);
        Equal("0,12,24,35,47", string.Join(",", data.AxisIndices));
        var midnight = new TrendChartData(source.Take(2));
        Equal("23:00", midnight.TimeLabel(0, TimeZoneInfo.Utc)); Equal("00:00", midnight.TimeLabel(1, TimeZoneInfo.Utc));
        Equal(0, new TrendChartData([]).AxisIndices.Count());
        Equal("0", string.Join(",", new TrendChartData(source.Take(1)).AxisIndices));
    })),
    ("Usage chart preserves unknown, zero and small costs with exact request counts", () => Sync(() =>
    {
        var start = DateTimeOffset.Parse("2026-11-10T23:00:00Z", CultureInfo.InvariantCulture);
        var data = new TrendChartData(new decimal?[] { null, 0, -1, 0.0007m, 2 }.Select((cost, i) =>
            new TrendBucket { BucketStart = start.AddHours(i), TotalCostUsd = cost, RequestCount = 12345 }));
        Equal("3,3,3,3,88", string.Join(",", Enumerable.Range(0, 5).Select(data.BarHeight)));
        Equal(true, data.Tooltip(0).Contains("费用 —")); Equal(true, data.Tooltip(1).Contains("费用 $0.00"));
        Equal($"{DisplayFormat.LocalDate(start.AddHours(3))} · 费用 $0.0007 · 12345 次请求", data.Tooltip(3));
    })),
    ("Stats decode cache counters, channel rates, requests and speed", () => Sync(() =>
    {
        var stats = JsonSerializer.Deserialize<ApiEnvelope<RequestStats>>(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "stats.json")), KrillJson.Options)!.Data!;
        Equal(2L, stats.FailedRequests); Equal(64m, AccountDetails.CachePercent(stats));
        Equal(640000L, stats.CacheReadInputTokens); Equal(120000L, stats.CacheCreationInputTokens);
        Equal(48500L, stats.ReasoningTokens); Equal(1.8m, stats.Rpm); Equal(17842m, stats.Tpm);
        Equal(0.64m, stats.ChannelCacheRates[0].CacheRate); Equal("64.0%", AccountDetails.Percentage(AccountDetails.CachePercent(stats)));
    })),
    ("Cache rates prefer dedicated counters and require complete fallback data", () => Sync(() =>
    {
        var stats = new RequestStats { InputTokens = 50, CacheCreationInputTokens = 10, CacheReadInputTokens = 60 };
        Equal(50m, AccountDetails.CachePercent(stats));
        stats.CacheRateReadTokens = 10; stats.CacheRateEligibleTokens = 100;
        Equal(10m, AccountDetails.CachePercent(stats));
        foreach (var value in new RequestStats[] { new(),
            new() { CacheReadInputTokens = 60, CacheCreationInputTokens = 10 },
            new() { InputTokens = 50, CacheReadInputTokens = 60 },
            new() { CacheRateReadTokens = 0, CacheRateEligibleTokens = 0 },
            new() { CacheRateReadTokens = 101, CacheRateEligibleTokens = 100 },
            new() { CacheRateReadTokens = -1, CacheRateEligibleTokens = 100 } })
            Equal(null, AccountDetails.CachePercent(value));
        Equal(0m, AccountDetails.CachePercent(new() { CacheRateReadTokens = 0, CacheRateEligibleTokens = 100 }));
        Equal("—", AccountDetails.Percentage(null));
    })),
    ("Request cards use shared rolling windows without inventing remaining quota", () => Sync(() =>
    {
        var response = JsonSerializer.Deserialize<ApiEnvelope<SubscriptionResponse>>(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "request-count.json")), KrillJson.Options)!.Data!;
        var shared = response.RequestCountQuota!;
        Equal(95L, shared.Used5h); Equal(100L, shared.Limit5h); Equal(100L, response.Subscriptions[0].Plan.RateLimit5h);
        var windows = AccountDetails.Windows(shared);
        Equal(5d, windows[0].Percent); Equal(70d, windows[1].Percent); Equal(80.5d, windows[2].Percent);
        var quota = QuotaProjection.From(response.Subscriptions[0], shared);
        Equal("按次额度 · 5 小时", quota.Cycle); Equal(5m, quota.Remaining); Equal(5d, quota.Percent);
        Equal("周期 2099-10-01 ~ 2099-11-01", AccountDetails.Cycle(shared));
        Equal(null, QuotaProjection.From(response.Subscriptions[0]).Remaining);
        shared.Used5h = null;
        Equal(70d, QuotaProjection.From(response.Subscriptions[0], shared).Percent);
        shared.Used5h = -1; Equal(null, AccountDetails.Windows(shared)[0].Percent);
    })),
    ("Reset cards preserve history, usable subscriptions and exact expiry", async () =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "reset-cards.json"));
        var cards = JsonSerializer.Deserialize<ApiEnvelope<ResetCardsResponse>>(json, KrillJson.Options)!.Data!.Items;
        var now = DateTimeOffset.Parse("2099-10-20T00:00:00Z", CultureInfo.InvariantCulture);
        var states = new[] { ResetCardState.Unused, ResetCardState.Used, ResetCardState.Revoked, ResetCardState.Expired, ResetCardState.Unused, ResetCardState.Unknown, ResetCardState.Expired };
        for (var i = 0; i < cards.Count; i++) Equal(states[i], AccountDetails.State(cards[i], now));
        Equal(1, cards.Count(x => AccountDetails.CanUse(x, now)));
        Equal(30m, cards[0].CapUsd); Equal(12.50m, cards[1].GrantedUsd);
        Equal("可用 · 1 个订阅", AccountDetails.AvailabilityText(cards[0], now));
        Equal("未使用 · 暂无适用订阅", AccountDetails.AvailabilityText(cards[4], now));
        Equal(false, AccountDetails.CanUse(cards[0], now.AddDays(1)));
        using var http = new HttpClient(new Handler(request =>
        {
            Equal(HttpMethod.Get, request.Method); Equal("https://www.krill-code.com/api/reset-cards", request.RequestUri!.AbsoluteUri);
            Equal(null, request.Content); Equal("syntheticxx.payloadxx.signaturexx", request.Headers.Authorization!.Parameter);
            return Task.FromResult(Json(json));
        }));
        Equal(7, (await new KrillApiClient(http).GetResetCardsAsync("syntheticxx.payloadxx.signaturexx")).Items.Count);
    }),

    ("Monthly rollover skips exhausted, expired and upcoming cards and prefers another month", () => Sync(() =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "subscription-selection.json"));
        var plans = JsonSerializer.Deserialize<ApiEnvelope<SubscriptionResponse>>(json, KrillJson.Options)!.Data!.Subscriptions;
        using var fixture = JsonDocument.Parse(json);
        var now = fixture.RootElement.GetProperty("evaluation_time").GetDateTimeOffset();
        Equal(205L, SubscriptionSelection.Select(plans, 201, now)!.SubscriptionId);
        Equal(205L, SubscriptionSelection.Select(plans, 203, now)!.SubscriptionId);
        Equal(205L, SubscriptionSelection.Select(plans.AsEnumerable().Reverse(), 201, now)!.SubscriptionId);
        Equal(100L, SubscriptionSelection.Select(plans, 100, now)!.SubscriptionId);
        Equal(206L, SubscriptionSelection.Select(plans, 206, now)!.SubscriptionId);
        Equal(100L, SubscriptionSelection.Select(plans, null, now)!.SubscriptionId);
        Equal(205L, SubscriptionSelection.Select(plans, 999, now, preferredWasMonthly: true)!.SubscriptionId);
        Equal(now.AddSeconds(10), SubscriptionSelection.NextTransition(plans, now));
        Equal(202L, SubscriptionSelection.Select(plans, 205, now.AddSeconds(10))!.SubscriptionId);
    })),
    ("Monthly availability preserves unknown quotas and usable carryover", () => Sync(() =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "subscription-selection.json"));
        var plans = JsonSerializer.Deserialize<ApiEnvelope<SubscriptionResponse>>(json, KrillJson.Options)!.Data!.Subscriptions;
        using var fixture = JsonDocument.Parse(json);
        var now = fixture.RootElement.GetProperty("evaluation_time").GetDateTimeOffset();
        var expected = new Dictionary<long, SubscriptionAvailability>
        {
            [100] = SubscriptionAvailability.Available, [201] = SubscriptionAvailability.Exhausted,
            [202] = SubscriptionAvailability.NotStarted, [203] = SubscriptionAvailability.Expired,
            [204] = SubscriptionAvailability.Frozen, [205] = SubscriptionAvailability.Available,
            [206] = SubscriptionAvailability.Available, [207] = SubscriptionAvailability.Exhausted,
            [208] = SubscriptionAvailability.Available, [209] = SubscriptionAvailability.Available,
            [210] = SubscriptionAvailability.Exhausted
        };
        foreach (var plan in plans) Equal(expected[plan.SubscriptionId], SubscriptionSelection.Availability(plan, now));
        foreach (var id in new long[] { 208, 209 }) Equal(id, SubscriptionSelection.Select(plans, id, now)!.SubscriptionId);
        var unavailable = plans.Where(x => new long[] { 201, 202, 203, 204, 207, 210 }.Contains(x.SubscriptionId)).ToArray();
        Equal(null, SubscriptionSelection.Select(unavailable, 201, now));
        Equal(100L, SubscriptionSelection.Select(unavailable.Append(plans[0]), 201, now)!.SubscriptionId);
        plans[0].Quota.RemainingUsd = 0;
        Equal(SubscriptionAvailability.Available, SubscriptionSelection.Availability(plans[0], now));
        plans[0].Plan.DurationDays = 30;
        Equal(SubscriptionAvailability.Exhausted, SubscriptionSelection.Availability(plans[0], now));
    })),
    ("Login supports chained HTTPS providers but only trusts Krill for credentials", () => Sync(() =>
    {
        foreach (var value in new[] { "https://github.com/login", "https://appleid.apple.com/auth/authorize",
            "https://accounts.google.com", "https://connect.linux.do/oauth2/authorize", "https://sso.example.org/login" })
        {
            Equal(true, BrowserLogin.AllowsNavigation(value));
            Equal(false, KrillApiClient.IsTrustedPage(new Uri(value)));
        }
        foreach (var value in new[] { "http://github.com", "javascript:alert(1)", "file:///tmp/login.html",
            "https://user:password@github.com", "https://github.com:444", "about:blank", "" })
            Equal(false, BrowserLogin.AllowsNavigation(value));
        Equal(false, KrillApiClient.IsTrustedPage(new Uri("https://www.krill-code.com.evil.example")));
    })),
    ("Browser session import rejects URLs, scripts and header injection", () => Sync(() =>
    {
        const string token = "syntheticxx.payloadxx.signaturexx";
        Equal(token, BrowserLogin.SessionFromText(" \n" + token + "\r\n"));
        Equal(token, BrowserLogin.SessionFromText("Bearer " + token));
        foreach (var value in new[] { "", "null", "undefined", "\"" + token + "\"", "https://www.krill-code.com/#token=" + token,
            token + "\r\nX-Test: injected", BrowserLogin.ExportScript, new string('a', 16401) })
            Equal(null, BrowserLogin.SessionFromText(value));
    })),
    ("Release versions compare numerically and reject unsupported tags", () => Sync(() =>
    {
        Equal(true, ReleaseVersion.Parse("v1.10.2")!.CompareTo(ReleaseVersion.Parse("1.9.99")) > 0);
        foreach (var value in new[] { "1.2", "1.2.3.4", "1.2.3-beta.1", "01.2.3", "1.02.3", "-1.2.3", "1.2.65536", "1.2.٣", "1.2.3\n" })
            Equal(null, ReleaseVersion.Parse(value));
    })),
    ("Shared release contract rejects drafts, downgrades, missing digests and foreign assets", () => Sync(() =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Updates", "latest.json"));
        var release = JsonSerializer.Deserialize<GitHubRelease>(json, new JsonSerializerOptions(JsonSerializerDefaults.Web))!;
        var current = ReleaseVersion.Parse("1.9.0")!;
        Equal(1024L, release.UpdateAfter(current, "win-x64")!.Size);
        Equal(2048L, release.UpdateAfter(current, "macos-universal")!.Size);
        Equal(null, release.UpdateAfter(current, "win-arm64"));
        Equal(null, release.UpdateAfter(ReleaseVersion.Parse("1.10.2")!, "win-x64"));
        Equal(null, release.UpdateAfter(ReleaseVersion.Parse("2.0.0")!, "win-x64"));
        Equal(null, (release with { Draft = true }).UpdateAfter(current, "win-x64"));
        Equal(null, (release with { Prerelease = true }).UpdateAfter(current, "win-x64"));
        var asset = release.Assets[0];
        foreach (var invalid in new[] { asset with { Digest = null }, asset with { Digest = "sha256:bad" },
            asset with { Digest = "sha256:" + new string('z', 64) }, asset with { Size = 0 }, asset with { Size = 1_073_741_825 },
            asset with { BrowserDownloadUrl = "https://evil.example/update.zip" },
            asset with { BrowserDownloadUrl = asset.BrowserDownloadUrl.Replace("https:", "http:") } })
            Equal(null, (release with { Assets = [invalid] }).UpdateAfter(current, "win-x64"));
    })),
    ("Release checks use GitHub anonymously and handle an unpublished repository", async () =>
    {
        using var http = new HttpClient(new Handler(request =>
        {
            Equal(GitHubRelease.LatestUrl, request.RequestUri!.AbsoluteUri);
            Equal(null, request.Headers.Authorization); Equal(false, request.Headers.Contains("Cookie"));
            Equal(true, request.Headers.UserAgent.Count > 0);
            return Task.FromResult(Json(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Updates", "latest.json"))));
        }));
        Equal("v1.10.2", (await GitHubRelease.LatestAsync(http, default))!.TagName);
        using var empty = new HttpClient(new Handler(_ => Task.FromResult(new HttpResponseMessage(HttpStatusCode.NotFound))));
        Equal(null, await GitHubRelease.LatestAsync(empty, default));
    }),
    ("Shared Windows and macOS subscription fixtures preserve projection rules", () => Sync(() =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "subscriptions.json"));
        var response = JsonSerializer.Deserialize<ApiEnvelope<SubscriptionResponse>>(json, KrillJson.Options)!;
        Equal(true, response.Success); var data = response.Data!;
        Equal(24.80m, data.CreditBalanceUsd); Equal(3.50m, data.WelfareBalanceUsd);
        var weekly = QuotaProjection.From(data.Subscriptions[0]);
        Equal("本周额度", weekly.Cycle); Equal(100m, weekly.Limit); Equal(72.60m, weekly.Remaining);
        Equal(72.6, weekly.Percent); Equal(8.20m, weekly.Forwarded);
        Equal(75d, QuotaProjection.From(data.Subscriptions[1]).Percent);
        Equal(80d, QuotaProjection.From(data.Subscriptions[2]).Percent);
        Equal(null, QuotaProjection.From(data.Subscriptions[3]).Percent);
    })),
    ("Shared Windows and macOS statistics fixture preserves numeric strings", () => Sync(() =>
    {
        var json = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "stats.json"));
        var stats = JsonSerializer.Deserialize<ApiEnvelope<RequestStats>>(json, KrillJson.Options)!.Data!;
        Equal(128L, stats.TotalRequests); Equal(1_284_600L, stats.TotalTokens);
        Equal("$0.0007", DisplayFormat.Cost(stats.TotalCostUsd)); Equal(9L, stats.Trend[0].RequestCount);
    })),
    ("HAR-shaped amounts deserialize exactly and missing balances stay unknown", () => Sync(() =>
    {
        var data = JsonSerializer.Deserialize<SubscriptionResponse>("""
        {"credit_balance_usd":"24.80","subscriptions":[{"subscription_id":1,"status":"active","plan":{"name":"Sample","billing_type":"usd_weekly"},"quota":{"daily_limit_usd":"100.0000","used_usd":"27.4000","remaining_usd":"72.6000","forwarded_remaining_usd":"8.20","window_reset_at":"2026-10-19T00:00:00+08:00"}}]}
        """, KrillJson.Options)!;
        Equal(24.80m, data.CreditBalanceUsd); Equal(null, data.WelfareBalanceUsd);
        var quota = QuotaProjection.From(data.Subscriptions[0]);
        Equal(72.60m, quota.Remaining); Equal(8.20m, quota.Forwarded); Equal("本周额度", quota.Cycle);
        Equal(72.6, quota.Percent); Equal("$72.60", DisplayFormat.Amount(quota, quota.Remaining));
        Equal(TimeSpan.FromHours(8), quota.ResetAt!.Value.Offset);
    })),
    ("Current window and lifetime totals remain separate", () => Sync(() =>
    {
        var s = DemoData.Subscriptions(DateTimeOffset.Now).Subscriptions[0];
        s.TotalLimitUsd = 400; s.TotalRemainingUsd = 200;
        Equal(72.60m, QuotaProjection.From(s).Remaining);
        Equal(100m, QuotaProjection.From(s).Limit);
    })),
    ("Zero and missing quotas do not create a percentage", () => Sync(() =>
    {
        Equal(null, new QuotaProjection("未知", 0, 0, 0, null, null, true, "USD").Percent);
        Equal(null, new QuotaProjection("未知", 100, 0, null, null, null, true, "USD").Percent);
        Equal(0d, new QuotaProjection("额度", 100, 110, -10, null, null, true, "USD").Percent);
        Equal(100d, new QuotaProjection("额度", 100, 0, 120, null, null, true, "USD").Percent);
    })),
    ("Unsupported request-count fields stay unknown", () => Sync(() =>
    {
        var q = QuotaProjection.From(new Subscription { Plan = new() { BillingType = "request_count" } });
        Equal(false, q.IsMoney); Equal(null, q.Percent); Equal("—", DisplayFormat.Amount(q, q.Remaining));
    })),
    ("Currency display is independent of desktop number culture", () => Sync(() =>
    {
        var previous = CultureInfo.CurrentCulture;
        try { CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("de-DE"); Equal("$1,234.50", DisplayFormat.Money(1234.5m)); Equal("$0.0007", DisplayFormat.Cost(.0007m)); }
        finally { CultureInfo.CurrentCulture = previous; }
    })),
    ("Local-day requests use the correct UTC boundary", () => Sync(() =>
    {
        var zone = TimeZoneInfo.FindSystemTimeZoneById("China Standard Time");
        var range = DisplayFormat.LocalDayRange(DateTimeOffset.Parse("2026-10-15T00:30:00+08:00"), zone);
        Equal(DateTimeOffset.Parse("2026-10-14T16:00:00Z"), range.From);
        Equal(DateTimeOffset.Parse("2026-10-14T16:30:00Z"), range.To);
    })),
    ("Daylight-saving transition uses midnight's own offset", () => Sync(() =>
    {
        var zone = TimeZoneInfo.FindSystemTimeZoneById("Eastern Standard Time");
        var range = DisplayFormat.LocalDayRange(DateTimeOffset.Parse("2026-03-08T12:00:00-04:00"), zone);
        Equal(DateTimeOffset.Parse("2026-03-08T05:00:00Z"), range.From);
    })),
    ("Session extraction accepts only the exact HTTPS origin", () => Sync(() =>
    {
        Equal(true, KrillApiClient.IsTrustedPage(new("https://www.krill-code.com/app")));
        foreach (var uri in new[] { "http://www.krill-code.com/app", "https://www.krill-code.com.evil.example/app", "https://www.krill-code.com:444/app", "https://user@www.krill-code.com/app", "https://github.com/app" })
            Equal(false, KrillApiClient.IsTrustedPage(new(uri)));
    })),
    ("Credentials reject header injection and malformed values", () => Sync(() =>
    {
        Equal(true, KrillApiClient.IsPlausibleSession("synthetic.header.signature"));
        foreach (var token in new[] { "a", "abc.def", "synthetic.header.signature\r\nX-Injected: yes", "synthetic.header.signature extra" })
            Equal(false, KrillApiClient.IsPlausibleSession(token));
    })),
    ("Stats requests preserve method, range, Bearer and same-site cookie", async () =>
    {
        using var http = new HttpClient(new Handler(async request =>
        {
            Equal("https://www.krill-code.com/api/request-logs/stats", request.RequestUri!.AbsoluteUri);
            Equal(HttpMethod.Post, request.Method);
            Equal("Bearer", request.Headers.Authorization!.Scheme);
            Equal("synthetic.header.signature", request.Headers.Authorization.Parameter);
            Equal("krill_jwt=synthetic.header.signature", request.Headers.GetValues("Cookie").Single());
            using var json = JsonDocument.Parse(await request.Content!.ReadAsStringAsync());
            Equal("2026-10-14T16:00:00.0000000+00:00", json.RootElement.GetProperty("start_time").GetString());
            return Json("{\"success\":true,\"code\":0,\"data\":{\"total_requests\":5,\"total_cost_usd\":\"0.0007\"}}");
        }));
        var result = await new KrillApiClient(http).GetStatsAsync("synthetic.header.signature", DateTimeOffset.Parse("2026-10-15T00:00:00+08:00"), DateTimeOffset.Parse("2026-10-15T01:00:00+08:00"));
        Equal(5L, result.TotalRequests); Equal(.0007m, result.TotalCostUsd);
    }),
    ("Public announcements do not transmit an account session", async () =>
    {
        using var http = new HttpClient(new Handler(request =>
        {
            Equal(null, request.Headers.Authorization); Equal(false, request.Headers.Contains("Cookie"));
            return Task.FromResult(Json("{\"success\":true,\"data\":{\"items\":[]}}"));
        }));
        Equal(0, (await new KrillApiClient(http).GetAnnouncementsAsync()).Items.Count);
    }),
    ("HTTP 401 and envelope 401 require login", async () =>
    {
        using var first = new HttpClient(new Handler(_ => Task.FromResult(new HttpResponseMessage(HttpStatusCode.Unauthorized))));
        await Fails(ApiFailure.SessionExpired, () => new KrillApiClient(first).GetProfileAsync("synthetic.header.signature"));
        using var second = new HttpClient(new Handler(_ => Task.FromResult(Json("{\"success\":false,\"code\":401}"))));
        await Fails(ApiFailure.SessionExpired, () => new KrillApiClient(second).GetProfileAsync("synthetic.header.signature"));
    }),
    ("429, forbidden, redirects and HTML remain distinct failures", async () =>
    {
        foreach (var (status, expected) in new[] { ((HttpStatusCode)429, ApiFailure.RateLimited), (HttpStatusCode.Forbidden, ApiFailure.Forbidden), (HttpStatusCode.Redirect, ApiFailure.Service) })
        {
            using var http = new HttpClient(new Handler(_ => Task.FromResult(new HttpResponseMessage(status))));
            await Fails(expected, () => new KrillApiClient(http).GetProfileAsync("synthetic.header.signature"));
        }
        using var invalid = new HttpClient(new Handler(_ => Task.FromResult(Json("<html>not JSON</html>"))));
        await Fails(ApiFailure.InvalidResponse, () => new KrillApiClient(invalid).GetProfileAsync("synthetic.header.signature"));
    }),
    ("Failed business envelopes do not yield an empty financial result", async () =>
    {
        using var http = new HttpClient(new Handler(_ => Task.FromResult(Json("{\"success\":false,\"code\":500,\"data\":{}}"))));
        await Fails(ApiFailure.InvalidResponse, () => new KrillApiClient(http).GetSubscriptionsAsync("synthetic.header.signature"));
    }),
    ("Announcements are plain text and bounded", () => Sync(() =>
    {
        Equal("Hello & world", DisplayFormat.PlainText("<script>ignored()</script><b>Hello &amp; world</b>"));
        Equal("abc…", DisplayFormat.PlainText("abcdef", 3));
    }))
};

var failures = 0;
if (args.Contains("--har"))
{
    try
    {
        var index = Array.IndexOf(args, "--har");
        using var archive = JsonDocument.Parse(await File.ReadAllTextAsync(args[index + 1]));
        var verified = 0;
        foreach (var entry in archive.RootElement.GetProperty("log").GetProperty("entries").EnumerateArray())
        {
            var path = new Uri(entry.GetProperty("request").GetProperty("url").GetString()!).AbsolutePath;
            if (path is not ("/api/subscription" or "/api/request-logs/stats" or "/api/public/announcements")) continue;
            var content = entry.GetProperty("response").GetProperty("content");
            var body = content.GetProperty("text").GetString()!;
            if (content.TryGetProperty("encoding", out var encoding) && encoding.GetString() == "base64")
                body = Encoding.UTF8.GetString(Convert.FromBase64String(body));
            switch (path)
            {
                case "/api/subscription":
                    var subscription = JsonSerializer.Deserialize<ApiEnvelope<SubscriptionResponse>>(body, KrillJson.Options)!;
                    Equal(true, subscription.Success);
                    foreach (var item in subscription.Data!.Subscriptions) _ = QuotaProjection.From(item).Percent;
                    break;
                case "/api/request-logs/stats":
                    Equal(true, JsonSerializer.Deserialize<ApiEnvelope<RequestStats>>(body, KrillJson.Options)!.Success); break;
                case "/api/public/announcements":
                    Equal(true, JsonSerializer.Deserialize<ApiEnvelope<AnnouncementsResponse>>(body, KrillJson.Options)!.Success); break;
            }
            verified++;
        }
        Equal(true, verified > 0);
        Console.WriteLine($"PASS local HAR response compatibility ({verified} responses; no credentials read or transmitted)");
    }
    catch (Exception error) { failures++; Console.WriteLine("FAIL local HAR response compatibility (" + error.GetType().Name + ")"); }
}
if (args.Contains("--live-public"))
{
    try
    {
        using var http = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false, UseCookies = false }) { Timeout = TimeSpan.FromSeconds(20) };
        _ = await new KrillApiClient(http).GetAnnouncementsAsync();
        Console.WriteLine("PASS live public announcements (anonymous; no account session)");
    }
    catch (Exception error) { failures++; Console.WriteLine("FAIL live public announcements (" + error.GetType().Name + ")"); }
}
foreach (var test in tests)
{
    try { await test.Run(); Console.WriteLine("PASS " + test.Name); }
    catch (Exception error) { failures++; Console.WriteLine("FAIL " + test.Name + " (" + error.GetType().Name + ")"); }
}
Console.WriteLine($"{tests.Length - failures}/{tests.Length} passed");
return failures == 0 ? 0 : 1;

static Task Sync(Action action) { action(); return Task.CompletedTask; }
static void Equal<T>(T expected, T actual)
{ if (!EqualityComparer<T>.Default.Equals(expected, actual)) throw new InvalidOperationException("Assertion failed."); }
static HttpResponseMessage Json(string value) => new(HttpStatusCode.OK) { Content = new StringContent(value, Encoding.UTF8, "application/json") };
static async Task Fails(ApiFailure expected, Func<Task> operation)
{
    try { await operation(); } catch (KrillApiException error) { Equal(expected, error.Failure); return; }
    throw new InvalidOperationException("Expected API failure.");
}
sealed class Handler(Func<HttpRequestMessage, Task<HttpResponseMessage>> send) : HttpMessageHandler
{
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) => send(request);
}
