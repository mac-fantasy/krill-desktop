use crate::{api::ApiError, auth::Session, model::*, presentation::*};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    #[serde(
        rename = "selectedSubscriptionID",
        alias = "SelectedSubscriptionId",
        alias = "selectedSubscriptionId"
    )]
    pub selected_subscription_id: Option<i64>,
    #[serde(
        rename = "lastReadAnnouncementID",
        alias = "LastReadAnnouncementId",
        alias = "lastReadAnnouncementId"
    )]
    pub last_read_announcement_id: i64,
    #[serde(alias = "RefreshSeconds")]
    pub refresh_seconds: u64,
    #[serde(alias = "PanelPinned")]
    pub panel_pinned: bool,
    pub show_percentage: bool,
    pub use_liquid_glass: bool,
    #[serde(alias = "AlwaysOnTop")]
    pub always_on_top: bool,
    #[serde(alias = "SnapToEdge")]
    pub snap_to_edge: bool,
    #[serde(alias = "StartWithWindows")]
    pub start_with_windows: bool,
    #[serde(alias = "OrbLeftPixels")]
    pub orb_left_pixels: Option<i32>,
    #[serde(alias = "OrbTopPixels")]
    pub orb_top_pixels: Option<i32>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            selected_subscription_id: None,
            last_read_announcement_id: 0,
            refresh_seconds: 60,
            panel_pinned: false,
            show_percentage: false,
            use_liquid_glass: true,
            always_on_top: true,
            snap_to_edge: true,
            start_with_windows: false,
            orb_left_pixels: None,
            orb_top_pixels: None,
        }
    }
}
impl Preferences {
    pub fn normalize(&mut self) {
        if ![30, 60, 120, 300].contains(&self.refresh_seconds) {
            self.refresh_seconds = 60;
        }
    }
}

#[derive(Clone, Debug)]
pub enum RefreshEvent {
    Core(Profile, Subscriptions),
    Stats(Stats),
    Cards(ResetCards),
    Announcements(Announcements),
    CoreError(ApiError),
    StatsError(ApiError),
    CardsError(ApiError),
    AnnouncementsError,
    Done,
}
#[derive(Clone, Debug)]
pub struct DashboardState {
    pub preferences: Preferences,
    pub profile: Option<Profile>,
    pub subscriptions: Option<Subscriptions>,
    pub stats: Option<Stats>,
    pub reset_cards: Option<Vec<ResetCard>>,
    pub announcements: Vec<Announcement>,
    pub is_demo: bool,
    pub demo_only: bool,
    pub needs_login: bool,
    pub is_stale: bool,
    pub is_loading: bool,
    pub panel_visible: bool,
    pub status_detail: String,
    pub stats_note: String,
    pub reset_cards_note: String,
    pub announcement_note: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub evaluated_at: DateTime<Utc>,
    pub generation: u64,
    pub failures: u32,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_stats: Option<DateTime<Utc>>,
    pub last_cards: Option<DateTime<Utc>>,
    pub last_announcements: Option<DateTime<Utc>>,
    token: Option<Session>,
}
impl DashboardState {
    pub fn new(preferences: Preferences, token: Option<Session>, demo: bool) -> Self {
        let mut s = Self {
            preferences,
            profile: None,
            subscriptions: None,
            stats: None,
            reset_cards: None,
            announcements: vec![],
            is_demo: false,
            demo_only: demo,
            needs_login: token.is_none(),
            is_stale: false,
            is_loading: false,
            panel_visible: false,
            status_detail: "使用浏览器或官网任意登录方式连接账号。".into(),
            stats_note: "打开面板后自动加载今日统计。".into(),
            reset_cards_note: "连接后加载重置卡。".into(),
            announcement_note: "连接后加载官网最新公告。".into(),
            updated_at: None,
            evaluated_at: Utc::now(),
            generation: 0,
            failures: 0,
            last_attempt: None,
            last_stats: None,
            last_cards: None,
            last_announcements: None,
            token,
        };
        s.preferences.normalize();
        if demo {
            s.use_demo();
        }
        s
    }
    pub fn selected(&self) -> Option<&Subscription> {
        select(
            &self.subscriptions.as_ref()?.subscriptions,
            self.preferences.selected_subscription_id,
            false,
            self.evaluated_at,
        )
    }
    pub fn quota(&self) -> Option<QuotaProjection> {
        Some(QuotaProjection::new(
            self.selected()?,
            self.subscriptions.as_ref()?.request_count_quota.as_ref(),
        ))
    }
    pub fn primary_remaining(&self) -> String {
        self.quota()
            .map(|q| q.amount(q.remaining))
            .unwrap_or_else(|| {
                money(
                    self.subscriptions
                        .as_ref()
                        .and_then(|s| s.credit_balance_usd),
                )
            })
    }
    pub fn menu_text(&self) -> String {
        if self.needs_login {
            "登录".into()
        } else if let Some(p) = self.quota().and_then(|q| q.percent()) {
            format!("{p:.0}%")
        } else if self.selected().is_some() {
            "—".into()
        } else {
            self.primary_remaining()
        }
    }
    pub fn status_title(&self) -> &'static str {
        if self.is_demo {
            "演示模式"
        } else if self.needs_login {
            if self.is_stale {
                "需要重新登录"
            } else {
                "尚未连接"
            }
        } else if self.is_stale {
            "暂时无法同步"
        } else {
            "已连接"
        }
    }
    pub fn tooltip(&self) -> String {
        format!(
            "Krill Desktop\n{} · {}\n{} · {}",
            self.selected()
                .map(|s| s.plan.name.as_str())
                .unwrap_or("充值余额"),
            self.primary_remaining(),
            self.status_title(),
            date(self.updated_at)
        )
    }
    pub fn unread_count(&self) -> usize {
        self.announcements
            .iter()
            .filter(|a| a.id > self.preferences.last_read_announcement_id)
            .count()
    }
    pub fn reset_summary(&self) -> String {
        self.reset_cards
            .as_ref()
            .map(|cards| {
                format!(
                    "{} 张可用 · {} 张记录",
                    cards
                        .iter()
                        .filter(|c| c.can_use(self.evaluated_at))
                        .count(),
                    cards.len()
                )
            })
            .unwrap_or("—".into())
    }
    pub fn apply_subscriptions(&mut self, value: Subscriptions, now: DateTime<Utc>) {
        let was_monthly = self.selected().is_some_and(Subscription::is_monthly);
        self.subscriptions = Some(value);
        self.reconcile(now, was_monthly);
    }
    fn reconcile(&mut self, now: DateTime<Utc>, was_monthly: bool) {
        if let Some(subs) = &self.subscriptions {
            self.preferences.selected_subscription_id = select(
                &subs.subscriptions,
                self.preferences.selected_subscription_id,
                was_monthly,
                now,
            )
            .map(|s| s.subscription_id);
        }
        self.evaluated_at = now;
    }
    pub fn advance_time(&mut self, now: DateTime<Utc>) -> bool {
        let boundary = self
            .subscriptions
            .as_ref()
            .and_then(|s| next_transition(&s.subscriptions, self.evaluated_at));
        let expired_card = self.reset_cards.as_ref().is_some_and(|cards| {
            cards.iter().any(|c| {
                c.expires_at
                    .is_some_and(|t| t > self.evaluated_at && t <= now)
            })
        });
        if boundary.is_some_and(|t| t <= now) || expired_card {
            let monthly = self.selected().is_some_and(Subscription::is_monthly);
            self.reconcile(now, monthly);
            true
        } else {
            false
        }
    }
    pub fn refresh_interval(&self) -> i64 {
        let base = if self.panel_visible {
            self.preferences.refresh_seconds.min(30)
        } else {
            self.preferences.refresh_seconds
        };
        (base * (1 << self.failures.min(3))).min(300) as i64
    }
    pub fn refresh_due(&self, now: DateTime<Utc>) -> bool {
        !self.is_demo
            && self.token.is_some()
            && !self.is_loading
            && self
                .last_attempt
                .is_none_or(|t| (now - t).num_seconds() >= self.refresh_interval())
    }
    pub fn begin_refresh(&mut self, now: DateTime<Utc>) -> Option<(u64, Session)> {
        if self.is_demo || self.is_loading {
            return None;
        }
        let token = self.token.clone()?;
        self.last_attempt = Some(now);
        self.is_loading = true;
        Some((self.generation, token))
    }
    // Returns true if persisted credentials must be removed.
    pub fn apply_event(
        &mut self,
        generation: u64,
        event: RefreshEvent,
        now: DateTime<Utc>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        let error = match &event {
            RefreshEvent::CoreError(e)
            | RefreshEvent::StatsError(e)
            | RefreshEvent::CardsError(e) => Some(*e),
            _ => None,
        };
        if error == Some(ApiError::SessionExpired) {
            self.generation += 1;
            self.token = None;
            self.needs_login = true;
            self.is_stale = true;
            self.is_loading = false;
            self.status_detail = ApiError::SessionExpired.to_string();
            return true;
        }
        match event {
            RefreshEvent::Core(profile, subscriptions) => {
                self.profile = Some(profile);
                self.apply_subscriptions(subscriptions, now);
                self.updated_at = Some(now);
                self.needs_login = false;
                self.is_stale = false;
                self.failures = 0;
                self.status_detail = "账号数据已同步，后台会自动更新。".into();
            }
            RefreshEvent::Stats(stats) => {
                self.stats = Some(stats);
                self.last_stats = Some(now);
                self.stats_note = format!("今日 · 本机时区 · 更新于 {}", date(Some(now)));
            }
            RefreshEvent::Cards(cards) => {
                self.reset_cards = Some(cards.items);
                self.last_cards = Some(now);
                self.reset_cards_note =
                    "重置卡只恢复适用订阅的本周额度；使用与预览请前往官网。".into();
            }
            RefreshEvent::Announcements(mut a) => {
                a.items.sort_by_key(|a| std::cmp::Reverse(a.created_at));
                a.items.truncate(50);
                self.announcements = a.items;
                self.last_announcements = Some(now);
                self.announcement_note = "官网最新公告 · 点击查看完整内容".into();
            }
            RefreshEvent::CoreError(e) => {
                self.failures = self.failures.saturating_add(1);
                self.is_stale = true;
                self.status_detail = e.to_string();
            }
            RefreshEvent::StatsError(_) => {
                self.stats_note = "今日统计暂时无法更新，已保留上次结果。".into()
            }
            RefreshEvent::CardsError(_) => {
                self.reset_cards_note = "重置卡暂时无法更新，已保留上次结果。".into()
            }
            RefreshEvent::AnnouncementsError => {
                self.announcement_note = "公告暂时无法更新，可稍后刷新或在官网查看。".into()
            }
            RefreshEvent::Done => self.is_loading = false,
        }
        false
    }
    pub fn accept_session(&mut self, token: Session, profile: Profile) {
        self.clear_account();
        self.token = Some(token);
        self.profile = Some(profile);
        self.needs_login = false;
    }
    pub fn clear_account(&mut self) {
        let generation = self.generation + 1;
        let preferences = self.preferences.clone();
        let demo_only = self.demo_only;
        let visible = self.panel_visible;
        *self = Self::new(preferences, None, false);
        self.demo_only = demo_only;
        self.panel_visible = visible;
        self.generation = generation;
    }
    pub fn use_demo(&mut self) {
        self.clear_account();
        self.is_demo = true;
        self.needs_login = false;
        self.profile = Some(Profile {
            display_name: Some("Krill 演示用户".into()),
            email: Some("demo@example.com".into()),
            ..Default::default()
        });
        let decode = |bytes: &[u8]| {
            serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["data"].clone()
        };
        let mut subscriptions: Subscriptions = serde_json::from_value(decode(include_bytes!(
            "../../../contracts/fixtures/subscriptions.json"
        )))
        .unwrap();
        // Fixtures carry future dates; demo cards must be effective on the current day.
        for s in &mut subscriptions.subscriptions {
            s.subscription_start_at = None;
            s.subscription_end_at = Some(Utc::now() + chrono::Duration::days(20));
        }
        self.apply_subscriptions(subscriptions, Utc::now());
        self.stats = Some(
            serde_json::from_value(decode(include_bytes!(
                "../../../contracts/fixtures/stats.json"
            )))
            .unwrap(),
        );
        let cards: ResetCards = serde_json::from_value(decode(include_bytes!(
            "../../../contracts/fixtures/reset-cards.json"
        )))
        .unwrap();
        self.reset_cards = Some(cards.items);
        self.announcements = vec![Announcement {
            id: 102,
            title: "欢迎使用 Krill Desktop".into(),
            summary: Some("虚构演示公告。额度、套餐与统计随手可见。".into()),
            created_at: Some(Utc::now()),
            ..Default::default()
        }];
        self.updated_at = Some(Utc::now());
        self.status_detail = "虚构样例，不代表真实账号。".into();
        self.stats_note = "今日 · 演示数据".into();
        self.announcement_note = "虚构演示公告".into();
    }
}
