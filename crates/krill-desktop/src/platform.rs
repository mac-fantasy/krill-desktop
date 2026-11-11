use anyhow::Result;
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "macos")]
fn native_window(window: &Window) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    if cfg!(test) {
        return None;
    }
    let handle = HasWindowHandle::window_handle(window).ok()?;
    if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
        // GPUI owns the view for at least the lifetime of this borrowed Window.
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        view.window()
    } else {
        None
    }
}
pub fn accessory() {
    #[cfg(target_os = "macos")]
    if let Some(mtm) = objc2::MainThreadMarker::new() {
        objc2_app_kit::NSApplication::sharedApplication(mtm)
            .setActivationPolicy(objc2_app_kit::NSApplicationActivationPolicy::Accessory);
    }
}
pub fn show(window: &Window) {
    #[cfg(target_os = "macos")]
    if let Some(w) = native_window(window) {
        w.makeKeyAndOrderFront(None);
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    windows::Win32::Foundation::HWND(h.hwnd.get() as *mut _),
                    windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
                );
            }
        }
    }
}
pub fn hide(window: &Window) {
    #[cfg(target_os = "macos")]
    if let Some(w) = native_window(window) {
        w.orderOut(None);
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    windows::Win32::Foundation::HWND(h.hwnd.get() as *mut _),
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
        }
    }
}
pub fn configure_panel(window: &Window) {
    #[cfg(target_os = "macos")]
    if let Some(w) = native_window(window) {
        w.setLevel(3);
        w.setCollectionBehavior(objc2_app_kit::NSWindowCollectionBehavior::MoveToActiveSpace);
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            unsafe {
                use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
                let hwnd = HWND(h.hwnd.get() as *mut _);
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    (style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize),
                );
            }
        }
    }
}
pub fn set_material(window: &Window, enabled: bool) {
    if cfg!(test) {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        use gpui_kit::WindowBackgroundAppearance;
        use objc2::MainThreadOnly;
        use objc2_app_kit::{
            NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle,
            NSWindowOrderingMode,
        };
        let major = objc2_foundation::NSProcessInfo::processInfo()
            .operatingSystemVersion()
            .majorVersion;
        if major < 26 {
            window.set_background_appearance(if enabled {
                WindowBackgroundAppearance::Blurred
            } else {
                WindowBackgroundAppearance::Opaque
            });
            return;
        }
        window.set_background_appearance(if enabled {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        });
        if let Some(content) = native_window(window).and_then(|w| w.contentView()) {
            for child in content.subviews() {
                if child.downcast_ref::<NSGlassEffectView>().is_some() {
                    child.removeFromSuperview();
                }
            }
            if enabled {
                if let Some(mtm) = objc2::MainThreadMarker::new() {
                    let glass = NSGlassEffectView::initWithFrame(
                        NSGlassEffectView::alloc(mtm),
                        content.bounds(),
                    );
                    glass.setStyle(NSGlassEffectViewStyle::Regular);
                    glass.setCornerRadius(18.);
                    glass.setAutoresizingMask(
                        NSAutoresizingMaskOptions::ViewWidthSizable
                            | NSAutoresizingMaskOptions::ViewHeightSizable,
                    );
                    content.addSubview_positioned_relativeTo(
                        &glass,
                        NSWindowOrderingMode::Below,
                        None,
                    );
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, enabled);
}
pub fn place_panel(window: &Window, rect: Option<tray_icon::Rect>) {
    #[cfg(target_os = "macos")]
    if let (Some(w), Some(rect)) = (native_window(window), rect) {
        if let Some(screen) = w.screen() {
            let screen_frame = screen.frame();
            let visible = screen.visibleFrame();
            let frame = w.frame();
            // tray-icon's screen position has a top-left origin, AppKit uses bottom-left.
            let x = (rect.position.x + rect.size.width as f64 / 2. - frame.size.width / 2.).clamp(
                visible.origin.x + 8.,
                (visible.origin.x + visible.size.width - frame.size.width - 8.)
                    .max(visible.origin.x + 8.),
            );
            let y = (screen_frame.origin.y + screen_frame.size.height
                - rect.position.y
                - rect.size.height as f64
                - frame.size.height
                - 8.)
                .clamp(
                    visible.origin.y + 8.,
                    (visible.origin.y + visible.size.height - frame.size.height - 8.)
                        .max(visible.origin.y + 8.),
                );
            w.setFrameOrigin(objc2_foundation::NSPoint::new(x, y));
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(rect) = rect {
        let _ = crate::windows_native::place_panel(
            window,
            crate::geometry::Rect {
                left: rect.position.x as i32,
                top: rect.position.y as i32,
                width: rect.size.width as i32,
                height: rect.size.height as i32,
            },
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (window, rect);
}
pub fn launch_status() -> (bool, bool) {
    if cfg!(test) {
        return (false, false);
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_service_management::{SMAppService, SMAppServiceStatus};
        let status = unsafe { SMAppService::mainAppService().status() };
        return (
            status == SMAppServiceStatus::Enabled,
            status == SMAppServiceStatus::RequiresApproval,
        );
    }
    #[cfg(target_os = "windows")]
    {
        return (crate::windows_native::startup_enabled(), false);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    (false, false)
}
pub fn set_launch_at_login(enabled: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let service = unsafe { objc2_service_management::SMAppService::mainAppService() };
        let result = if enabled {
            unsafe { service.registerAndReturnError() }
        } else {
            unsafe { service.unregisterAndReturnError() }
        };
        result.map_err(|_| {
            anyhow::anyhow!("无法修改登录项，请从固定的应用程序目录运行打包后的应用。")
        })?;
    }
    #[cfg(target_os = "windows")]
    {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let mut command = std::process::Command::new("reg.exe");
        if enabled {
            command
                .args(["add", key, "/v", "KrillDesktop", "/t", "REG_SZ", "/d"])
                .arg(format!(
                    "\"{}\" --background",
                    std::env::current_exe()?.display()
                ))
                .arg("/f");
        } else {
            command.args(["delete", key, "/v", "KrillDesktop", "/f"]);
        }
        anyhow::ensure!(command.status()?.success(), "无法修改开机启动设置。");
    }
    Ok(())
}
pub fn open_login_settings() {
    #[cfg(target_os = "macos")]
    unsafe {
        objc2_service_management::SMAppService::openSystemSettingsLoginItems();
    }
}
