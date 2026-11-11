use crate::{
    app::Dashboard,
    geometry::{self, Side},
    windows_native as native,
};
use gpui_kit::{prelude::*, *};
use std::time::Duration;

pub struct Orb {
    dashboard: WeakEntity<Dashboard>,
    side: Option<Side>,
    collapsed: bool,
    hovered: bool,
    dragging: bool,
    down: Option<(i32, i32)>,
    collapse_generation: u64,
    snap: bool,
    topmost: bool,
    window: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}
pub fn open(
    dashboard: &Entity<Dashboard>,
    cx: &mut App,
) -> anyhow::Result<(AnyWindowHandle, Entity<Orb>)> {
    let weak = dashboard.downgrade();
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(88.), px(88.)),
                cx,
            ))),
            titlebar: None,
            show: false,
            focus: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| Orb::new(weak, window, cx)),
    )?;
    Ok((handle.into(), handle.root(cx)?))
}
impl Orb {
    fn new(dashboard: WeakEntity<Dashboard>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root = dashboard
            .upgrade()
            .expect("dashboard owns the orb lifecycle");
        let prefs = root.read(cx).state.preferences.clone();
        let _ = native::configure(window, true, prefs.always_on_top);
        let _ = native::initialize_orb(window, prefs.orb_left_pixels, prefs.orb_top_pixels);
        let observer = cx.observe_in(&root, window, |this, root, window, cx| {
            let p = &root.read(cx).state.preferences;
            let (snap, topmost) = (p.snap_to_edge, p.always_on_top);
            if this.topmost != topmost {
                this.topmost = topmost;
                let _ = native::configure(window, true, topmost);
            }
            if this.snap != snap {
                this.snap = snap;
                if !snap {
                    this.expand(window);
                    this.side = None;
                    this.collapse_generation += 1;
                } else {
                    this.snap_to_edge(window, true);
                    this.schedule_collapse(cx);
                }
            }
            let _ = native::tooltip(window, &root.read(cx).state.tooltip());
            cx.notify();
        });
        let bounds = cx.observe_window_bounds(window, |this, window, cx| {
            if this.dragging {
                return;
            }
            if let Ok(r) = native::rect(window) {
                if let Ok(area) = native::work_area(r) {
                    let r2 = if let Some(side) = this.side {
                        geometry::docked(r, area, native::scale(window), side, this.collapsed)
                    } else {
                        r.clamp(area, 4)
                    };
                    if r != r2 {
                        let _ = native::move_resize(window, r2);
                    }
                }
            }
            cx.notify();
        });
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            crate::platform::hide(window);
            let _ = weak.update(cx, |this, _| {
                this.hovered = false;
                this.collapse_generation += 1;
            });
            false
        });
        let mut this = Self {
            dashboard,
            side: None,
            collapsed: false,
            hovered: false,
            dragging: false,
            down: None,
            collapse_generation: 0,
            snap: prefs.snap_to_edge,
            topmost: prefs.always_on_top,
            window: window.window_handle(),
            _subscriptions: vec![observer, bounds],
        };
        this.snap_to_edge(window, false);
        this.schedule_collapse(cx);
        let _ = native::show_without_activation(window);
        this
    }
    fn snap_to_edge(&mut self, window: &Window, force: bool) {
        if let Ok(r) = native::rect(window) {
            if let Ok(area) = native::work_area(r) {
                self.side = if self.snap {
                    geometry::dock_side(r, area, native::scale(window), force)
                } else {
                    None
                };
                let r2 = self
                    .side
                    .map(|side| geometry::docked(r, area, native::scale(window), side, false))
                    .unwrap_or_else(|| r.clamp(area, 4));
                let _ = native::move_resize(window, r2);
            }
        }
    }
    fn expand(&mut self, window: &Window) {
        self.collapse_generation += 1;
        if !self.collapsed {
            return;
        }
        self.collapsed = false;
        if let (Ok(r), Some(side)) = (native::rect(window), self.side) {
            if let Ok(area) = native::work_area(r) {
                let _ = native::move_resize(
                    window,
                    geometry::docked(r, area, native::scale(window), side, false),
                );
            }
        }
    }
    fn schedule_collapse(&mut self, cx: &mut Context<Self>) {
        self.collapse_generation += 1;
        let generation = self.collapse_generation;
        if !self.snap || self.side.is_none() || self.hovered || self.dragging {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(650))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.collapse_generation != generation
                    || this.hovered
                    || this.dragging
                    || !this.snap
                {
                    return;
                }
                if let Some(side) = this.side {
                    let _ = this.window.update(cx, |_, window, _| {
                        if let Ok(r) = native::rect(window) {
                            if let Ok(area) = native::work_area(r) {
                                if native::move_resize(
                                    window,
                                    geometry::docked(r, area, native::scale(window), side, true),
                                )
                                .is_ok()
                                {
                                    this.collapsed = true;
                                }
                            }
                        }
                    });
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn save_position(&self, r: geometry::Rect, scale: f64, cx: &mut Context<Self>) {
        let (x, y) = geometry::expanded_anchor(r, scale, self.side, self.collapsed);
        let _ = self.dashboard.update(cx, |root, _| {
            root.state.preferences.orb_left_pixels = Some(x);
            root.state.preferences.orb_top_pixels = Some(y);
            root.save();
        });
    }
    fn begin_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.down = None;
        self.dragging = true;
        self.expand(window);
        let Ok(hwnd) = native::hwnd(window) else {
            self.dragging = false;
            return;
        };
        let hwnd = hwnd.0 as isize;
        cx.spawn(async move |this, cx| {
            // Win32's modal move loop must not retain an App/Entity borrow.
            native::start_move(hwnd);
            let _ = this.update(cx, |this, cx| {
                let result = this.window.update(cx, |_, window, _| {
                    this.dragging = false;
                    this.snap_to_edge(window, true);
                    native::rect(window).map(|r| (r, native::scale(window)))
                });
                if let Ok(Ok((r, scale))) = result {
                    this.hovered = native::cursor().is_some_and(|(x, y)| {
                        x >= r.left && x < r.right() && y >= r.top && y < r.bottom()
                    });
                    this.save_position(r, scale, cx);
                }
                this.schedule_collapse(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn verify_native(&mut self, cx: &mut Context<Self>) -> serde_json::Value {
        self.collapse_generation += 1;
        let result = self
            .window
            .update(cx, |_, window, _| -> anyhow::Result<bool> {
                let original = native::rect(window)?;
                let area = native::work_area(original)?;
                let scale = native::scale(window);
                let mut success = native::valid_orb_style(window);
                let foreground =
                    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
                for side in [Side::Left, Side::Right] {
                    let expanded = geometry::docked(original, area, scale, side, false);
                    native::move_resize(window, expanded)?;
                    let collapsed = geometry::docked(expanded, area, scale, side, true);
                    native::move_resize(window, collapsed)?;
                    let actual = native::rect(window)?;
                    success &= actual.width == (16. * scale).round() as i32
                        && geometry::expanded_anchor(actual, scale, Some(side), true)
                            == (expanded.left, expanded.top);
                    native::move_resize(window, expanded)?;
                    success &= native::rect(window)? == expanded;
                }
                native::move_resize(window, original)?;
                success &= foreground
                    == unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
                Ok(success)
            });
        serde_json::json!({"success":matches!(result,Ok(Ok(true))),"checks":["no_activate_tool_window","left_and_right_collapse_expand","preserved_expanded_anchor","resize_does_not_steal_focus"]})
    }
}
impl Render for Orb {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self
            .dashboard
            .upgrade()
            .map(|root| root.read(cx).state.clone());
        let percent = state
            .as_ref()
            .and_then(|s| s.quota())
            .and_then(|q| q.percent())
            .unwrap_or(0.);
        let color = if state.as_ref().is_some_and(|s| s.is_stale || s.needs_login) {
            0x8fa4ad
        } else if percent < 20. {
            0xf3bc73
        } else {
            0x7ce3cd
        };
        let text = state.as_ref().map(|s| s.menu_text()).unwrap_or("—".into());
        let caption = state
            .as_ref()
            .map(|s| {
                if s.is_stale {
                    "离线"
                } else if s.needs_login {
                    "连接账号"
                } else if s.selected().is_some() {
                    "周期剩余"
                } else {
                    "充值余额"
                }
            })
            .unwrap_or("Krill");
        let visual = if self.collapsed {
            div().size_full().py(px(3.)).child(
                div()
                    .size_full()
                    .p(px(4.))
                    .border_1()
                    .border_color(rgb(0x3a505c))
                    .rounded(px(5.))
                    .bg(rgb(0x101e26))
                    .child(
                        div()
                            .size_full()
                            .relative()
                            .rounded_full()
                            .bg(rgb(0x263b44))
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .w_full()
                                    .h(relative(percent as f32 / 100.))
                                    .rounded_full()
                                    .bg(rgb(color)),
                            ),
                    ),
            )
        } else {
            div().size_full().p(px(5.)).child(
                div()
                    .size_full()
                    .relative()
                    .rounded_full()
                    .bg(rgb(0x101e26))
                    .border_1()
                    .border_color(rgb(0x3a505c))
                    .child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |bounds, _, window, _| {
                                let center = bounds.center();
                                for (value, stroke) in [(100., 0x263b44), (percent, color)] {
                                    let mut path = PathBuilder::stroke(px(3.));
                                    let steps = (value * 2.).round() as usize;
                                    if steps == 0 {
                                        continue;
                                    }
                                    for i in 0..=steps {
                                        let a = -std::f32::consts::FRAC_PI_2
                                            + i as f32 / steps as f32 * value as f32 / 100.
                                                * std::f32::consts::TAU;
                                        let p = point(
                                            center.x + px(a.cos() * 32.),
                                            center.y + px(a.sin() * 32.),
                                        );
                                        if i == 0 {
                                            path.move_to(p);
                                        } else {
                                            path.line_to(p);
                                        }
                                    }
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, rgb(stroke));
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
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(color))
                                    .child(text),
                            )
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(0x9bb3bf))
                                    .child(caption),
                            ),
                    )
                    .when(state.as_ref().is_some_and(|s| s.is_stale), |d| {
                        d.child(
                            div()
                                .absolute()
                                .right(px(7.))
                                .top(px(8.))
                                .size(px(7.))
                                .rounded_full()
                                .bg(rgb(0xf3bc73)),
                        )
                    }),
            )
        };
        div()
            .id("orb")
            .debug_selector(|| "orb".into())
            .size_full()
            .cursor_pointer()
            .tooltip({
                let tooltip = state.as_ref().map(|s| s.tooltip()).unwrap_or_default();
                move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
                }
            })
            .child(visual)
            .on_hover(cx.listener(|this, hover: &bool, window, cx| {
                this.hovered = *hover;
                if *hover {
                    this.expand(window);
                } else {
                    this.schedule_collapse(cx);
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.expand(window);
                    this.down = native::cursor();
                    native::capture(window);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let Some(down) = this.down else {
                    return;
                };
                let Some(current) = native::cursor() else {
                    return;
                };
                if ((current.0 - down.0).abs() as f64).max((current.1 - down.1).abs() as f64)
                    < 4. * native::scale(window)
                {
                    return;
                }
                this.begin_drag(window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    native::release_capture();
                    if this.down.take().is_some() {
                        let _ = this.dashboard.update(cx, |root, cx| root.toggle_panel(cx));
                    }
                    this.schedule_collapse(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| {
                    let _ = this
                        .dashboard
                        .update(cx, |root, cx| root.show_context(window, cx));
                }),
            )
    }
}
