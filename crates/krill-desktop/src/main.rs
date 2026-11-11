#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
extern crate gpui_kit as gpui;
mod app;
#[cfg(any(target_os = "windows", test))]
mod geometry;
#[cfg(target_os = "windows")]
mod orb;
mod platform;
mod smoke;
mod storage;
mod tray;
mod updates;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod windows_native;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
gpui_kit::actions!(krill, [Quit]);

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let smoke = args.iter().any(|s| s == "--smoke-test");
    let demo = smoke || args.iter().any(|s| s == "--demo");
    let output = args
        .windows(2)
        .find(|s| s[0] == "--output")
        .map(|s| std::path::PathBuf::from(&s[1]))
        .unwrap_or_else(|| "artifacts/rust-native".into());
    let success = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let smoke_success = success.clone();
    let directory = args
        .windows(2)
        .find(|s| s[0] == "--data-dir")
        .map(|s| std::path::PathBuf::from(&s[1]));
    let storage = storage::Storage::new(demo, directory)?;
    let show = smoke || args.iter().any(|s| s == "--show");
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-q"
            } else {
                "ctrl-q"
            },
            Quit,
            None,
        )]);
        cx.set_menus([
            Menu::new("Krill Desktop").items([MenuItem::action("退出 Krill Desktop", Quit)]),
            Menu::new("编辑").items([
                MenuItem::action("撤销", base::input::Undo),
                MenuItem::action("重做", base::input::Redo),
                MenuItem::action("剪切", base::input::Cut),
                MenuItem::action("复制", base::input::Copy),
                MenuItem::action("粘贴", base::input::Paste),
                MenuItem::action("全选", base::input::SelectAll),
            ]),
        ]);
        platform::accessory();
        let bounds = Bounds::centered(None, size(px(520.), px(800.)), cx);
        let (window, dashboard) = open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                show,
                focus: show,
                window_min_size: Some(size(px(430.), px(500.))),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| app::Dashboard::new(storage, demo, show, window, cx)),
        )
        .expect("Unable to create Krill window");
        #[cfg(target_os = "windows")]
        match orb::open(&dashboard, cx) {
            Ok((handle, orb)) => dashboard.update(cx, |view, _| {
                view.orb_window = Some(handle);
                view.orb = Some(orb);
                view.orb_visible = true;
            }),
            Err(_) => dashboard.update(cx, |view, cx| {
                view.state.status_detail = "悬浮球暂时无法启动，已打开详情面板。".into();
                view.show_panel(cx);
            }),
        }
        if smoke {
            smoke::run(window, dashboard, output, smoke_success, cx);
        }
    });
    anyhow::ensure!(
        !smoke || success.load(std::sync::atomic::Ordering::Acquire),
        "Native UI smoke verification failed."
    );
    Ok(())
}
