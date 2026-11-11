use crate::{
    auth::{ORIGIN, Session},
    model::*,
    presentation::day_range,
};
use chrono::{Local, SecondsFormat, Utc};
use reqwest::{
    Method,
    blocking::{Client, Request},
    redirect::Policy,
};
use serde::de::DeserializeOwned;
use std::{io::Read, time::Duration};

pub const MAX_RESPONSE: usize = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("登录已失效，请重新连接账号。")]
    SessionExpired,
    #[error("当前账号暂时无权读取此项数据。")]
    Forbidden,
    #[error("请求较频繁，稍后会自动重试。")]
    RateLimited,
    #[error("暂时无法连接，已保留上次成功读取的数据。")]
    Network,
    #[error("网站暂时不可用，稍后会自动重试。")]
    Service,
    #[error("网站返回的数据格式有变化，请稍后重试。")]
    InvalidResponse,
}
#[derive(Clone, Copy, Debug)]
pub enum Endpoint {
    Profile,
    Subscriptions,
    Stats,
    ResetCards,
    Announcements,
}
impl Endpoint {
    pub fn path(self) -> &'static str {
        match self {
            Self::Profile => "/api/auth/me",
            Self::Subscriptions => "/api/subscription",
            Self::Stats => "/api/request-logs/stats",
            Self::ResetCards => "/api/reset-cards",
            Self::Announcements => "/api/public/announcements",
        }
    }
}
#[derive(Clone)]
pub struct Api {
    client: Client,
}
impl Api {
    pub fn new() -> Result<Self, ApiError> {
        Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(20))
            .user_agent("KrillDesktop-Rust")
            .build()
            .map(|client| Self { client })
            .map_err(|_| ApiError::Network)
    }
    pub fn request(
        &self,
        endpoint: Endpoint,
        token: Option<&Session>,
        range: Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)>,
    ) -> Result<Request, ApiError> {
        let public = matches!(endpoint, Endpoint::Announcements);
        let mut request = self
            .client
            .request(
                if matches!(endpoint, Endpoint::Stats) {
                    Method::POST
                } else {
                    Method::GET
                },
                format!("{ORIGIN}{}", endpoint.path()),
            )
            .header("Accept", "application/json")
            .header("X-Language", "zh-CN")
            .header("Accept-Language", "zh-CN");
        if !public {
            let token = token.ok_or(ApiError::SessionExpired)?;
            let mut bearer =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
                    .map_err(|_| ApiError::SessionExpired)?;
            let mut cookie =
                reqwest::header::HeaderValue::from_str(&format!("krill_jwt={}", token.expose()))
                    .map_err(|_| ApiError::SessionExpired)?;
            bearer.set_sensitive(true);
            cookie.set_sensitive(true);
            request = request
                .header(reqwest::header::AUTHORIZATION, bearer)
                .header(reqwest::header::COOKIE, cookie);
        }
        if matches!(endpoint, Endpoint::Stats) {
            let (from, to) = range
                .or_else(|| day_range(Local::now()))
                .ok_or(ApiError::InvalidResponse)?;
            request=request.json(&serde_json::json!({"start_time":from.to_rfc3339_opts(SecondsFormat::Millis,true),"end_time":to.to_rfc3339_opts(SecondsFormat::Millis,true)}));
        }
        request.build().map_err(|_| ApiError::InvalidResponse)
    }
    fn send<T: DeserializeOwned>(
        &self,
        endpoint: Endpoint,
        token: Option<&Session>,
    ) -> Result<T, ApiError> {
        let response = self
            .client
            .execute(self.request(endpoint, token, None)?)
            .map_err(|_| ApiError::Network)?;
        let status = response.status().as_u16();
        status_result(status)?;
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE as u64)
        {
            return Err(ApiError::InvalidResponse);
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_RESPONSE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ApiError::Network)?;
        decode_response(status, &bytes)
    }
    pub fn profile(&self, token: &Session) -> Result<Profile, ApiError> {
        self.send(Endpoint::Profile, Some(token))
    }
    pub fn subscriptions(&self, token: &Session) -> Result<Subscriptions, ApiError> {
        self.send(Endpoint::Subscriptions, Some(token))
    }
    pub fn stats(&self, token: &Session) -> Result<Stats, ApiError> {
        self.send(Endpoint::Stats, Some(token))
    }
    pub fn reset_cards(&self, token: &Session) -> Result<ResetCards, ApiError> {
        self.send(Endpoint::ResetCards, Some(token))
    }
    pub fn announcements(&self) -> Result<Announcements, ApiError> {
        self.send(Endpoint::Announcements, None)
    }
}
fn status_result(status: u16) -> Result<(), ApiError> {
    match status {
        401 => Err(ApiError::SessionExpired),
        403 => Err(ApiError::Forbidden),
        429 => Err(ApiError::RateLimited),
        200..=299 => Ok(()),
        _ => Err(ApiError::Service),
    }
}
pub fn decode_response<T: DeserializeOwned>(status: u16, bytes: &[u8]) -> Result<T, ApiError> {
    status_result(status)?;
    if bytes.len() > MAX_RESPONSE {
        return Err(ApiError::InvalidResponse);
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| ApiError::InvalidResponse)?;
    if value.get("code").and_then(|v| v.as_i64()) == Some(401) {
        return Err(ApiError::SessionExpired);
    }
    if value.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Err(ApiError::InvalidResponse);
    }
    let data = value
        .get("data")
        .filter(|v| !v.is_null())
        .ok_or(ApiError::InvalidResponse)?;
    serde_json::from_value(data.clone()).map_err(|_| ApiError::InvalidResponse)
}
