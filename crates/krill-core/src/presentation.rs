use crate::model::*;
use chrono::{DateTime, Local, TimeZone, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use std::sync::LazyLock;

pub fn number(value: Decimal, digits: usize) -> String {
    let rounded = value.round_dp_with_strategy(
        digits as u32,
        rust_decimal::RoundingStrategy::MidpointAwayFromZero,
    );
    let text = format!("{rounded:.digits$}");
    let (whole, fractional) = text
        .split_once('.')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((&text, None));
    let (sign, whole) = whole
        .strip_prefix('-')
        .map(|s| ("-", s))
        .unwrap_or(("", whole));
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!(
        "{sign}{grouped}{}",
        fractional.map(|s| format!(".{s}")).unwrap_or_default()
    )
}
pub fn money(v: Option<Decimal>) -> String {
    v.map(|v| format!("${}", number(v, 2)))
        .unwrap_or("—".into())
}
pub fn cost(v: Option<Decimal>) -> String {
    v.map(|v| {
        format!(
            "${}",
            number(
                v,
                if v > Decimal::ZERO && v < Decimal::ONE {
                    4
                } else {
                    2
                }
            )
        )
    })
    .unwrap_or("—".into())
}
pub fn count(v: Option<i64>) -> String {
    v.filter(|v| *v >= 0)
        .map(|v| number(v.into(), 0))
        .unwrap_or("—".into())
}
pub fn compact(v: i64) -> String {
    if v >= 1_000_000 {
        format!(
            "{}M",
            number(Decimal::from(v) / Decimal::from(1_000_000), 2)
        )
    } else if v >= 10_000 {
        format!("{}k", number(Decimal::from(v) / Decimal::from(1_000), 1))
    } else {
        count(Some(v))
    }
}
pub fn percentage(v: Option<Decimal>) -> String {
    v.filter(|v| *v >= Decimal::ZERO && *v <= Decimal::ONE_HUNDRED)
        .map(|v| format!("{}%", number(v, 1)))
        .unwrap_or("—".into())
}
pub fn rate(v: Option<Decimal>) -> String {
    v.filter(|v| *v >= Decimal::ZERO)
        .map(|v| number(v, 1))
        .unwrap_or("—".into())
}
pub fn date(v: Option<DateTime<Utc>>) -> String {
    v.map(|v| v.with_timezone(&Local).format("%m.%d %H:%M").to_string())
        .unwrap_or("—".into())
}
pub fn plain_text(v: Option<&str>, limit: usize) -> String {
    static BLOCK: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)>").unwrap()
    });
    static TAG: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"<[^>]*>").unwrap());
    let Some(v) = v.filter(|v| !v.trim().is_empty()) else {
        return "点击前往官网查看完整内容。".into();
    };
    let text = BLOCK.replace_all(v, " ");
    let mut text = TAG.replace_all(&text, " ").into_owned();
    for (a, b) in [
        ("&nbsp;", " "),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&amp;", "&"),
    ] {
        text = text.replace(a, b);
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.chars().take(limit).collect::<String>()
        + if text.chars().count() > limit {
            "…"
        } else {
            ""
        }
}
pub fn day_range<T: TimeZone>(now: DateTime<T>) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let midnight = now.date_naive().and_hms_opt(0, 0, 0)?;
    let start = now.timezone().from_local_datetime(&midnight).earliest()?;
    Some((start.with_timezone(&Utc), now.with_timezone(&Utc)))
}

#[derive(Clone, Debug)]
pub struct QuotaProjection {
    pub cycle: String,
    pub limit: Option<Decimal>,
    pub used: Option<Decimal>,
    pub remaining: Option<Decimal>,
    pub forwarded: Option<Decimal>,
    pub reset_at: Option<DateTime<Utc>>,
    pub is_money: bool,
    pub unit: &'static str,
}
impl QuotaProjection {
    pub fn new(s: &Subscription, shared: Option<&RequestCountQuota>) -> Self {
        let q = &s.quota;
        if s.plan.billing_type.as_deref() == Some("request_count") {
            if let Some(w) = shared.and_then(|q| {
                q.windows()
                    .into_iter()
                    .filter(|w| w.percent().is_some())
                    .min_by(|a, b| a.percent().partial_cmp(&b.percent()).unwrap())
            }) {
                return Self {
                    cycle: format!("按次额度 · {}", w.label),
                    limit: w.limit.map(Decimal::from),
                    used: w.used.map(Decimal::from),
                    remaining: w.remaining(),
                    forwarded: None,
                    reset_at: None,
                    is_money: false,
                    unit: "次",
                };
            }
            return Self {
                cycle: "按次额度".into(),
                limit: q.limit_requests.map(Decimal::from),
                used: q.used_requests.map(Decimal::from),
                remaining: q.remaining_requests.map(Decimal::from),
                forwarded: None,
                reset_at: q.window_reset_at,
                is_money: false,
                unit: "次",
            };
        }
        if s.plan.total_credits.is_some() || q.limit_credits.is_some() {
            return Self {
                cycle: "积分额度".into(),
                limit: q.limit_credits.or(s.plan.total_credits),
                used: q.used_credits,
                remaining: q.remaining_credits,
                forwarded: None,
                reset_at: q.window_reset_at,
                is_money: false,
                unit: "积分",
            };
        }
        Self {
            cycle: match s.plan.billing_type.as_deref() {
                Some("usd_daily") => "本日额度",
                Some("usd_weekly") => "本周额度",
                Some("usd_monthly") => "本月额度",
                _ => "当前周期额度",
            }
            .into(),
            limit: q.daily_limit_usd,
            used: q.used_usd,
            remaining: q.remaining_usd,
            forwarded: q.forwarded_remaining_usd,
            reset_at: q.window_reset_at,
            is_money: true,
            unit: "USD",
        }
    }
    pub fn percent(&self) -> Option<f64> {
        let limit = self.limit.filter(|v| *v > Decimal::ZERO)?;
        let remaining = self.remaining?;
        if remaining <= Decimal::ZERO {
            return Some(0.);
        }
        if remaining >= limit {
            return Some(100.);
        }
        remaining
            .checked_div(limit)?
            .checked_mul(Decimal::ONE_HUNDRED)?
            .to_f64()
    }
    pub fn amount(&self, v: Option<Decimal>) -> String {
        if self.is_money {
            money(v)
        } else {
            v.map(|v| format!("{} {}", number(v, 0), self.unit))
                .unwrap_or("—".into())
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Available,
    NotStarted,
    Expired,
    Exhausted,
    Frozen,
    Inactive,
}
impl Availability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "使用中",
            Self::NotStarted => "未生效",
            Self::Expired => "已到期",
            Self::Exhausted => "额度已用尽",
            Self::Frozen => "已冻结",
            Self::Inactive => "已结束 / 不可用",
        }
    }
}
impl Subscription {
    pub fn is_monthly(&self) -> bool {
        self.plan.billing_type.as_deref() == Some("usd_monthly")
            || self
                .plan
                .duration_days
                .is_some_and(|d| (28..=31).contains(&d))
    }
    pub fn availability(&self, now: DateTime<Utc>) -> Availability {
        if self.status.as_deref() == Some("expired")
            || self.subscription_end_at.is_some_and(|v| v <= now)
        {
            return Availability::Expired;
        }
        if self.status.as_deref() == Some("frozen") {
            return Availability::Frozen;
        }
        if self.status.as_deref() == Some("exhausted") {
            return Availability::Exhausted;
        }
        if self.subscription_start_at.is_some_and(|v| v > now) {
            return Availability::NotStarted;
        }
        if self.status.as_deref() != Some("active") {
            return Availability::Inactive;
        }
        let q = QuotaProjection::new(self, None);
        if self.is_monthly()
            && q.remaining.is_some_and(|v| v <= Decimal::ZERO)
            && !(q.is_money && q.forwarded.is_some_and(|v| v > Decimal::ZERO))
        {
            return Availability::Exhausted;
        }
        Availability::Available
    }
}
pub fn ordered(subscriptions: &[Subscription]) -> Vec<&Subscription> {
    let mut result: Vec<_> = subscriptions.iter().collect();
    result.sort_by_key(|s| (s.consume_priority, s.subscription_id));
    result
}
pub fn select(
    subscriptions: &[Subscription],
    preferred: Option<i64>,
    was_monthly: bool,
    now: DateTime<Utc>,
) -> Option<&Subscription> {
    let available: Vec<_> = ordered(subscriptions)
        .into_iter()
        .filter(|s| s.availability(now) == Availability::Available)
        .collect();
    if let Some(s) = available
        .iter()
        .find(|s| Some(s.subscription_id) == preferred)
    {
        return Some(s);
    }
    if was_monthly
        || subscriptions
            .iter()
            .any(|s| Some(s.subscription_id) == preferred && s.is_monthly())
    {
        if let Some(s) = available.iter().find(|s| s.is_monthly()) {
            return Some(s);
        }
    }
    available.first().copied()
}
pub fn next_transition(
    subscriptions: &[Subscription],
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    subscriptions
        .iter()
        .flat_map(|s| [s.subscription_start_at, s.subscription_end_at])
        .flatten()
        .filter(|d| *d > now)
        .min()
}
#[derive(Clone, Debug)]
pub struct RequestWindow {
    pub label: &'static str,
    pub used: Option<i64>,
    pub limit: Option<i64>,
}
impl RequestWindow {
    pub fn remaining(&self) -> Option<Decimal> {
        let used = self.used.filter(|v| *v >= 0)?;
        let limit = self.limit.filter(|v| *v > 0)?;
        Some((Decimal::from(limit) - Decimal::from(used)).max(Decimal::ZERO))
    }
    pub fn percent(&self) -> Option<f64> {
        (self.remaining()? / Decimal::from(self.limit?) * Decimal::ONE_HUNDRED).to_f64()
    }
    pub fn detail(&self) -> String {
        format!(
            "已用 {} / {} 次",
            count(self.used),
            count(self.limit.filter(|v| *v > 0))
        )
    }
}
impl RequestCountQuota {
    pub fn windows(&self) -> [RequestWindow; 3] {
        [
            RequestWindow {
                label: "5 小时",
                used: self.used_5h,
                limit: self.limit_5h,
            },
            RequestWindow {
                label: "本周",
                used: self.used_weekly,
                limit: self.limit_weekly,
            },
            RequestWindow {
                label: "本月",
                used: self.used_monthly,
                limit: self.limit_monthly,
            },
        ]
    }
    pub fn cycle(&self) -> String {
        format!(
            "周期 {} ~ {}",
            plain_text(Some(self.cycle_start.as_deref().unwrap_or("—")), 40),
            plain_text(Some(self.cycle_end.as_deref().unwrap_or("—")), 40)
        )
    }
}
impl Stats {
    pub fn cache_percent(&self) -> Option<Decimal> {
        let (read, eligible) = if let (Some(r), Some(e)) =
            (self.cache_rate_read_tokens, self.cache_rate_eligible_tokens)
        {
            (Decimal::from(r), Decimal::from(e))
        } else {
            let (r, w, i) = (
                self.cache_read_input_tokens?,
                self.cache_creation_input_tokens?,
                self.input_tokens?,
            );
            if r < 0 || w < 0 || i < 0 {
                return None;
            }
            (
                Decimal::from(r),
                Decimal::from(r) + Decimal::from(w) + Decimal::from(i),
            )
        };
        if read < Decimal::ZERO || eligible <= Decimal::ZERO || read > eligible {
            return None;
        }
        Some(read / eligible * Decimal::ONE_HUNDRED)
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResetCardState {
    Unused,
    Used,
    Revoked,
    Expired,
    Unknown,
}
impl ResetCardState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unused => "未使用",
            Self::Used => "已使用",
            Self::Revoked => "已作废",
            Self::Expired => "已过期",
            Self::Unknown => "状态未知",
        }
    }
}
impl ResetCard {
    pub fn state(&self, now: DateTime<Utc>) -> ResetCardState {
        match self.status {
            Some(1) => return ResetCardState::Used,
            Some(2) => return ResetCardState::Revoked,
            _ => {}
        }
        if self.expired == Some(true) || self.expires_at.is_some_and(|d| d <= now) {
            ResetCardState::Expired
        } else if self.status == Some(0) {
            ResetCardState::Unused
        } else {
            ResetCardState::Unknown
        }
    }
    pub fn can_use(&self, now: DateTime<Utc>) -> bool {
        self.state(now) == ResetCardState::Unused
            && self
                .usable_subscriptions
                .as_ref()
                .is_some_and(|v| !v.is_empty())
    }
    pub fn availability_text(&self, now: DateTime<Utc>) -> String {
        if self.state(now) != ResetCardState::Unused {
            return self.state(now).label().into();
        }
        match &self.usable_subscriptions {
            None => "未使用 · 适用订阅未知".into(),
            Some(v) if v.is_empty() => "未使用 · 暂无适用订阅".into(),
            Some(v) => format!("可用 · {} 个订阅", v.len()),
        }
    }
}
#[derive(Clone, Debug)]
pub struct TrendChart {
    pub buckets: Vec<TrendBucket>,
    maximum: Decimal,
}
impl TrendChart {
    pub fn new(source: &[TrendBucket]) -> Self {
        let mut buckets = source.to_vec();
        buckets.sort_by_key(|b| b.bucket_start);
        if buckets.len() > 48 {
            buckets.drain(..buckets.len() - 48);
        }
        let maximum = buckets
            .iter()
            .filter_map(|b| b.total_cost_usd)
            .max()
            .unwrap_or_default();
        Self { buckets, maximum }
    }
    pub fn index(&self, x: f32, width: f32) -> Option<usize> {
        if self.buckets.is_empty()
            || !x.is_finite()
            || !width.is_finite()
            || width <= 0.
            || x < 0.
            || x > width
        {
            return None;
        }
        Some(((x / width * self.buckets.len() as f32) as usize).min(self.buckets.len() - 1))
    }
    pub fn height(&self, i: usize) -> f32 {
        self.buckets[i]
            .total_cost_usd
            .filter(|v| *v > Decimal::ZERO && self.maximum > Decimal::ZERO)
            .and_then(|v| (v / self.maximum).to_f32())
            .map(|v| (v * 88.).clamp(3., 88.))
            .unwrap_or(3.)
    }
    pub fn tooltip(&self, i: usize) -> String {
        let b = &self.buckets[i];
        format!(
            "{} · 费用 {} · {} 次请求",
            date(Some(b.bucket_start)),
            cost(b.total_cost_usd),
            b.request_count
        )
    }
    pub fn axis_indices(&self) -> Vec<usize> {
        let n = self.buckets.len().min(5);
        if n < 2 {
            return (0..n).collect();
        }
        (0..n)
            .map(|i| (i as f64 * (self.buckets.len() - 1) as f64 / (n - 1) as f64).round() as usize)
            .collect()
    }
}
