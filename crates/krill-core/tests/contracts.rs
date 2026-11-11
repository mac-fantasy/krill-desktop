use chrono::{DateTime, Duration, TimeZone, Utc};
use krill_core::{
    api::{Api, ApiError, Endpoint, decode_response},
    auth::*,
    release::*,
    state::*,
    *,
};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::str::FromStr;

fn fixture<T: DeserializeOwned>(name: &str) -> T {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/fixtures")
        .join(name);
    decode_response(200, &std::fs::read(root).unwrap()).unwrap()
}
fn now() -> DateTime<Utc> {
    "2099-10-20T00:00:00Z".parse().unwrap()
}
fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}
fn session() -> Session {
    Session::parse("syntheticxx.payloadxx.signaturexx").unwrap()
}

#[test]
fn money_and_current_cycle_preserve_exact_decimal_strings() {
    let s: Subscriptions = fixture("subscriptions.json");
    assert_eq!(s.credit_balance_usd, Some(dec("24.80")));
    assert_eq!(s.welfare_balance_usd, Some(dec("3.50")));
    let q = QuotaProjection::new(&s.subscriptions[0], None);
    assert_eq!(q.remaining, Some(dec("72.6000")));
    assert_eq!(q.percent(), Some(72.6));
    assert_eq!(q.forwarded, Some(dec("8.20")));
    assert_eq!(q.cycle, "本周额度");
    assert_eq!(q.amount(q.remaining), "$72.60");
    assert_eq!(
        QuotaProjection::new(&s.subscriptions[1], None).amount(Some(750.into())),
        "750 次"
    );
    assert_eq!(
        QuotaProjection::new(&s.subscriptions[2], None).limit,
        Some(2000.into())
    );
    assert_eq!(
        QuotaProjection::new(&s.subscriptions[3], None).percent(),
        None
    );
}
#[test]
fn invalid_amount_types_do_not_turn_into_zero_or_round_through_float() {
    for v in [
        json!("NaN"),
        json!("inf"),
        json!(" 1.2"),
        json!("$1"),
        json!({}),
        json!(true),
    ] {
        assert!(serde_json::from_value::<Quota>(json!({"remaining_usd":v})).is_err());
    }
    let q: Quota =
        serde_json::from_str(r#"{"remaining_usd":0.1234567890123456789012345678}"#).unwrap();
    assert_eq!(q.remaining_usd, Some(dec("0.1234567890123456789012345678")));
    assert_eq!(cost(Some(dec("0.0007"))), "$0.0007");
    assert_eq!(money(None), "—");
    assert_eq!(money(Some(Decimal::ZERO)), "$0.00");
}
#[test]
fn monthly_selection_matches_shared_cases_and_expiry_boundary() {
    let s: Subscriptions = fixture("subscription-selection.json");
    for (preferred, expected) in [
        (201, 205),
        (203, 205),
        (100, 100),
        (206, 206),
        (208, 208),
        (209, 209),
    ] {
        assert_eq!(
            select(&s.subscriptions, Some(preferred), false, now())
                .unwrap()
                .subscription_id,
            expected
        );
    }
    assert_eq!(
        select(&s.subscriptions, Some(999), true, now())
            .unwrap()
            .subscription_id,
        205
    );
    assert_eq!(
        select(&s.subscriptions, None, false, now())
            .unwrap()
            .subscription_id,
        100
    );
    assert_eq!(
        select(
            &s.subscriptions,
            Some(205),
            true,
            now() + Duration::seconds(10)
        )
        .unwrap()
        .subscription_id,
        202
    );
    assert_eq!(
        next_transition(&s.subscriptions, now()),
        Some(now() + Duration::seconds(10))
    );
    for (id, state) in [
        (201, Availability::Exhausted),
        (202, Availability::NotStarted),
        (203, Availability::Expired),
        (204, Availability::Frozen),
        (207, Availability::Exhausted),
        (208, Availability::Available),
        (209, Availability::Available),
        (210, Availability::Exhausted),
    ] {
        assert_eq!(
            s.subscriptions
                .iter()
                .find(|s| s.subscription_id == id)
                .unwrap()
                .availability(now()),
            state
        );
    }
}
#[test]
fn request_cards_share_the_most_constrained_complete_window() {
    let s: Subscriptions = fixture("request-count.json");
    let shared = s.request_count_quota.as_ref().unwrap();
    let windows = shared.windows();
    assert_eq!(windows[0].percent(), Some(5.));
    assert_eq!(windows[1].percent(), Some(70.));
    let q = QuotaProjection::new(&s.subscriptions[0], Some(shared));
    assert_eq!(q.cycle, "按次额度 · 5 小时");
    assert_eq!(q.remaining, Some(5.into()));
    assert_eq!(
        QuotaProjection::new(&s.subscriptions[0], None).remaining,
        None
    );
    let mut shared = shared.clone();
    shared.used_5h = None;
    assert_eq!(
        QuotaProjection::new(&s.subscriptions[0], Some(&shared)).percent(),
        Some(70.)
    );
    shared.used_5h = Some(-1);
    assert!(shared.windows()[0].percent().is_none());
}
#[test]
fn reset_card_state_preserves_history_and_expires_at_exact_time() {
    let cards: ResetCards = fixture("reset-cards.json");
    let expected = [
        ResetCardState::Unused,
        ResetCardState::Used,
        ResetCardState::Revoked,
        ResetCardState::Expired,
        ResetCardState::Unused,
        ResetCardState::Unknown,
        ResetCardState::Expired,
    ];
    for (card, state) in cards.items.iter().zip(expected) {
        assert_eq!(card.state(now()), state);
    }
    assert_eq!(cards.items.iter().filter(|c| c.can_use(now())).count(), 1);
    assert!(!cards.items[0].can_use(now() + Duration::days(1)));
    assert_eq!(cards.items[0].availability_text(now()), "可用 · 1 个订阅");
}
#[test]
fn cache_rate_uses_dedicated_counters_and_requires_complete_fallback() {
    let s: Stats = fixture("stats.json");
    assert_eq!(s.cache_percent(), Some(64.into()));
    assert_eq!(s.reasoning_tokens, Some(48500));
    let mut s = Stats {
        input_tokens: Some(50),
        cache_read_input_tokens: Some(60),
        cache_creation_input_tokens: Some(10),
        ..Default::default()
    };
    assert_eq!(s.cache_percent(), Some(50.into()));
    s.cache_rate_read_tokens = Some(10);
    s.cache_rate_eligible_tokens = Some(100);
    assert_eq!(s.cache_percent(), Some(10.into()));
    s.cache_rate_read_tokens = Some(101);
    assert_eq!(s.cache_percent(), None);
    assert_eq!(
        Stats {
            input_tokens: Some(50),
            cache_read_input_tokens: Some(60),
            ..Default::default()
        }
        .cache_percent(),
        None
    );
    assert_eq!(
        Stats {
            cache_rate_read_tokens: Some(0),
            cache_rate_eligible_tokens: Some(100),
            ..Default::default()
        }
        .cache_percent(),
        Some(Decimal::ZERO)
    );
}
#[test]
fn trend_sorting_whole_slots_and_small_cost_readout() {
    let buckets: Vec<_> = (0..60)
        .rev()
        .map(|i| TrendBucket {
            bucket_start: now() + Duration::hours(i),
            request_count: 12345,
            total_cost_usd: Some(dec("0.0007")),
        })
        .collect();
    let chart = TrendChart::new(&buckets);
    assert_eq!(chart.buckets.len(), 48);
    assert_eq!(chart.buckets[0].bucket_start, now() + Duration::hours(12));
    assert_eq!(chart.axis_indices(), [0, 12, 24, 35, 47]);
    assert_eq!(chart.index(0., 480.), Some(0));
    assert_eq!(chart.index(480., 480.), Some(47));
    for i in 0..48 {
        assert_eq!(chart.index(i as f32 * 10. + 1., 480.), Some(i));
    }
    for x in [-1., 481., f32::NAN, f32::INFINITY] {
        assert_eq!(chart.index(x, 480.), None);
    }
    assert!(chart.tooltip(0).contains("费用 $0.0007 · 12345 次请求"));
    let chart = TrendChart::new(&[TrendBucket {
        bucket_start: now(),
        request_count: 0,
        total_cost_usd: None,
    }]);
    assert_eq!(chart.height(0), 3.);
    assert!(chart.tooltip(0).contains("费用 —"));
    assert_eq!(TrendChart::new(&[]).index(0., 480.), None);
}
#[test]
fn local_day_respects_dst_midnight_offset() {
    let zone = chrono_tz::America::Los_Angeles;
    let now = zone.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap();
    let (start, end) = day_range(now).unwrap();
    assert_eq!(
        start,
        "2026-03-08T08:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!((end - start).num_hours(), 11);
}
#[test]
fn session_import_never_accepts_urls_scripts_or_header_injection() {
    let valid = "syntheticxx.payloadxx.signaturexx";
    assert_eq!(
        Session::parse(&format!(" \nBearer {valid}\r\n"))
            .unwrap()
            .expose(),
        valid
    );
    for v in [
        String::new(),
        "null".into(),
        format!("https://www.krill-code.com/#{valid}"),
        format!("{valid}\r\nX-Test: injected"),
        EXPORT_SCRIPT.into(),
        "a".repeat(16401),
    ] {
        assert!(Session::parse(&v).is_none());
    }
    assert!(!format!("{:?}", session()).contains(valid));
}
#[test]
fn delegated_login_allows_https_but_only_krill_can_supply_session() {
    for v in [
        "https://github.com/login",
        "https://appleid.apple.com/auth/authorize",
        "https://sso.example.org/login",
    ] {
        assert!(allows_navigation(v));
        assert!(!trusted_origin(v));
    }
    for v in [
        "http://www.krill-code.com",
        "https://user:password@www.krill-code.com",
        "https://www.krill-code.com:444",
        "about:blank",
        "file:///tmp/login",
    ] {
        assert!(!allows_navigation(v));
    }
    assert!(!trusted_origin("https://www.krill-code.com.evil.example"));
    assert!(extracted_session(LOGIN_URL, ORIGIN, session().expose()).is_some());
    assert!(extracted_session(LOGIN_URL, "https://github.com", session().expose()).is_none());
}
#[test]
fn requests_pin_destination_method_auth_and_public_anonymity() {
    let api = Api::new().unwrap();
    let token = session();
    let request = api
        .request(
            Endpoint::Stats,
            Some(&token),
            Some((now(), now() + Duration::seconds(5))),
        )
        .unwrap();
    assert_eq!(request.method(), reqwest::Method::POST);
    assert_eq!(
        request.url().as_str(),
        "https://www.krill-code.com/api/request-logs/stats"
    );
    assert_eq!(
        request.headers()["Authorization"],
        format!("Bearer {}", token.expose())
    );
    assert_eq!(
        request.headers()["Cookie"],
        format!("krill_jwt={}", token.expose())
    );
    let body: serde_json::Value =
        serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
    assert_eq!(body["start_time"], "2099-10-20T00:00:00.000Z");
    let public = api
        .request(Endpoint::Announcements, Some(&token), None)
        .unwrap();
    assert!(!public.headers().contains_key("Authorization"));
    assert!(!public.headers().contains_key("Cookie"));
    assert!(api.request(Endpoint::Profile, None, None).is_err());
}
#[test]
fn http_business_errors_oversize_and_html_do_not_create_empty_account_data() {
    for (status, error) in [
        (401, ApiError::SessionExpired),
        (403, ApiError::Forbidden),
        (429, ApiError::RateLimited),
        (302, ApiError::Service),
    ] {
        assert!(matches!(decode_response::<Subscriptions>(status,b"{}"),Err(e) if e==error));
    }
    assert!(matches!(
        decode_response::<Subscriptions>(200, br#"{"success":false,"code":401,"data":[]}"#),
        Err(ApiError::SessionExpired)
    ));
    for bytes in [
        b"<html></html>".as_slice(),
        br#"{"success":false,"data":{}}"#,
        br#"{"success":true,"data":null}"#,
    ] {
        assert!(decode_response::<Subscriptions>(200, bytes).is_err());
    }
    assert!(
        decode_response::<Subscriptions>(200, &vec![b' '; krill_core::api::MAX_RESPONSE + 1])
            .is_err()
    );
}
#[test]
fn announcements_are_bounded_native_text() {
    assert_eq!(
        plain_text(
            Some("<script>secret()</script><b> Hi &amp; Krill </b>"),
            100
        ),
        "Hi & Krill"
    );
    assert_eq!(plain_text(Some("你好世界"), 2), "你好…");
}
#[test]
fn release_versions_are_strict_and_assets_are_bound_to_repository_digest_and_platform() {
    assert!("1.10.2".parse::<Version>().unwrap() > "1.9.99".parse::<Version>().unwrap());
    for v in [
        "1.2",
        "1.2.3-beta",
        "01.2.3",
        "1.2.65536",
        "1.2.3\n",
        "1.2.٣",
    ] {
        assert!(v.parse::<Version>().is_err());
    }
    let mut release: Release =
        serde_json::from_str(include_str!("../../../contracts/updates/latest.json")).unwrap();
    let current = "1.9.0".parse().unwrap();
    assert_eq!(release.update(current, "win-x64").unwrap().size, 1024);
    assert_eq!(release.update(current, "macos-arm64").unwrap().size, 2048);
    assert!(release.update(current, "macos-universal").is_none());
    assert!(release.update(current, "win-arm64").is_none());
    assert!(
        release
            .update("1.10.2".parse().unwrap(), "win-x64")
            .is_none()
    );
    release.assets[0].browser_download_url = "https://evil.example/update.zip".into();
    assert!(release.update(current, "win-x64").is_none());
    release.draft = true;
    assert!(release.update(current, "macos-universal").is_none());
}
#[test]
fn old_windows_and_macos_preferences_load_without_losing_selection() {
    for text in [
        r#"{"SelectedSubscriptionId":205,"RefreshSeconds":120,"PanelPinned":true,"OrbLeftPixels":42}"#,
        r#"{"selectedSubscriptionID":205,"refreshSeconds":120,"panelPinned":true,"orbLeftPixels":42}"#,
    ] {
        let p: Preferences = serde_json::from_str(text).unwrap();
        assert_eq!(p.selected_subscription_id, Some(205));
        assert_eq!(p.refresh_seconds, 120);
        assert!(p.panel_pinned);
        assert_eq!(p.orb_left_pixels, Some(42));
    }
}
#[test]
fn state_rollover_tracks_removed_month_and_local_expiry_without_network() {
    let mut state = DashboardState::new(
        Preferences {
            selected_subscription_id: Some(205),
            ..Default::default()
        },
        None,
        false,
    );
    state.apply_subscriptions(fixture("subscription-selection.json"), now());
    assert_eq!(state.selected().unwrap().subscription_id, 205);
    assert!(state.advance_time(now() + Duration::seconds(10)));
    assert_eq!(state.selected().unwrap().subscription_id, 202);
    let mut subscriptions: Subscriptions = fixture("subscription-selection.json");
    subscriptions
        .subscriptions
        .retain(|s| s.subscription_id != 202);
    state.apply_subscriptions(subscriptions, now() + Duration::seconds(10));
    assert_eq!(state.selected().unwrap().subscription_id, 206);
}
#[test]
fn stale_generations_cannot_restore_data_after_logout_or_session_expiry() {
    let mut state = DashboardState::new(Preferences::default(), Some(session()), false);
    let (generation, _) = state.begin_refresh(now()).unwrap();
    state.clear_account();
    state.apply_event(
        generation,
        RefreshEvent::Core(Profile::default(), fixture("subscriptions.json")),
        now(),
    );
    assert!(state.subscriptions.is_none());
    state.accept_session(session(), Profile::default());
    let (generation, _) = state.begin_refresh(now()).unwrap();
    assert!(state.apply_event(
        generation,
        RefreshEvent::CoreError(ApiError::SessionExpired),
        now()
    ));
    state.apply_event(
        generation,
        RefreshEvent::Stats(fixture("stats.json")),
        now(),
    );
    assert!(state.stats.is_none());
    assert!(state.needs_login);
}
#[test]
fn partial_errors_keep_successful_data_and_backoff_and_demo_are_isolated() {
    let mut state = DashboardState::new(Preferences::default(), Some(session()), false);
    let g = state.generation;
    state.apply_event(
        g,
        RefreshEvent::Core(Profile::default(), fixture("subscriptions.json")),
        now(),
    );
    state.apply_event(g, RefreshEvent::Stats(fixture("stats.json")), now());
    state.apply_event(g, RefreshEvent::Cards(fixture("reset-cards.json")), now());
    state.apply_event(g, RefreshEvent::StatsError(ApiError::Network), now());
    state.apply_event(g, RefreshEvent::CardsError(ApiError::Service), now());
    assert!(state.stats.is_some());
    assert!(state.reset_cards.is_some());
    state.apply_event(g, RefreshEvent::CoreError(ApiError::Network), now());
    assert!(state.subscriptions.is_some());
    assert!(state.is_stale);
    assert_eq!(state.refresh_interval(), 120);
    state.panel_visible = true;
    assert_eq!(state.refresh_interval(), 60);
    state.failures = 100;
    assert_eq!(state.refresh_interval(), 240);
    state.clear_account();
    assert!(state.reset_cards.is_none());
    assert!(state.stats.is_none());
    state.use_demo();
    assert!(state.subscriptions.is_some());
    assert!(!state.refresh_due(now()));
    assert!(state.begin_refresh(now()).is_none());
}
