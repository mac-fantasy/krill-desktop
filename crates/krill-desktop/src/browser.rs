use crate::browser_window::PopupWindow;
use anyhow::Result;
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{prelude::*, *};
use krill_core::auth::{self, LOGIN_URL, SESSION_SCRIPT, Session};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use wry::{NewWindowFeatures, NewWindowResponse, WebView, WebViewBuilder};

enum BrowserEvent {
    Extracted(u64, String),
    Close(u64),
    Blocked,
    Loaded(u64),
}
struct Popup {
    view: WebView,
    window: PopupWindow,
    id: u64,
}
impl Drop for Popup {
    fn drop(&mut self) {
        let _ = self.view.load_url("about:blank");
    }
}
struct SessionWindows {
    popups: RefCell<Vec<Popup>>,
    alive: Cell<bool>,
    next_id: Cell<u64>,
    sender: async_channel::Sender<BrowserEvent>,
}
impl Drop for SessionWindows {
    fn drop(&mut self) {
        self.alive.set(false);
        self.popups.get_mut().clear();
    }
}
pub struct BrowserLogin {
    webview: Option<WebView>,
    windows: Rc<SessionWindows>,
    window: AnyWindowHandle,
    note: String,
    on_candidate: Rc<dyn Fn(Session)>,
    on_close: Rc<dyn Fn()>,
    last_candidate: Option<String>,
    loaded: Vec<u64>,
    _subscriptions: Vec<Subscription>,
}
impl BrowserLogin {
    pub fn open(
        cx: &mut App,
        on_candidate: impl Fn(Session) + 'static,
        on_close: impl Fn() + 'static,
    ) -> Result<(AnyWindowHandle, Entity<Self>)> {
        Self::open_mode(cx, on_candidate, on_close, false)
    }
    fn open_mode(
        cx: &mut App,
        on_candidate: impl Fn(Session) + 'static,
        on_close: impl Fn() + 'static,
        smoke: bool,
    ) -> Result<(AnyWindowHandle, Entity<Self>)> {
        let on_candidate: Rc<dyn Fn(Session)> = Rc::new(on_candidate);
        let on_close: Rc<dyn Fn()> = Rc::new(on_close);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.), px(720.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("连接 Krill 账号 · 应用内登录".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(700.), px(620.))),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| Self::new(window, cx, on_candidate, on_close, smoke)),
        )
    }
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        on_candidate: Rc<dyn Fn(Session)>,
        on_close: Rc<dyn Fn()>,
        smoke: bool,
    ) -> Self {
        let (sender, receiver) = async_channel::unbounded();
        let windows = Rc::new(SessionWindows {
            popups: RefCell::new(Vec::new()),
            alive: Cell::new(true),
            next_id: Cell::new(1),
            sender,
        });
        let browser = build(windows.clone(), 0, None, smoke)
            .and_then(|builder| Ok(if smoke{builder.with_html("<!doctype html><html><body>Krill OAuth regression<script>window.received=null;window.addEventListener('message',event=>window.received=event.data);</script></body></html>")}else{builder.with_url(LOGIN_URL)}.build_as_child(&*window)?));
        let (webview, note) = match browser {
            Ok(view) => (
                Some(view),
                "请选择官网任意登录方式；若服务商限制应用内登录，可改用浏览器登录。".into(),
            ),
            Err(error) => (
                None,
                format!(
                    "无法启动应用内浏览器，请使用系统浏览器登录。Windows 需要 WebView2 Runtime。（{error}）"
                ),
            ),
        };
        let bounds = cx.observe_window_bounds(window, |this, window, _| this.resize(window));
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = weak.update(cx, |this, _| this.close());
            true
        });
        cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                if this.update(cx, |this, cx| this.event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                if this.update(cx, |this, _| this.poll()).is_err() {
                    break;
                }
            }
        })
        .detach();
        let this = Self {
            webview,
            windows,
            window: window.window_handle(),
            note,
            on_candidate,
            on_close,
            last_candidate: None,
            loaded: Vec::new(),
            _subscriptions: vec![bounds],
        };
        this.resize(window);
        this
    }
    fn resize(&self, window: &Window) {
        let size = window.bounds().size;
        if let Some(view) = &self.webview {
            let _ = view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0., 74.).into(),
                size: wry::dpi::LogicalSize::new(
                    f64::from(size.width),
                    f64::from(size.height).max(74.) - 74.,
                )
                .into(),
            });
        }
    }
    fn poll(&mut self) {
        if !self.windows.alive.get() {
            return;
        }
        if let Some(view) = &self.webview {
            extract(view, 0, &self.windows.sender);
        }
        self.windows
            .popups
            .borrow_mut()
            .retain(|p| p.window.is_open());
        for popup in self.windows.popups.borrow().iter() {
            let (width, height) = popup.window.size();
            let _ = popup.view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0., 0.).into(),
                size: wry::dpi::LogicalSize::new(width, height).into(),
            });
            extract(&popup.view, popup.id, &self.windows.sender);
        }
    }
    fn event(&mut self, event: BrowserEvent, cx: &mut Context<Self>) {
        if !self.windows.alive.get() {
            return;
        }
        match event {
            BrowserEvent::Extracted(id, json) => {
                let current = if id == 0 {
                    self.webview.as_ref().and_then(|v| v.url().ok())
                } else {
                    self.windows
                        .popups
                        .borrow()
                        .iter()
                        .find(|p| p.id == id)
                        .and_then(|p| p.view.url().ok())
                };
                if let (Some(page), Ok(value)) =
                    (current, serde_json::from_str::<serde_json::Value>(&json))
                {
                    if let (Some(origin), Some(token)) =
                        (value["origin"].as_str(), value["token"].as_str())
                    {
                        if let Some(session) = auth::extracted_session(&page, origin, token) {
                            // Suppress repeated API validation of the same rejected candidate.
                            use sha2::{Digest, Sha256};
                            let digest =
                                format!("{:x}", Sha256::digest(session.expose().as_bytes()));
                            if self.last_candidate.as_ref() != Some(&digest) {
                                self.last_candidate = Some(digest);
                                self.note = "已读取 Krill 会话，正在验证身份…".into();
                                (self.on_candidate)(session);
                                cx.notify();
                            }
                        }
                    }
                }
            }
            BrowserEvent::Close(0) => {
                self.close();
                let _ = self
                    .window
                    .update(cx, |_, window, _| window.remove_window());
            }
            BrowserEvent::Close(id) => self.windows.popups.borrow_mut().retain(|p| p.id != id),
            BrowserEvent::Blocked => {
                self.note = "已阻止非 HTTPS 页面跳转；可改用系统浏览器登录。".into();
                cx.notify();
            }
            BrowserEvent::Loaded(id) => {
                if !self.loaded.contains(&id) {
                    self.loaded.push(id);
                }
            }
        }
    }
    pub fn close(&mut self) {
        if !self.windows.alive.replace(false) {
            return;
        }
        self.windows.popups.borrow_mut().clear();
        self.webview.take();
        (self.on_close)();
    }
    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.note = note;
        cx.notify();
    }
    pub async fn verify(cx: &mut AsyncApp) -> Result<serde_json::Value> {
        let (window, browser) = cx.update(|cx| Self::open_mode(cx, |_| {}, || {}, true))?;
        let result=async {
            let ready=browser.update(cx,|browser,_|browser.webview.is_some());
            anyhow::ensure!(ready,"{}",browser.update(cx,|browser,_|browser.note.clone()));
            for _ in 0..40 {
                if Self::evaluate(&browser,0,"document.readyState",cx).await?=="\"complete\""{break;}
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            let opened=Self::evaluate(&browser,0,"window.child=window.open('about:blank'); !!window.child",cx).await?;
            anyhow::ensure!(opened=="true","OAuth popup did not open");
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let popup=browser.update(cx,|browser,_|browser.windows.popups.borrow().first().map(|p|p.id)).ok_or_else(||anyhow::anyhow!("OAuth popup was not retained"))?;
            anyhow::ensure!(Self::evaluate(&browser,popup,"!!window.opener",cx).await?=="true","OAuth popup lost opener");
            let private=browser.update(cx,|browser,_|{
                #[cfg(target_os="macos")]
                {use wry::WebViewExtMacOS;let main=browser.webview.as_ref().unwrap().webview();let popups=browser.windows.popups.borrow();let child=popups[0].view.webview();
                    unsafe{let a=main.configuration().websiteDataStore();let b=child.configuration().websiteDataStore();!a.isPersistent() && objc2::rc::Retained::as_ptr(&a)==objc2::rc::Retained::as_ptr(&b)}}
                #[cfg(target_os="windows")]
                {
                    use wry::WebViewExtWindows;
                    use windows_core::Interface;
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_13;
                    let popups=browser.windows.popups.borrow();
                    let profiles=(|| -> windows_core::Result<_> {unsafe {
                        let main=browser.webview.as_ref().unwrap().webview().cast::<ICoreWebView2_13>()?.Profile()?;
                        let child=popups[0].view.webview().cast::<ICoreWebView2_13>()?.Profile()?;
                        let mut a=Default::default(); let mut b=Default::default();
                        main.IsInPrivateModeEnabled(&mut a)?;
                        child.IsInPrivateModeEnabled(&mut b)?;
                        Ok(a.as_bool() && b.as_bool() && browser.webview.as_ref().unwrap().environment()==popups[0].view.environment())
                    }})();
                    profiles.unwrap_or(false)
                }
            });
            anyhow::ensure!(private,"OAuth popup did not share a private data store");
            Self::evaluate(&browser,popup,"window.opener.postMessage('callback received','*'); 'sent'",cx).await?;
            let mut received=false;for _ in 0..30 {
                if Self::evaluate(&browser,0,"window.received",cx).await?=="\"callback received\""{received=true;break;}
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            anyhow::ensure!(received,"OAuth popup callback did not reach opener");
            Self::evaluate(&browser,0,"window.child.close(); 'closed'",cx).await?;
            cx.background_executor().timer(Duration::from_millis(400)).await;
            anyhow::ensure!(browser.update(cx,|browser,_|browser.windows.popups.borrow().is_empty()),"Script did not close the OAuth popup independently");
            Self::evaluate(&browser,0,"window.child=window.open('about:blank'); !!window.child",cx).await?;
            cx.background_executor().timer(Duration::from_millis(200)).await;
            browser.update(cx,|browser,_|browser.close());
            anyhow::ensure!(browser.update(cx,|browser,_|browser.windows.popups.borrow().is_empty() && browser.webview.is_none()),"Closing login did not dispose child webviews");
            Ok(serde_json::json!({"success":true,"checks":["native_child_popup","window_opener","private_shared_session","post_message_callback","independent_script_close","parent_disposes_popups"]}))
        }.await;
        browser.update(cx, |browser, _| browser.close());
        let _ = window.update(cx, |_, window, _| window.remove_window());
        result
    }
    async fn evaluate(
        browser: &Entity<Self>,
        id: u64,
        script: &str,
        cx: &mut AsyncApp,
    ) -> Result<String> {
        // Wry queues scripts before didFinishNavigation but discards their
        // callbacks; wait for the load event before requesting a result.
        let mut loaded = false;
        for _ in 0..300 {
            if browser.update(cx, |browser, _| browser.loaded.contains(&id)) {
                loaded = true;
                break;
            }
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
        }
        anyhow::ensure!(
            loaded,
            "Synthetic WebView page did not finish loading: {id}"
        );
        let (sender, receiver) = async_channel::bounded(1);
        browser.update(cx, |browser, _| -> Result<()> {
            let callback = move |json| {
                let _ = sender.try_send(json);
            };
            if id == 0 {
                browser
                    .webview
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Main webview missing"))?
                    .evaluate_script_with_callback(script, callback)?;
            } else {
                browser
                    .windows
                    .popups
                    .borrow()
                    .iter()
                    .find(|p| p.id == id)
                    .ok_or_else(|| anyhow::anyhow!("Popup missing"))?
                    .view
                    .evaluate_script_with_callback(script, callback)?;
            }
            Ok(())
        })?;
        for _ in 0..300 {
            if let Ok(value) = receiver.try_recv() {
                return Ok(value);
            }
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
        }
        anyhow::bail!("WebView evaluation did not complete for synthetic check: {id} / {script}")
    }
}
impl Drop for BrowserLogin {
    fn drop(&mut self) {
        self.close();
    }
}
impl Render for BrowserLogin {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x0e191f))
            .text_color(rgb(0xe4edef))
            .child(
                div()
                    .h(px(74.))
                    .px_4()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child("连接 Krill 账号")
                            .child(
                                Button::new("system-browser")
                                    .small()
                                    .ghost()
                                    .label("改用系统浏览器登录")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close();
                                        window.remove_window();
                                        cx.open_url(LOGIN_URL);
                                    })),
                            ),
                    )
                    .child(div().text_size(px(11.)).child(self.note.clone())),
            )
    }
}
fn extract(view: &WebView, id: u64, sender: &async_channel::Sender<BrowserEvent>) {
    if !view
        .url()
        .ok()
        .is_some_and(|url| auth::trusted_origin(&url))
    {
        return;
    }
    let sender = sender.clone();
    let _ = view.evaluate_script_with_callback(SESSION_SCRIPT, move |json| {
        if json.len() <= 20000 {
            let _ = sender.try_send(BrowserEvent::Extracted(id, json));
        }
    });
}
fn build(
    session: Rc<SessionWindows>,
    id: u64,
    features: Option<NewWindowFeatures>,
    smoke: bool,
) -> Result<WebViewBuilder<'static>> {
    let navigation = session.sender.clone();
    let close = session.sender.clone();
    let loaded = session.sender.clone();
    let weak = Rc::downgrade(&session);
    let mut builder = WebViewBuilder::new()
        .with_incognito(true)
        .with_devtools(false)
        .with_clipboard(true)
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, wry::PageLoadEvent::Finished) {
                let _ = loaded.try_send(BrowserEvent::Loaded(id));
            }
        })
        .with_navigation_handler(move |url| {
            let allowed =
                auth::allows_navigation(&url) || ((id != 0 || smoke) && url == "about:blank");
            if !allowed {
                let _ = navigation.try_send(BrowserEvent::Blocked);
            }
            allowed
        })
        .with_initialization_script(
            "window.close = function () { window.ipc.postMessage('__krill_close_window__'); };",
        )
        .with_ipc_handler(move |request| {
            if request.body() == "__krill_close_window__" {
                let _ = close.try_send(BrowserEvent::Close(id));
            }
        })
        .with_new_window_req_handler(move |url, features| {
            let Some(session) = weak.upgrade().filter(|s| s.alive.get()) else {
                return NewWindowResponse::Deny;
            };
            session.popups.borrow_mut().retain(|p| p.window.is_open());
            if session.popups.borrow().len() >= 5
                || !(url == "about:blank" || auth::allows_navigation(&url))
            {
                return NewWindowResponse::Deny;
            }
            let id = session.next_id.get();
            session.next_id.set(id + 1);
            let result = (|| -> Result<_> {
                let window = PopupWindow::new()?;
                let view =
                    build(session.clone(), id, Some(features), false)?.build_as_child(&window)?;
                #[cfg(target_os = "macos")]
                let response = {
                    use wry::WebViewExtMacOS;
                    NewWindowResponse::Create {
                        webview: view.webview().into_super(),
                    }
                };
                #[cfg(target_os = "windows")]
                let response = {
                    use wry::WebViewExtWindows;
                    NewWindowResponse::Create {
                        webview: view.webview(),
                    }
                };
                window.show();
                session.popups.borrow_mut().push(Popup { view, window, id });
                Ok(response)
            })();
            result.unwrap_or(NewWindowResponse::Deny)
        });
    if let Some(features) = features {
        #[cfg(target_os = "macos")]
        {
            use wry::WebViewBuilderExtMacos;
            // Keep WebKit's exact target configuration and private data store,
            // but give this view its own IPC controller. Reusing the opener's
            // controller routes child window.close() messages to the parent.
            let mtm = objc2::MainThreadMarker::new()
                .ok_or_else(|| anyhow::anyhow!("Popup requires main thread"))?;
            let controller = unsafe { objc2_web_kit::WKUserContentController::new(mtm) };
            unsafe {
                features
                    .opener
                    .target_configuration
                    .setUserContentController(&controller);
            }
            builder = builder.with_webview_configuration(features.opener.target_configuration);
        }
        #[cfg(target_os = "windows")]
        {
            use wry::WebViewBuilderExtWindows;
            builder = builder.with_environment(features.opener.environment);
        }
    }
    Ok(builder)
}
