use std::fmt;
use url::Url;

pub const ORIGIN: &str = "https://www.krill-code.com";
pub const LOGIN_URL: &str = "https://www.krill-code.com/login";
pub const EXPORT_SCRIPT: &str = "(() => {\n  if (location.origin !== 'https://www.krill-code.com') { alert('请在 Krill 官网运行'); return; }\n  const token = localStorage.getItem('krill_jwt');\n  if (!token) { alert('请先完成 Krill 登录'); return; }\n  prompt('复制 Krill 会话并粘贴到 Krill Desktop（请勿分享给他人）', token);\n})()";
pub const SESSION_SCRIPT: &str = "(() => { if (location.origin !== 'https://www.krill-code.com') return null; return {origin: location.origin, token: localStorage.getItem('krill_jwt')}; })()";

#[derive(Clone)]
pub struct Session(String);
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session([redacted])")
    }
}
impl Session {
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > 16400 {
            return None;
        }
        let value = text.trim();
        let value = value.strip_prefix("Bearer ").unwrap_or(value);
        if !(16..=16384).contains(&value.len()) {
            return None;
        }
        let parts: Vec<_> = value.split('.').collect();
        if parts.len() != 3
            || !parts.iter().all(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
        {
            return None;
        }
        Some(Self(value.into()))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
pub fn allows_navigation(value: &str) -> bool {
    Url::parse(value).ok().is_some_and(|u| {
        u.scheme() == "https"
            && u.host_str().is_some()
            && u.port_or_known_default() == Some(443)
            && u.username().is_empty()
            && u.password().is_none()
    })
}
pub fn trusted_origin(value: &str) -> bool {
    allows_navigation(value)
        && Url::parse(value)
            .ok()
            .is_some_and(|u| u.host_str() == Some("www.krill-code.com"))
}
pub fn extracted_session(page_url: &str, script_origin: &str, value: &str) -> Option<Session> {
    if !trusted_origin(page_url) || script_origin != ORIGIN {
        return None;
    }
    Session::parse(value)
}
