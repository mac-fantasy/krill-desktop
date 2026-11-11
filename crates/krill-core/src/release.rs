use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
pub const REPOSITORY: &str = "mac-fantasy/krill-desktop";
pub const LATEST_URL: &str =
    "https://api.github.com/repos/mac-fantasy/krill-desktop/releases/latest";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u16, pub u16, pub u16);
impl FromStr for Version {
    type Err = ();
    fn from_str(text: &str) -> Result<Self, ()> {
        let parts: Vec<_> = text.strip_prefix('v').unwrap_or(text).split('.').collect();
        if parts.len() != 3 {
            return Err(());
        }
        let mut numbers = [0; 3];
        for (i, p) in parts.iter().enumerate() {
            if p.is_empty()
                || !p.bytes().all(|c| c.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
            {
                return Err(());
            }
            numbers[i] = p.parse().map_err(|_| ())?;
        }
        Ok(Self(numbers[0], numbers[1], numbers[2]))
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Release {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub digest: Option<String>,
    pub size: u64,
}
impl Asset {
    pub fn sha256(&self) -> Option<String> {
        let hex = self.digest.as_deref()?.strip_prefix("sha256:")?;
        (hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit()))
            .then(|| hex.to_ascii_lowercase())
    }
}
impl Release {
    pub fn update(&self, current: Version, platform: &str) -> Option<&Asset> {
        let version: Version = self.tag_name.parse().ok()?;
        if self.draft
            || self.prerelease
            || version <= current
            || self.tag_name != format!("v{version}")
        {
            return None;
        }
        let name = format!("Krill.Desktop-{version}-{platform}.zip");
        self.assets.iter().find(|a| {
            a.name == name
                && a.sha256().is_some()
                && a.size > 0
                && a.size <= 1_073_741_824
                && a.browser_download_url
                    == format!(
                        "https://github.com/{REPOSITORY}/releases/download/{}/{name}",
                        self.tag_name
                    )
        })
    }
}
