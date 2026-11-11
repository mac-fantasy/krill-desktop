use anyhow::Result;
use krill_core::state::DashboardState;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

#[derive(Clone, Copy, Debug)]
pub enum TrayAction {
    Toggle,
    Open,
    Refresh,
    CheckUpdate,
    Website,
    Quit,
    #[cfg(target_os = "windows")]
    ToggleOrb,
}
pub struct SystemTray {
    icon: TrayIcon,
    last: String,
    #[cfg(target_os = "windows")]
    menu: Menu,
}
impl SystemTray {
    pub fn new(handler: impl Fn(TrayAction) + Send + Sync + 'static) -> Result<Self> {
        let handler = std::sync::Arc::new(handler);
        let menu = Menu::new();
        let open = MenuItem::new("打开详情", true, None);
        let refresh = MenuItem::new("立即刷新", true, None);
        let update = MenuItem::new("检查更新", true, None);
        let website = MenuItem::new("打开官网", true, None);
        let quit = MenuItem::new("退出 Krill Desktop", true, None);
        menu.append_items(&[&open, &refresh, &update, &website, &quit])?;
        let entries = vec![
            (open.id().clone(), TrayAction::Open),
            (refresh.id().clone(), TrayAction::Refresh),
            (update.id().clone(), TrayAction::CheckUpdate),
            (website.id().clone(), TrayAction::Website),
            (quit.id().clone(), TrayAction::Quit),
        ];
        #[cfg(target_os = "windows")]
        let entries = {
            let mut entries = entries;
            let toggle = MenuItem::new("显示 / 隐藏悬浮球", true, None);
            menu.prepend(&toggle)?;
            entries.push((toggle.id().clone(), TrayAction::ToggleOrb));
            entries
        };
        let callback = handler.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, action)) = entries.iter().find(|(id, _)| *id == event.id) {
                callback(*action);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                handler(TrayAction::Toggle);
            }
        }));
        let icon = TrayIconBuilder::new()
            .with_tooltip("Krill Desktop")
            .with_icon(render_icon(None, true, false)?)
            .with_icon_as_template(cfg!(target_os = "macos"))
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()?;
        Ok(Self {
            icon,
            last: String::new(),
            #[cfg(target_os = "windows")]
            menu,
        })
    }
    #[cfg(target_os = "windows")]
    pub fn show_context(&self, window: &gpui_kit::Window, cx: &mut gpui_kit::App) {
        let _ = crate::windows_native::show_context(self.menu.clone(), window, cx);
    }
    pub fn rect(&self) -> Option<tray_icon::Rect> {
        let rect = self.icon.rect()?;
        #[cfg(target_os = "macos")]
        let mut rect = rect;
        if let Some(window) = self
            .icon
            .ns_status_item()
            .and_then(|s| s.button(objc2::MainThreadMarker::new()?))
            .and_then(|b| b.window())
        {
            let scale = window.backingScaleFactor();
            rect.position.x /= scale;
            rect.position.y /= scale;
            rect.size.width = (rect.size.width as f64 / scale).round() as u32;
            rect.size.height = (rect.size.height as f64 / scale).round() as u32;
        }
        Some(rect)
    }
    #[cfg(target_os = "macos")]
    pub fn status_window(&self) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
        self.icon
            .ns_status_item()?
            .button(objc2::MainThreadMarker::new()?)?
            .window()
    }
    pub fn update(&mut self, state: &DashboardState) -> Result<()> {
        let percent = state.quota().and_then(|q| q.percent());
        let title = if state.preferences.show_percentage {
            state.menu_text()
        } else {
            String::new()
        };
        let key = format!(
            "{percent:?}|{}|{}|{}|{title}",
            state.needs_login,
            state.is_stale,
            state.tooltip()
        );
        if self.last == key {
            return Ok(());
        }
        self.icon.set_icon(Some(render_icon(
            percent,
            state.needs_login,
            state.is_stale,
        )?))?;
        self.icon.set_tooltip(Some(state.tooltip()))?;
        self.icon.set_title(Some(title));
        self.last = key;
        Ok(())
    }
}

fn render_icon(percent: Option<f64>, needs_login: bool, stale: bool) -> Result<Icon> {
    let progress = if needs_login {
        0.
    } else {
        percent
            .filter(|v| v.is_finite())
            .unwrap_or(0.)
            .clamp(0., 100.)
    };
    let color = if cfg!(target_os = "macos") {
        "black"
    } else {
        "#7ce3cd"
    };
    let badge = if needs_login || stale {
        format!(
            r#"<circle cx="18" cy="19" r="2.8" fill="black"/><circle cx="18" cy="19" r="2" stroke="white" stroke-width="1" fill="{}"/>"#,
            if needs_login { "black" } else { "white" }
        )
    } else {
        String::new()
    };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="44" height="44" viewBox="0 0 22 22">
      <defs><mask id="mask"><rect width="22" height="22" fill="white"/>{badge}</mask></defs>
      <g fill="none" stroke="{color}" stroke-width="1.8" mask="url(#mask)">
        <circle cx="11" cy="11" r="9" opacity=".22"/>
        <circle cx="11" cy="11" r="9" stroke-dasharray="{} 56.549" transform="rotate(-90 11 11)" stroke-linecap="round"/>
      </g><g transform="translate(3.824 3.502) scale(.023)" fill="{color}">
      <path fill-rule="evenodd" d="M331 112H553C553 241 461 346 331 346Z M309 112C250 119 194 143 155 177L309 330Z M138 195C97 240 73 294 72 348H291Z M72 371C72 432 94 490 135 532L295 371Z M154 550C197 586 251 607 309 610V394Z M331 394L491 547C449 585 391 608 331 610Z M483 210A48 48 0 1 1 387 210A48 48 0 1 1 483 210Z"/>
      <path d="M550 116C548 73 522 42 482 42H132M548 116C532 91 505 78 473 78H211" fill="none" stroke="{color}" stroke-width="26" stroke-linecap="round"/>
      </g>{}</svg>"#,
        progress / 100. * 56.549,
        if needs_login {
            format!(
                r#"<circle cx="18" cy="19" r="2" fill="none" stroke="{color}" stroke-width="1"/>"#
            )
        } else if stale {
            format!(r#"<circle cx="18" cy="19" r="2" fill="{color}"/>"#)
        } else {
            String::new()
        }
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(44, 44)
        .ok_or_else(|| anyhow::anyhow!("Unable to allocate tray icon"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    // tiny-skia returns premultiplied channels; tray-icon expects straight RGBA.
    let mut rgba = pixmap.data().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        if alpha > 0 {
            for c in &mut pixel[..3] {
                *c = ((*c as u32 * 255) / alpha).min(255) as u8;
            }
        }
    }
    Ok(Icon::from_rgba(rgba, 44, 44)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn tray_icon_rasterizes_for_zero_unknown_full_and_login_states() {
        for (percent, login, stale) in [
            (None, true, false),
            (Some(0.), false, false),
            (Some(100.), false, false),
            (Some(50.), false, true),
        ] {
            assert!(render_icon(percent, login, stale).is_ok());
        }
    }
}
