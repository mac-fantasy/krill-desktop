use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::Value;
use std::str::FromStr;

// Decode JSON numbers through their exact text; never pass account amounts through f64.
fn decimal<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    let v = Value::deserialize(d)?;
    let text = match v {
        Value::Null => return Ok(None),
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => return Err(D::Error::custom("Invalid decimal type")),
    };
    static NUMBER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\A-?[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?\z").unwrap()
    });
    if !NUMBER.is_match(&text) {
        return Err(D::Error::custom("Invalid decimal"));
    }
    Decimal::from_str(&text)
        .or_else(|_| Decimal::from_scientific(&text))
        .map(Some)
        .map_err(|_| D::Error::custom("Decimal outside supported range"))
}
fn integer<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    match Value::deserialize(d)? {
        Value::Null => Ok(None),
        Value::String(s) => s
            .parse()
            .map(Some)
            .map_err(|_| D::Error::custom("Invalid integer")),
        Value::Number(n) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| D::Error::custom("Invalid integer")),
        _ => Err(D::Error::custom("Invalid integer type")),
    }
}
fn required_integer<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(integer(d)?.unwrap_or(0))
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Envelope<T> {
    pub success: bool,
    pub code: Option<i64>,
    pub data: Option<T>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Profile {
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub display_name: Option<String>,
}
impl Profile {
    pub fn friendly_name(&self) -> String {
        [
            &self.display_name,
            &self.nickname,
            &self.name,
            &self.username,
        ]
        .into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .cloned()
        .or_else(|| {
            self.email.as_ref().and_then(|s| {
                s.split('@')
                    .next()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            })
        })
        .unwrap_or_else(|| "Krill 用户".into())
    }
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Subscriptions {
    pub subscriptions: Vec<Subscription>,
    pub request_count_quota: Option<RequestCountQuota>,
    #[serde(deserialize_with = "decimal")]
    pub credit_balance_usd: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub welfare_balance_usd: Option<Decimal>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Subscription {
    #[serde(deserialize_with = "required_integer")]
    pub subscription_id: i64,
    pub status: Option<String>,
    pub plan: Plan,
    pub quota: Quota,
    pub subscription_start_at: Option<DateTime<Utc>>,
    pub subscription_end_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "required_integer")]
    pub consume_priority: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Plan {
    pub name: String,
    pub billing_type: Option<String>,
    #[serde(deserialize_with = "integer")]
    pub duration_days: Option<i64>,
    #[serde(deserialize_with = "decimal")]
    pub total_credits: Option<Decimal>,
    #[serde(deserialize_with = "integer")]
    pub rate_limit_5h: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub rate_limit_weekly: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub rate_limit_monthly: Option<i64>,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            name: "Krill 套餐".into(),
            billing_type: None,
            duration_days: None,
            total_credits: None,
            rate_limit_5h: None,
            rate_limit_weekly: None,
            rate_limit_monthly: None,
        }
    }
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Quota {
    #[serde(deserialize_with = "decimal")]
    pub daily_limit_usd: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub used_usd: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub remaining_usd: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub forwarded_remaining_usd: Option<Decimal>,
    pub window_reset_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "decimal")]
    pub limit_credits: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub used_credits: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub remaining_credits: Option<Decimal>,
    #[serde(deserialize_with = "integer")]
    pub limit_requests: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub used_requests: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub remaining_requests: Option<i64>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Stats {
    #[serde(deserialize_with = "required_integer")]
    pub total_requests: i64,
    #[serde(deserialize_with = "required_integer")]
    pub success_requests: i64,
    #[serde(deserialize_with = "integer")]
    pub failed_requests: Option<i64>,
    #[serde(deserialize_with = "required_integer")]
    pub total_tokens: i64,
    #[serde(deserialize_with = "integer")]
    pub input_tokens: Option<i64>,
    #[serde(deserialize_with = "required_integer")]
    pub output_tokens: i64,
    #[serde(deserialize_with = "integer")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub cache_rate_read_tokens: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub cache_rate_eligible_tokens: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub reasoning_tokens: Option<i64>,
    #[serde(deserialize_with = "decimal")]
    pub rpm: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub tpm: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub total_cost_usd: Option<Decimal>,
    pub channel_cache_rates: Vec<ChannelCacheRate>,
    pub trend: Vec<TrendBucket>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ChannelCacheRate {
    pub channel_name: String,
    #[serde(deserialize_with = "decimal")]
    pub cache_rate: Option<Decimal>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrendBucket {
    pub bucket_start: DateTime<Utc>,
    #[serde(default, deserialize_with = "required_integer")]
    pub request_count: i64,
    #[serde(default, deserialize_with = "decimal")]
    pub total_cost_usd: Option<Decimal>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct RequestCountQuota {
    #[serde(deserialize_with = "integer")]
    pub used_5h: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub limit_5h: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub used_weekly: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub limit_weekly: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub used_monthly: Option<i64>,
    #[serde(deserialize_with = "integer")]
    pub limit_monthly: Option<i64>,
    pub cycle_start: Option<String>,
    pub cycle_end: Option<String>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ResetCards {
    pub items: Vec<ResetCard>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ResetCard {
    #[serde(deserialize_with = "required_integer")]
    pub id: i64,
    pub plan_name: Option<String>,
    #[serde(deserialize_with = "integer")]
    pub status: Option<i64>,
    pub expired: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
    pub used_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "decimal")]
    pub cap_usd: Option<Decimal>,
    #[serde(deserialize_with = "decimal")]
    pub granted_usd: Option<Decimal>,
    pub note: Option<String>,
    pub usable_subscriptions: Option<Vec<ResetCardSubscription>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResetCardSubscription {
    #[serde(deserialize_with = "required_integer")]
    pub subscription_id: i64,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Announcements {
    pub items: Vec<Announcement>,
}
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Announcement {
    #[serde(deserialize_with = "required_integer")]
    pub id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
