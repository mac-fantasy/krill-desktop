use crate::browser::BrowserLogin;
use crate::storage::Storage;
use crate::updates;
use crate::{
    platform,
    tray::{SystemTray, TrayAction},
};
use chrono::{DateTime, Local, Utc};
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use gpui_kit::{prelude::*, *};
use krill_core::{
    self as core,
    api::{Api, ApiError},
    auth::{EXPORT_SCRIPT, LOGIN_URL, ORIGIN, Session},
    state::{DashboardState, RefreshEvent},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

const MINT: u32 = 0x7ce3cd;
const MUTED: u32 = 0x8fa4ad;
const BG: u32 = 0x0e191f;
const CARD: u32 = 0x16262f;
const BORDER: u32 = 0x2c414b;
const TABS: [&str; 5] = ["概览", "套餐", "用量", "公告", "设置"];

enum Event {
    Refresh(u64, RefreshEvent),
    Login(u64, Result<(Session, core::Profile), ApiError>),
    Tray(TrayAction),
    UpdateCheck(Result<Option<updates::AvailableUpdate>, String>),
    UpdatePrepared(Result<updates::PreparedUpdate, String>),
    BrowserCandidate(u64, Session),
    BrowserClosed(u64),
}

pub struct Dashboard {
    pub state: DashboardState,
    storage: Storage,
    api: Option<Api>,
    tab: usize,
    scroll: ScrollHandle,
    input: Entity<InputState>,
    login_open: bool,
    login_busy: bool,
    login_note: String,
    confirm_sign_out: bool,
    hovered: Option<DateTime<Utc>>,
    sender: async_channel::Sender<Event>,
    epoch: Arc<AtomicU64>,
    tray: Option<SystemTray>,
    window: AnyWindowHandle,
    focus: FocusHandle,
    _subscriptions: Vec<gpui_kit::Subscription>,
    launch_at_login: bool,
    launch_approval: bool,
    settings_note: String,
    updater: updates::UpdateState,
    #[cfg(target_os = "windows")]
    _instance: Option<crate::windows_native::Instance>,
    browser: Option<Entity<BrowserLogin>>,
    browser_window: Option<AnyWindowHandle>,
    #[cfg(target_os = "windows")]
    pub(crate) orb_window: Option<AnyWindowHandle>,
    #[cfg(target_os = "windows")]
    pub(crate) orb: Option<Entity<crate::orb::Orb>>,
    #[cfg(target_os = "windows")]
    pub(crate) orb_visible: bool,
}
impl Dashboard {
    pub fn new(
        storage: Storage,
        demo: bool,
        show: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preferences = storage.preferences();
        let token = storage.session();
        let mut state = DashboardState::new(
            preferences,
            token.as_ref().ok().and_then(|t| t.clone()),
            demo,
        );
        if let Err(e) = token {
            state.status_detail = e.to_string();
        }
        state.panel_visible = show;
        let (sender, receiver) = async_channel::unbounded();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                if this.update(cx, |this, cx| this.event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let mut last_tick = Utc::now();
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update(cx, |this, cx| {
                        let now = Utc::now();
                        // A long gap means the machine or event loop was suspended.
                        // Clear retry backoff so a resumed client resynchronizes promptly.
                        if (now - last_tick).num_seconds() > 5 {
                            this.state.last_attempt = None;
                        }
                        last_tick = now;
                        if this.state.advance_time(now) {
                            this.save();
                            cx.notify();
                        }
                        if this.state.refresh_due(now) {
                            this.refresh(cx);
                        }
                        if !this.state.is_demo
                            && this
                                .updater
                                .last_check
                                .is_none_or(|t| (now - t).num_seconds() >= 6 * 60 * 60)
                        {
                            this.check_update(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("粘贴 Krill 会话")
                .masked(true)
        });
        let tray_sender = sender.clone();
        let tray = if cfg!(test) {
            None
        } else {
            match SystemTray::new(move |action| {
                let _ = tray_sender.try_send(Event::Tray(action));
            }) {
                Ok(mut tray) => {
                    let _ = tray.update(&state);
                    Some(tray)
                }
                Err(_) => {
                    state.status_detail = "无法创建系统托盘，已打开详情面板。".into();
                    state.panel_visible = true;
                    platform::show(window);
                    None
                }
            }
        };
        platform::configure_panel(window);
        #[cfg(target_os = "windows")]
        if !cfg!(test) {
            let _ =
                crate::windows_native::configure(window, false, state.preferences.always_on_top);
        }
        platform::set_material(window, state.preferences.use_liquid_glass);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if this.state.panel_visible
                && !window.is_window_active()
                && !this.state.preferences.panel_pinned
                && !this.login_open
            {
                this.state.panel_visible = false;
                this.hovered = None;
                platform::hide(window);
                cx.notify();
            }
        });
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.state.panel_visible = false;
                this.hovered = None;
                cx.notify();
            });
            platform::hide(window);
            false
        });
        let (launch_at_login, launch_approval) = platform::launch_status();
        let state_epoch = state.generation;
        Self {
            state,
            storage,
            api: Api::new().ok(),
            tab: 0,
            scroll: ScrollHandle::new(),
            input,
            login_open: false,
            login_busy: false,
            login_note: "在默认浏览器登录后，按指引手动导入一次会话。".into(),
            confirm_sign_out: false,
            hovered: None,
            sender,
            epoch: Arc::new(AtomicU64::new(state_epoch)),
            tray,
            window: window.window_handle(),
            focus,
            _subscriptions: vec![activation],
            launch_at_login,
            launch_approval,
            settings_note: String::new(),
            updater: updates::UpdateState::default(),
            #[cfg(target_os = "windows")]
            _instance: None,
            browser: None,
            browser_window: None,
            #[cfg(target_os = "windows")]
            orb_window: None,
            #[cfg(target_os = "windows")]
            orb: None,
            #[cfg(target_os = "windows")]
            orb_visible: false,
        }
    }
    pub(crate) fn save(&mut self) {
        if self
            .storage
            .save_preferences(&self.state.preferences)
            .is_err()
        {
            self.state.status_detail = "本次设置暂时无法保存，请检查应用数据目录。".into();
        }
        self.update_tray();
    }
    #[cfg(target_os = "windows")]
    pub fn watch_instance(&mut self, instance: crate::windows_native::Instance) {
        let sender = self.sender.clone();
        instance.watch(move || {
            let _ = sender.try_send(Event::Tray(TrayAction::Open));
        });
        self._instance = Some(instance);
    }
    fn update_tray(&mut self) {
        if let Some(tray) = &mut self.tray {
            let _ = tray.update(&self.state);
        }
    }
    fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.updater.busy {
            return;
        }
        self.updater.busy = true;
        self.updater.last_check = Some(Utc::now());
        self.updater.status = "正在检查 GitHub Release…".into();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send_blocking(Event::UpdateCheck(
                updates::check().map_err(|e| e.to_string()),
            ));
        });
        cx.notify();
    }
    fn install_update(&mut self, cx: &mut Context<Self>) {
        if self.updater.busy {
            return;
        }
        let Some(update) = self.updater.available.clone() else {
            return;
        };
        let destination = match updates::destination() {
            Ok(d) => d,
            Err(e) => {
                self.updater.status = e.to_string();
                cx.notify();
                return;
            }
        };
        self.updater.busy = true;
        self.updater.status = format!("正在下载并校验 {}…", update.version);
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send_blocking(Event::UpdatePrepared(
                updates::download(&update, &destination).map_err(|e| e.to_string()),
            ));
        });
        cx.notify();
    }
    pub(crate) fn show_panel(&mut self, cx: &mut Context<Self>) {
        self.state.panel_visible = true;
        #[cfg(target_os = "windows")]
        if !self.orb_visible {
            if let Some(orb) = self.orb_window {
                let _ = orb.update(cx, |_, window, _| {
                    crate::windows_native::show_without_activation(window)
                });
            }
            self.orb_visible = true;
        }
        let rect = self.tray.as_ref().and_then(|t| t.rect());
        let focus = self.focus.clone();
        #[cfg(target_os = "macos")]
        let status_window = self.tray.as_ref().and_then(|t| t.status_window());
        #[cfg(target_os = "windows")]
        let anchor = self.orb_window.and_then(|orb| {
            orb.update(cx, |_, window, _| crate::windows_native::rect(window).ok())
                .ok()
                .flatten()
        });
        let _ = self.window.update(cx, |_, window, cx| {
            platform::place_panel(window, rect);
            #[cfg(target_os = "macos")]
            if let Some(status_window) = status_window {
                platform::place_mac_panel(window, &status_window);
            }
            #[cfg(target_os = "windows")]
            if let Some(anchor) = anchor {
                let _ = crate::windows_native::place_panel(window, anchor);
            }
            platform::show(window);
            window.activate_window();
            window.focus(&focus, cx);
            cx.activate(true);
        });
        (self.launch_at_login, self.launch_approval) = platform::launch_status();
        self.refresh(cx);
        cx.notify();
    }
    #[cfg(target_os = "windows")]
    pub(crate) fn toggle_panel(&mut self, cx: &mut Context<Self>) {
        if self.state.panel_visible {
            self.hide_panel(cx);
        } else {
            self.show_panel(cx);
        }
    }
    #[cfg(target_os = "windows")]
    pub(crate) fn show_context(&self, window: &Window, cx: &mut App) {
        if let Some(tray) = &self.tray {
            tray.show_context(window, cx);
        }
    }
    fn hide_panel(&mut self, cx: &mut Context<Self>) {
        self.state.panel_visible = false;
        self.hovered = None;
        let _ = self
            .window
            .update(cx, |_, window, _| platform::hide(window));
        cx.notify();
    }
    fn sync_epoch(&self) {
        self.epoch.store(self.state.generation, Ordering::Release);
    }
    fn open_embedded(&mut self, cx: &mut Context<Self>) {
        if self.state.demo_only || self.login_busy {
            return;
        }
        if let Some(window) = self.browser_window {
            let _ = window.update(cx, |_, window, cx| {
                platform::show(window);
                window.activate_window();
                cx.activate(true);
            });
            return;
        }
        let generation = self.state.generation;
        let candidate = self.sender.clone();
        let closed = self.sender.clone();
        match BrowserLogin::open(
            cx,
            move |session| {
                let _ = candidate.try_send(Event::BrowserCandidate(generation, session));
            },
            move || {
                let _ = closed.try_send(Event::BrowserClosed(generation));
            },
        ) {
            Ok((window, browser)) => {
                self.browser_window = Some(window);
                self.browser = Some(browser);
            }
            Err(_) => self.login_note = "无法启动应用内浏览器，请改用系统浏览器登录。".into(),
        }
        cx.notify();
    }
    fn close_embedded(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = self.browser.take() {
            browser.update(cx, |browser, _| browser.close());
        }
        if let Some(window) = self.browser_window.take() {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }
    fn validate_session(&mut self, session: Session, cx: &mut Context<Self>) {
        if self.login_busy || self.state.demo_only {
            return;
        }
        let Some(api) = self.api.clone() else {
            self.login_note = ApiError::Network.to_string();
            return;
        };
        self.login_busy = true;
        self.login_note = "正在验证会话…".into();
        let generation = self.state.generation;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = api.profile(&session).map(|profile| (session, profile));
            let _ = sender.send_blocking(Event::Login(generation, result));
        });
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.api.clone() else {
            self.state.status_detail = ApiError::Network.to_string();
            cx.notify();
            return;
        };
        let now = Utc::now();
        let Some((generation, token)) = self.state.begin_refresh(now) else {
            return;
        };
        self.sync_epoch();
        let epoch = self.epoch.clone();
        let sender = self.sender.clone();
        let profile = self.state.profile.clone();
        let details = self.state.panel_visible
            && self
                .state
                .last_stats
                .is_none_or(|t| (now - t).num_seconds() >= 30);
        let cards = self.state.panel_visible
            && self
                .state
                .last_cards
                .is_none_or(|t| (now - t).num_seconds() >= 30);
        let announcements = self
            .state
            .last_announcements
            .is_none_or(|t| (now - t).num_seconds() >= 600);
        std::thread::spawn(move || {
            let send = |event| {
                if epoch.load(Ordering::Acquire) != generation {
                    return false;
                }
                sender
                    .send_blocking(Event::Refresh(generation, event))
                    .is_ok()
            };
            let result = profile
                .map(Ok)
                .unwrap_or_else(|| api.profile(&token))
                .and_then(|profile| api.subscriptions(&token).map(|s| (profile, s)));
            match result {
                Ok((p, s)) => {
                    if !send(RefreshEvent::Core(p, s)) {
                        return;
                    }
                }
                Err(e) => {
                    send(RefreshEvent::CoreError(e));
                    send(RefreshEvent::Done);
                    return;
                }
            }
            if cards && epoch.load(Ordering::Acquire) == generation {
                let result = api.reset_cards(&token);
                let expired = matches!(result, Err(ApiError::SessionExpired));
                if !send(match result {
                    Ok(v) => RefreshEvent::Cards(v),
                    Err(e) => RefreshEvent::CardsError(e),
                }) || expired
                {
                    return;
                }
            }
            if details && epoch.load(Ordering::Acquire) == generation {
                let result = api.stats(&token);
                let expired = matches!(result, Err(ApiError::SessionExpired));
                if !send(match result {
                    Ok(v) => RefreshEvent::Stats(v),
                    Err(e) => RefreshEvent::StatsError(e),
                }) || expired
                {
                    return;
                }
            }
            if announcements && epoch.load(Ordering::Acquire) == generation {
                if !send(match api.announcements() {
                    Ok(v) => RefreshEvent::Announcements(v),
                    Err(_) => RefreshEvent::AnnouncementsError,
                }) {
                    return;
                }
            }
            send(RefreshEvent::Done);
        });
        cx.notify();
    }
    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Refresh(generation, event) => {
                let previous = self.state.preferences.selected_subscription_id;
                if self.state.apply_event(generation, event, Utc::now()) {
                    self.sync_epoch();
                    if let Err(e) = self.storage.delete_session() {
                        self.state.status_detail = e.to_string();
                    }
                }
                if previous != self.state.preferences.selected_subscription_id {
                    self.save();
                }
            }
            Event::Login(generation, result) => {
                if generation != self.state.generation {
                    return;
                }
                self.login_busy = false;
                match result {
                    Ok((session, profile)) => match self.storage.save_session(&session) {
                        Ok(()) => {
                            self.state.accept_session(session, profile);
                            self.sync_epoch();
                            self.login_open = false;
                            self.close_embedded(cx);
                            self.refresh(cx);
                        }
                        Err(e) => self.login_note = e.to_string(),
                    },
                    Err(e) => self.login_note = e.to_string(),
                }
                if let Some(browser) = &self.browser {
                    let note = self.login_note.clone();
                    browser.update(cx, |browser, cx| browser.set_note(note, cx));
                }
            }
            Event::Tray(action) => match action {
                TrayAction::Toggle => {
                    if self.state.panel_visible {
                        self.hide_panel(cx)
                    } else {
                        self.show_panel(cx)
                    }
                }
                TrayAction::Open => self.show_panel(cx),
                TrayAction::Refresh => self.refresh(cx),
                TrayAction::Website => cx.open_url(&format!("{ORIGIN}/app/account")),
                TrayAction::Quit => cx.quit(),
                #[cfg(target_os = "windows")]
                TrayAction::ToggleOrb => {
                    self.orb_visible = !self.orb_visible;
                    if !self.orb_visible {
                        self.hide_panel(cx);
                    }
                    if let Some(orb) = self.orb_window {
                        let show = self.orb_visible;
                        let _ = orb.update(cx, |_, window, _| {
                            if show {
                                let _ = crate::windows_native::show_without_activation(window);
                            } else {
                                platform::hide(window);
                            }
                        });
                    }
                }
                TrayAction::CheckUpdate => {
                    self.tab = 4;
                    self.show_panel(cx);
                    self.check_update(cx);
                }
            },
            Event::UpdateCheck(result) => {
                self.updater.busy = false;
                match result {
                    Ok(update) => {
                        self.updater.status = update
                            .as_ref()
                            .map(|u| format!("发现新版本 {}，可下载并重启更新。", u.version))
                            .unwrap_or_else(|| {
                                format!("当前已是最新版本（{}）。", updates::CURRENT_VERSION)
                            });
                        self.updater.available = update;
                    }
                    Err(error) => self.updater.status = error,
                }
            }
            Event::UpdatePrepared(result) => {
                self.updater.busy = false;
                match result {
                    Ok(prepared) => match prepared.install() {
                        Ok(()) => cx.quit(),
                        Err(error) => self.updater.status = error.to_string(),
                    },
                    Err(error) => self.updater.status = error,
                }
            }
            Event::BrowserCandidate(generation, session) => {
                if generation == self.state.generation {
                    self.validate_session(session, cx);
                }
            }
            Event::BrowserClosed(generation) => {
                if generation == self.state.generation {
                    self.close_embedded(cx);
                    self.state.generation += 1;
                    self.sync_epoch();
                    self.state.is_loading = false;
                    self.login_busy = false;
                    cx.notify();
                }
            }
        }
        self.update_tray();
        cx.notify();
    }
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.login_busy || self.state.demo_only {
            return;
        }
        let Some(session) = Session::parse(self.input.read(cx).value().as_ref()) else {
            self.login_note = "会话格式不正确，请粘贴脚本弹出的完整 Krill 会话。".into();
            cx.notify();
            return;
        };
        self.input
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.validate_session(session, cx);
    }
    pub(crate) fn switch_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = tab;
        self.scroll = ScrollHandle::new();
        self.hovered = None;
        if tab == 2 || tab == 3 {
            self.refresh(cx);
        }
        cx.notify();
    }
    fn sign_out(&mut self, cx: &mut Context<Self>) {
        self.close_embedded(cx);
        self.state.clear_account();
        self.sync_epoch();
        self.confirm_sign_out = false;
        self.tab = 0;
        if let Err(e) = self.storage.delete_session() {
            self.state.status_detail = e.to_string();
            self.state.is_stale = true;
        }
        cx.notify();
    }
    fn primary(&self) -> Div {
        let q = self.state.quota();
        let percent = q.as_ref().and_then(|q| q.percent());
        card()
            .bg(rgb(0x193137))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .flex_1()
                            .child(
                                div().text_sm().child(
                                    self.state
                                        .selected()
                                        .map(|s| s.plan.name.clone())
                                        .unwrap_or("按余额使用".into()),
                                ),
                            )
                            .child(label(
                                q.as_ref()
                                    .map(|q| q.cycle.clone())
                                    .unwrap_or("充值余额".into()),
                            ))
                            .child(
                                div()
                                    .text_size(px(36.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MINT))
                                    .child(self.state.primary_remaining()),
                            )
                            .child(label(
                                q.as_ref()
                                    .map(|q| {
                                        format!(
                                            "已用 {} / 上限 {}",
                                            q.amount(q.used),
                                            q.amount(q.limit)
                                        )
                                    })
                                    .unwrap_or("余额按实际使用扣除".into()),
                            )),
                    )
                    .child(ring(percent)),
            )
            .child(
                div()
                    .mt_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(label(
                        q.as_ref()
                            .and_then(|q| q.reset_at)
                            .map(|t| format!("{} 重置", core::date(Some(t))))
                            .unwrap_or("周期时间以官网为准".into()),
                    )),
            )
            .children(
                q.as_ref()
                    .and_then(|q| q.forwarded)
                    .filter(|v| *v > rust_decimal::Decimal::ZERO)
                    .map(|v| label(format!("另有结转 {}", core::money(Some(v))))),
            )
    }
    fn overview(&self, cx: &mut Context<Self>) -> Div {
        let profile = self.state.profile.as_ref();
        let subscriptions = self.state.subscriptions.as_ref();
        column()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(profile.map(|p| p.friendly_name()).unwrap_or_default())
                            .child(label(
                                profile.and_then(|p| p.email.clone()).unwrap_or_default(),
                            )),
                    )
                    .child(
                        Button::new("account")
                            .ghost()
                            .small()
                            .label("账户 ↗")
                            .on_click(|_, _, cx| cx.open_url(&format!("{ORIGIN}/app/account"))),
                    ),
            )
            .child(self.primary())
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(metric(
                        "充值余额 / USD",
                        core::money(subscriptions.and_then(|s| s.credit_balance_usd)),
                    ))
                    .child(metric(
                        "福利余额 / USD",
                        core::money(subscriptions.and_then(|s| s.welfare_balance_usd)),
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child("今日用量")
                    .child(
                        Button::new("usage-more")
                            .ghost()
                            .small()
                            .label("查看详情")
                            .on_click(cx.listener(|this, _, _, cx| this.switch_tab(2, cx))),
                    ),
            )
            .child(self.metrics())
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(metric(
                        "缓存率",
                        core::percentage(self.state.stats.as_ref().and_then(|s| s.cache_percent())),
                    ))
                    .child(metric("重置卡", self.state.reset_summary())),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child("最新公告")
                    .child(
                        Button::new("announcements-more")
                            .ghost()
                            .small()
                            .label("全部公告")
                            .on_click(cx.listener(|this, _, _, cx| this.switch_tab(3, cx))),
                    ),
            )
            .children(
                self.state
                    .announcements
                    .iter()
                    .take(2)
                    .map(|a| self.announcement(a, cx)),
            )
            .when(self.state.announcements.is_empty(), |d| {
                d.child(label(self.state.announcement_note.clone()))
            })
    }
    fn metrics(&self) -> Div {
        let stats = self.state.stats.as_ref();
        div()
            .flex()
            .gap_3()
            .child(metric(
                "请求数",
                stats
                    .map(|s| core::compact(s.total_requests))
                    .unwrap_or("—".into()),
            ))
            .child(metric(
                "Tokens",
                stats
                    .map(|s| core::compact(s.total_tokens))
                    .unwrap_or("—".into()),
            ))
            .child(metric(
                "费用 / USD",
                core::cost(stats.and_then(|s| s.total_cost_usd)),
            ))
    }
    fn plans(&self, cx: &mut Context<Self>) -> Div {
        let mut content = column()
            .child(heading("我的套餐"))
            .child(
                Button::new("shop")
                    .ghost()
                    .label("前往选购 ↗")
                    .on_click(|_, _, cx| cx.open_url(&format!("{ORIGIN}/app/shop"))),
            )
            .child(label("显示的套餐"));
        if let Some(subscriptions) = &self.state.subscriptions {
            let plans = core::ordered(&subscriptions.subscriptions);
            for plan in &plans {
                if plan.availability(self.state.evaluated_at) != core::Availability::Available {
                    continue;
                }
                let id = plan.subscription_id;
                let selected = self.state.preferences.selected_subscription_id == Some(id);
                content = content.child(
                    Button::new(("select-plan", id as usize))
                        .label(format!(
                            "{}{}",
                            if selected { "✓ " } else { "" },
                            plan.plan.name
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.preferences.selected_subscription_id = Some(id);
                            this.save();
                            cx.notify();
                        })),
                );
            }
            for plan in plans {
                let quota =
                    core::QuotaProjection::new(plan, subscriptions.request_count_quota.as_ref());
                let state = plan.availability(self.state.evaluated_at);
                let mut view = card()
                    .when(state != core::Availability::Available, |d| d.opacity(0.55))
                    .child(row(plan.plan.name.clone(), state.label().into()))
                    .child(label(quota.cycle.clone()))
                    .child(
                        div()
                            .text_size(px(28.))
                            .text_color(rgb(MINT))
                            .child(quota.amount(quota.remaining)),
                    )
                    .child(progress(quota.percent()))
                    .child(row("已用".into(), quota.amount(quota.used)))
                    .child(row("上限".into(), quota.amount(quota.limit)));
                if plan.plan.billing_type.as_deref() == Some("request_count") {
                    view = view.child(label("狂蹬卡 / 按次额度 · 账号共享限额"));
                    let windows = subscriptions
                        .request_count_quota
                        .as_ref()
                        .map(|q| q.windows())
                        .unwrap_or([
                            core::RequestWindow {
                                label: "5 小时",
                                used: None,
                                limit: plan.plan.rate_limit_5h,
                            },
                            core::RequestWindow {
                                label: "本周",
                                used: None,
                                limit: plan.plan.rate_limit_weekly,
                            },
                            core::RequestWindow {
                                label: "本月",
                                used: None,
                                limit: plan.plan.rate_limit_monthly,
                            },
                        ]);
                    for w in windows {
                        view = view
                            .child(row(w.label.into(), w.detail()))
                            .child(progress(w.percent()));
                    }
                    view = view.child(label(
                        subscriptions
                            .request_count_quota
                            .as_ref()
                            .map(|q| q.cycle())
                            .unwrap_or("周期时间未提供".into()),
                    ));
                }
                if quota.forwarded.is_some() {
                    view = view.child(row("另有结转".into(), core::money(quota.forwarded)));
                }
                view = view.child(label(format!(
                    "{} 重置 · {} 到期",
                    core::date(quota.reset_at),
                    core::date(plan.subscription_end_at)
                )));
                content = content.child(view);
            }
        }
        content = content
            .child(heading("重置卡"))
            .child(label(self.state.reset_summary()))
            .child(label(self.state.reset_cards_note.clone()));
        if let Some(cards) = &self.state.reset_cards {
            if cards.is_empty() {
                content = content.child(label("暂无重置卡"));
            }
            for c in cards {
                let mut view = card()
                    .child(row(
                        c.plan_name.clone().unwrap_or("重置卡".into()),
                        format!("#{}", c.id),
                    ))
                    .child(label(c.availability_text(self.state.evaluated_at)))
                    .child(row("单张恢复上限".into(), core::money(c.cap_usd)))
                    .child(row(
                        "有效期".into(),
                        c.expires_at
                            .map(|t| format!("{} 前有效", core::date(Some(t))))
                            .unwrap_or("长期有效".into()),
                    ));
                if c.state(self.state.evaluated_at) == core::ResetCardState::Used {
                    view = view
                        .child(row("实际恢复".into(), core::money(c.granted_usd)))
                        .child(row("使用时间".into(), core::date(c.used_at)));
                }
                if let Some(note) = &c.note {
                    view = view.child(label(core::plain_text(Some(note), 180)));
                }
                content = content.child(view);
            }
        }
        content
            .child(
                Button::new("reset-cards-site")
                    .ghost()
                    .label("在官网管理重置卡 ↗")
                    .on_click(|_, _, cx| cx.open_url(&format!("{ORIGIN}/app"))),
            )
            .child(label(
                "金额单位为 USD。结转额度单独显示；套餐变更、续费与支付请前往官网。",
            ))
    }
    fn usage(&self, cx: &mut Context<Self>) -> Div {
        let stats = self.state.stats.as_ref();
        let chart = core::TrendChart::new(stats.map(|s| s.trend.as_slice()).unwrap_or(&[]));
        let selected = self
            .hovered
            .and_then(|start| chart.buckets.iter().position(|b| b.bucket_start == start));
        let mut chart_card = card().child("时段费用");
        if chart.buckets.is_empty() {
            chart_card = chart_card.child(
                div()
                    .h(px(104.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(label("暂无时段统计")),
            );
        } else {
            chart_card = chart_card.child(
                div()
                    .h(px(32.))
                    .text_size(px(11.))
                    .text_color(rgb(if selected.is_some() { MINT } else { MUTED }))
                    .child(
                        selected
                            .map(|i| chart.tooltip(i))
                            .unwrap_or("悬停图表查看时段、费用与请求数".into()),
                    ),
            );
            let mut plot = div()
                .id("usage-plot")
                .debug_selector(|| "usage-plot".into())
                .h(px(104.))
                .w_full()
                .flex()
                .items_end()
                .border_b_1()
                .border_color(rgb(BORDER))
                .on_hover(cx.listener(|this, hover: &bool, _, cx| {
                    if !*hover {
                        this.hovered = None;
                        cx.notify();
                    }
                }));
            for (i, bucket) in chart.buckets.iter().enumerate() {
                let start = bucket.bucket_start;
                let active = selected == Some(i);
                plot = plot.child(
                    div()
                        .id(("bucket", i))
                        .h_full()
                        .flex_1()
                        .min_w_0()
                        .relative()
                        .flex()
                        .items_end()
                        .px(px(2.))
                        .on_hover(cx.listener(move |this, hover: &bool, _, cx| {
                            if *hover {
                                this.hovered = Some(start);
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .w_full()
                                .h(px(chart.height(i)))
                                .rounded_t(px(2.))
                                .bg(rgb(if bucket.total_cost_usd.is_some() {
                                    MINT
                                } else {
                                    MUTED
                                }))
                                .opacity(if active { 1. } else { 0.75 }),
                        )
                        .when(active, |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(relative(0.5))
                                    .w(px(1.))
                                    .bg(rgb(MINT))
                                    .opacity(0.5),
                            )
                        }),
                );
            }
            let mut axis = div().flex().justify_between().h(px(26.)).pt_2();
            for i in chart.axis_indices() {
                axis = axis.child(
                    div().text_size(px(9.)).text_color(rgb(MUTED)).child(
                        chart.buckets[i]
                            .bucket_start
                            .with_timezone(&Local)
                            .format("%H:%M")
                            .to_string(),
                    ),
                );
            }
            chart_card = chart_card.child(plot).child(axis);
        }
        let success = stats
            .filter(|s| s.total_requests > 0)
            .map(|s| {
                format!(
                    "{:.1}%",
                    s.success_requests as f64 / s.total_requests as f64 * 100.
                )
            })
            .unwrap_or("—".into());
        let details = card()
            .child(row("请求成功率".into(), success))
            .child(row(
                "成功请求".into(),
                core::count(stats.map(|s| s.success_requests)),
            ))
            .child(row(
                "失败请求".into(),
                core::count(stats.and_then(|s| s.failed_requests)),
            ))
            .child(row(
                "每分钟请求 / RPM".into(),
                core::rate(stats.and_then(|s| s.rpm)),
            ))
            .child(row(
                "每分钟 Tokens / TPM".into(),
                core::rate(stats.and_then(|s| s.tpm)),
            ))
            .child(row(
                "输入 Tokens".into(),
                core::count(stats.and_then(|s| s.input_tokens)),
            ))
            .child(row(
                "输出 Tokens".into(),
                core::count(stats.map(|s| s.output_tokens)),
            ))
            .child(row(
                "思考 Tokens".into(),
                core::count(stats.and_then(|s| s.reasoning_tokens)),
            ));
        let mut cache = card()
            .child("缓存用量")
            .child(row(
                "缓存率".into(),
                core::percentage(stats.and_then(|s| s.cache_percent())),
            ))
            .child(row(
                "缓存读取 Tokens".into(),
                core::count(stats.and_then(|s| s.cache_read_input_tokens)),
            ))
            .child(row(
                "缓存写入 Tokens".into(),
                core::count(stats.and_then(|s| s.cache_creation_input_tokens)),
            ))
            .child(label(
                "优先使用官网缓存统计口径；缺失时按缓存读取 /（输入 + 缓存写入 + 缓存读取）计算。",
            ));
        if let Some(stats) = stats {
            for channel in &stats.channel_cache_rates {
                cache = cache.child(row(
                    core::plain_text(Some(&channel.channel_name), 60),
                    core::percentage(
                        channel
                            .cache_rate
                            .map(|v| v * rust_decimal::Decimal::ONE_HUNDRED),
                    ),
                ));
            }
        }
        column()
            .child(heading("今日用量"))
            .child(label(self.state.stats_note.clone()))
            .child(self.metrics())
            .child(chart_card)
            .child(details)
            .child(cache)
            .child(
                Button::new("usage-site")
                    .ghost()
                    .label("在官网查看完整用量记录 ↗")
                    .on_click(|_, _, cx| cx.open_url(&format!("{ORIGIN}/app/logs"))),
            )
    }
    fn announcement(&self, a: &core::Announcement, cx: &mut Context<Self>) -> AnyElement {
        let id = a.id;
        card()
            .id(("announcement", id as usize))
            .cursor_pointer()
            .child(label(format!(
                "{} {}",
                core::date(a.created_at),
                if id > self.state.preferences.last_read_announcement_id {
                    "• 未读"
                } else {
                    ""
                }
            )))
            .child(core::plain_text(Some(&a.title), 80))
            .child(label(core::plain_text(
                a.summary.as_deref().or(a.content.as_deref()),
                180,
            )))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.state.preferences.last_read_announcement_id =
                    this.state.preferences.last_read_announcement_id.max(id);
                this.save();
                cx.open_url(&format!("{ORIGIN}/app/announcements"));
                cx.notify();
            }))
            .into_any_element()
    }
    fn settings(&self, cx: &mut Context<Self>) -> Div {
        column()
            .child(heading("按你的习惯来"))
            .child(label("设置自动保存，仅作用于当前设备。"))
            .when(!self.settings_note.is_empty(), |d| {
                d.child(label(self.settings_note.clone()))
            })
            .child(
                card()
                    .child(if cfg!(target_os = "windows") {
                        "悬浮球与面板"
                    } else {
                        "菜单栏与面板"
                    })
                    .when(cfg!(target_os = "windows"), |d| {
                        d.child(
                            Button::new("always-on-top")
                                .ghost()
                                .label(format!(
                                    "{} 始终置顶",
                                    check(self.state.preferences.always_on_top)
                                ))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.state.preferences.always_on_top =
                                        !this.state.preferences.always_on_top;
                                    #[cfg(target_os = "windows")]
                                    {
                                        let _ = crate::windows_native::configure(
                                            window,
                                            false,
                                            this.state.preferences.always_on_top,
                                        );
                                    }
                                    #[cfg(not(target_os = "windows"))]
                                    let _ = window;
                                    this.save();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("snap-to-edge")
                                .ghost()
                                .label(format!(
                                    "{} 贴边收缩为进度条",
                                    check(self.state.preferences.snap_to_edge)
                                ))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.preferences.snap_to_edge =
                                        !this.state.preferences.snap_to_edge;
                                    this.save();
                                    cx.notify();
                                })),
                        )
                    })
                    .when(cfg!(target_os = "macos"), |d| {
                        d.child(
                            Button::new("glass-setting")
                                .ghost()
                                .label(format!(
                                    "{} 使用 Liquid Glass（macOS 26 及以上）",
                                    check(self.state.preferences.use_liquid_glass)
                                ))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.state.preferences.use_liquid_glass =
                                        !this.state.preferences.use_liquid_glass;
                                    platform::set_material(
                                        window,
                                        this.state.preferences.use_liquid_glass,
                                    );
                                    this.save();
                                    cx.notify();
                                })),
                        )
                    })
                    .when(cfg!(target_os = "macos"), |d| {
                        d.child(
                            Button::new("percentage")
                                .ghost()
                                .label(format!(
                                    "{} 在图标旁显示百分比或余额",
                                    check(self.state.preferences.show_percentage)
                                ))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.preferences.show_percentage =
                                        !this.state.preferences.show_percentage;
                                    this.save();
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("pin-setting")
                            .ghost()
                            .label(format!(
                                "{} 固定面板",
                                check(self.state.preferences.panel_pinned)
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.preferences.panel_pinned =
                                    !this.state.preferences.panel_pinned;
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("launch-at-login")
                            .ghost()
                            .label(format!("{} 登录后自动启动", check(self.launch_at_login)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                match platform::set_launch_at_login(!this.launch_at_login) {
                                    Ok(()) => {
                                        this.settings_note.clear();
                                        (this.launch_at_login, this.launch_approval) =
                                            platform::launch_status();
                                        this.state.preferences.start_with_windows =
                                            this.launch_at_login;
                                        this.save();
                                    }
                                    Err(e) => this.settings_note = e.to_string(),
                                }
                                cx.notify();
                            })),
                    )
                    .when(self.launch_approval, |d| {
                        d.child(
                            Button::new("launch-approval")
                                .label("在系统设置中允许登录项")
                                .on_click(|_, _, _| platform::open_login_settings()),
                        )
                    }),
            )
            .child(
                card()
                    .child("后台刷新间隔")
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children([30, 60, 120, 300].into_iter().map(|seconds| {
                                Button::new(("interval", seconds as usize))
                                    .small()
                                    .label(format!(
                                        "{}{} 秒",
                                        if self.state.preferences.refresh_seconds == seconds {
                                            "✓ "
                                        } else {
                                            ""
                                        },
                                        seconds
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.state.preferences.refresh_seconds = seconds;
                                        this.save();
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(label(
                        "展开面板时最多每 30 秒同步核心数据；公告约每 10 分钟检查一次。",
                    )),
            )
            .child(
                card()
                    .child("账号与连接")
                    .child(label(
                        "会话仅保存在本机系统凭据存储中。退出账号会清除本机会话。",
                    ))
                    .child(
                        Button::new("login-settings")
                            .primary()
                            .disabled(self.state.demo_only)
                            .label("连接账号")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.login_open = true;
                                cx.notify();
                            })),
                    )
                    .when(!self.state.needs_login, |d| {
                        d.child(Button::new("logout").danger().label("退出账号").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.confirm_sign_out = true;
                                cx.notify();
                            }),
                        ))
                    }),
            )
            .child(self.update_settings(cx))
            .child(label(format!(
                "KRILL DESKTOP · {}",
                updates::CURRENT_VERSION
            )))
            .child(
                Button::new("quit")
                    .ghost()
                    .label("退出程序")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
    fn welcome(&self, cx: &mut Context<Self>) -> Div {
        column()
            .pt_8()
            .child(div().text_size(px(60.)).text_color(rgb(MINT)).child("k"))
            .child(
                div()
                    .text_size(px(32.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("用量与余额，\n一眼就知道。"),
            )
            .child(label(
                "把 Krill 放在桌面一角。套餐剩余、账户余额和最新公告，随手可见。",
            ))
            .child(
                Button::new("connect")
                    .primary()
                    .label("连接 Krill 账号")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.login_open = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("demo")
                    .ghost()
                    .label("先看演示 →")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.use_demo();
                        this.sync_epoch();
                        cx.notify();
                    })),
            )
            .child(label("登录会话在本机加密保存"))
    }
    fn update_settings(&self, cx: &mut Context<Self>) -> Div {
        card()
            .child(format!("应用更新 · {}", updates::CURRENT_VERSION))
            .child(label(self.updater.status.clone()))
            .child(
                Button::new("check-update")
                    .label("检查更新")
                    .disabled(self.updater.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.check_update(cx))),
            )
            .when(self.updater.available.is_some(), |d| {
                d.child(
                    Button::new("install-update")
                        .primary()
                        .label("下载并重启更新")
                        .disabled(self.updater.busy)
                        .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
                )
            })
    }
    fn login(&self, cx: &mut Context<Self>) -> Div {
        column().child(heading("连接 Krill 账号"))
            .child(label("在默认浏览器中登录，支持官网任意登录方式及二次登录。"))
            .child(Button::new("browser").primary().label("在浏览器中登录 ↗").on_click(|_,_,cx|cx.open_url(LOGIN_URL)))
            .when(cfg!(target_os="windows"), |d|d.child(Button::new("webview-runtime").ghost().label("获取 WebView2 Runtime ↗").on_click(|_,_,cx|cx.open_url("https://developer.microsoft.com/microsoft-edge/webview2/"))))
            .child(Button::new("embedded-browser").ghost().label("改用应用内登录").disabled(self.state.demo_only).on_click(cx.listener(|this,_,_,cx|this.open_embedded(cx))))
            .child(label("1. 在浏览器中完成 Krill 登录。\n2. 在 Krill 官网打开开发者工具的控制台，运行下方按钮复制的脚本。\n3. 复制弹出的会话，粘贴到下面并连接。\n\n会话等同于登录凭据，请勿分享。"))
            .child(Button::new("export").label("复制会话导出脚本").on_click(|_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(EXPORT_SCRIPT.into()))))
            .child(Input::new(&self.input))
            .child(Button::new("import").primary().label(if self.login_busy{"验证中…"}else{"验证并连接"}).disabled(self.login_busy).on_click(cx.listener(|this,_,window,cx|this.import(window,cx))))
            .child(label(self.login_note.clone()))
            .child(Button::new("cancel-login").ghost().label("返回").on_click(cx.listener(|this,_,window,cx|{this.close_embedded(cx);this.login_open=false;this.login_busy=false;this.state.generation+=1;this.sync_epoch();this.state.is_loading=false;this.input.update(cx,|input,cx|input.set_value("",window,cx));cx.notify();})))
    }
}
impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.login_open {
            self.login(cx)
        } else if self.tab == 4 {
            self.settings(cx)
        } else if self.state.subscriptions.is_none() {
            self.welcome(cx)
        } else {
            match self.tab {
                0 => self.overview(cx),
                1 => self.plans(cx),
                2 => self.usage(cx),
                3 => column()
                    .child(heading("公告与动态"))
                    .child(label(format!(
                        "{} 条未读 · {}",
                        self.state.unread_count(),
                        self.state.announcement_note
                    )))
                    .children(
                        self.state
                            .announcements
                            .iter()
                            .map(|a| self.announcement(a, cx)),
                    ),
                _ => self.settings(cx),
            }
        };
        let mut body = div()
            .id("dashboard")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(
                if cfg!(target_os = "macos") && self.state.preferences.use_liquid_glass {
                    rgba(0x0e191fe8)
                } else {
                    rgb(BG)
                },
            )
            .text_color(rgb(0xe4edef))
            .text_size(px(13.))
            .font_family(if cfg!(target_os = "macos") {
                ".AppleSystemUIFont"
            } else {
                "Segoe UI"
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.state.panel_visible = false;
                    this.hovered = None;
                    platform::hide(window);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .h(px(76.))
                    .flex_none()
                    .px_6()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .when(cfg!(target_os = "windows"), |d| {
                                d.on_mouse_down(MouseButton::Left, |_, window, cx| {
                                    #[cfg(target_os = "windows")]
                                    if let Ok(hwnd) = crate::windows_native::hwnd(window) {
                                        let hwnd = hwnd.0 as isize;
                                        cx.foreground_executor()
                                            .spawn(async move {
                                                crate::windows_native::start_move(hwnd);
                                            })
                                            .detach();
                                    }
                                    #[cfg(not(target_os = "windows"))]
                                    let _ = (window, cx);
                                })
                            })
                            .child(
                                div()
                                    .size(px(34.))
                                    .bg(rgb(MINT))
                                    .rounded_lg()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .text_size(px(27.))
                                    .text_color(rgb(0x123b34))
                                    .child("k"),
                            )
                            .child(
                                div()
                                    .text_size(px(23.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("krill"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("refresh")
                                    .ghost()
                                    .label("↻")
                                    .disabled(self.state.is_loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                            )
                            .child(
                                Button::new("pin")
                                    .ghost()
                                    .label(if self.state.preferences.panel_pinned {
                                        "●"
                                    } else {
                                        "⌖"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.preferences.panel_pinned =
                                            !this.state.preferences.panel_pinned;
                                        this.save();
                                        cx.notify();
                                    })),
                            )
                            .child(Button::new("hide").ghost().label("×").on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.state.panel_visible = false;
                                    this.hovered = None;
                                    platform::hide(window);
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .child(
                div()
                    .mx_6()
                    .flex()
                    .flex_none()
                    .rounded_lg()
                    .bg(rgb(0x132028))
                    .p_1()
                    .children(TABS.into_iter().enumerate().map(|(i, title)| {
                        div()
                            .id(("tab", i))
                            .debug_selector(move || format!("tab-{i}"))
                            .flex_1()
                            .h(px(36.))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .justify_center()
                            .items_center()
                            .bg(rgb(if i == self.tab { 0x203b3c } else { 0x132028 }))
                            .text_color(rgb(if i == self.tab { MINT } else { MUTED }))
                            .child(title)
                            .on_click(cx.listener(move |this, _, _, cx| this.switch_tab(i, cx)))
                    })),
            )
            .when(self.state.is_demo, |d| {
                d.child(
                    div()
                        .mx_6()
                        .mt_3()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x302d26))
                        .text_color(rgb(0xe5c09d))
                        .text_size(px(11.))
                        .child("演示数据　虚构样例，不代表真实账号"),
                )
            })
            .when(self.updater.available.is_some() && !self.login_open, |d| {
                let version = self.updater.available.as_ref().unwrap().version.to_string();
                d.child(
                    div()
                        .mx_6()
                        .mt_3()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x203b3c))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(format!("新版本 {version} 可用"))
                        .child(
                            Button::new("update-banner")
                                .small()
                                .primary()
                                .label(if self.updater.busy {
                                    "准备更新中…"
                                } else {
                                    "下载并重启"
                                })
                                .disabled(self.updater.busy)
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
                        ),
                )
            })
            .when(self.state.is_stale, |d| {
                d.child(
                    div()
                        .mx_6()
                        .mt_3()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x302822))
                        .text_color(rgb(0xe5c09d))
                        .child(self.state.status_detail.clone()),
                )
            })
            .child(
                div()
                    .id("page-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px_6()
                    .py_4()
                    .child(content),
            )
            .child(
                div()
                    .h(px(68.))
                    .flex_none()
                    .mx_6()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(self.state.status_title())
                            .child(label(
                                self.state
                                    .updated_at
                                    .map(|d| format!("更新于 {}", core::date(Some(d))))
                                    .unwrap_or("等待首次同步".into()),
                            )),
                    )
                    .child(
                        Button::new("website")
                            .label("打开官网 ↗")
                            .on_click(|_, _, cx| cx.open_url(&format!("{ORIGIN}/app/account"))),
                    ),
            );
        if self.confirm_sign_out {
            body = body.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000bb))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        card()
                            .w(px(360.))
                            .child("退出账号？")
                            .child(label("本机会话与当前面板数据会被清除。"))
                            .child(
                                Button::new("confirm-logout")
                                    .danger()
                                    .label("退出账号")
                                    .on_click(cx.listener(|this, _, _, cx| this.sign_out(cx))),
                            )
                            .child(Button::new("cancel-logout").label("取消").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.confirm_sign_out = false;
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        }
        body
    }
}
fn check(v: bool) -> &'static str {
    if v { "✓" } else { "○" }
}
fn column() -> Div {
    div().flex().flex_col().gap_4()
}
fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(CARD))
}
fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
fn heading(text: &'static str) -> Div {
    div()
        .text_size(px(23.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}
fn row(title: String, value: String) -> Div {
    div()
        .flex()
        .justify_between()
        .gap_3()
        .child(label(title))
        .child(div().text_size(px(12.)).child(value))
}
fn metric(title: &'static str, value: String) -> Div {
    card().flex_1().min_w_0().p_3().child(label(title)).child(
        div()
            .text_size(px(23.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(value),
    )
}
fn progress(percent: Option<f64>) -> Div {
    div()
        .h(px(5.))
        .w_full()
        .rounded_full()
        .bg(rgb(0x253740))
        .child(
            div()
                .h_full()
                .w(relative(percent.unwrap_or(0.) as f32 / 100.))
                .rounded_full()
                .bg(rgb(MINT)),
        )
}
fn ring(percent: Option<f64>) -> Div {
    let text = percent.map(|p| format!("{p:.0}%")).unwrap_or("—".into());
    div()
        .size(px(98.))
        .flex_none()
        .relative()
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, window, _| {
                    let center = bounds.center();
                    let radius = 43.;
                    for (value, color) in [(100., 0x253740), (percent.unwrap_or(0.), MINT)] {
                        let mut path = PathBuilder::stroke(px(5.));
                        let steps = (value * 2.).round() as usize;
                        if steps == 0 {
                            continue;
                        }
                        for i in 0..=steps {
                            let angle = -std::f32::consts::FRAC_PI_2
                                + i as f32 / steps as f32 * value as f32 / 100.
                                    * std::f32::consts::TAU;
                            let p = point(
                                center.x + px(angle.cos() * radius),
                                center.y + px(angle.sin() * radius),
                            );
                            if i == 0 {
                                path.move_to(p);
                            } else {
                                path.line_to(p);
                            }
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(color));
                        }
                    }
                },
            )
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .gap_1()
                .child(div().text_size(px(22.)).text_color(rgb(MINT)).child(text))
                .child(label("周期剩余")),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    fn dashboard(
        cx: &mut TestAppContext,
    ) -> (tempfile::TempDir, Entity<Dashboard>, VisualTestContext) {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(true, Some(directory.path().join("demo"))).unwrap();
        let (window, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(520.), px(800.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Dashboard::new(storage, true, true, window, cx)),
            )
            .unwrap()
        });
        let visual = VisualTestContext::from_window(window, cx);
        visual.run_until_parked();
        (directory, view, visual)
    }
    #[gpui_kit::test]
    fn five_tabs_accept_pointer_input_across_the_whole_tab(cx: &mut TestAppContext) {
        let (_directory, view, mut cx) = dashboard(cx);
        for (selector, index) in [
            ("tab-1", 1),
            ("tab-2", 2),
            ("tab-3", 3),
            ("tab-4", 4),
            ("tab-0", 0),
        ] {
            let bounds = cx.debug_bounds(selector).expect("tab must be laid out");
            cx.simulate_click(bounds.origin + point(px(3.), px(3.)), Modifiers::default());
            assert_eq!(cx.read(|cx| view.read(cx).tab), index);
        }
    }
    #[gpui_kit::test]
    fn plot_hover_covers_whitespace_resets_on_leave_and_does_not_move(cx: &mut TestAppContext) {
        let (_directory, view, mut cx) = dashboard(cx);
        view.update(&mut cx, |view, cx| view.switch_tab(2, cx));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("usage-plot").unwrap();
        cx.simulate_mouse_move(
            bounds.origin + point(px(1.), px(1.)),
            None,
            Modifiers::default(),
        );
        assert!(cx.read(|cx| view.read(cx).hovered.is_some()));
        assert_eq!(cx.debug_bounds("usage-plot"), Some(bounds));
        cx.simulate_mouse_move(
            bounds.origin - point(px(1.), px(1.)),
            None,
            Modifiers::default(),
        );
        assert!(cx.read(|cx| view.read(cx).hovered.is_none()));
    }
    #[gpui_kit::test]
    fn masked_login_input_supports_paste_select_all_and_protects_copy(cx: &mut TestAppContext) {
        let (_directory, view, mut cx) = dashboard(cx);
        let input = view.update(&mut cx, |view, cx| {
            view.login_open = true;
            cx.notify();
            view.input.clone()
        });
        cx.run_until_parked();
        cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
        let command = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        for token in [
            "synthetic.header.signature",
            "replacement.payload.signature",
        ] {
            cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(token.into())));
            cx.simulate_keystrokes(&format!("{command}-a {command}-v"));
            assert_eq!(cx.read(|cx| input.read(cx).value().to_string()), token);
        }
        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
        cx.simulate_keystrokes(&format!("{command}-a {command}-c"));
        assert_eq!(
            cx.read(|cx| cx.read_from_clipboard().and_then(|c| c.text())),
            Some("sentinel".into())
        );
    }
}
