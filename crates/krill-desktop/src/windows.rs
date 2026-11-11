use crate::geometry::{self, Rect};
use anyhow::Result;
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect},
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

pub fn hwnd(window: &Window) -> Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)?;
    if let RawWindowHandle::Win32(h) = handle.as_raw() {
        Ok(HWND(h.hwnd.get() as *mut _))
    } else {
        anyhow::bail!("Window is not Win32")
    }
}
pub fn rect(window: &Window) -> Result<Rect> {
    let mut r = RECT::default();
    unsafe {
        GetWindowRect(hwnd(window)?, &mut r)?;
    }
    Ok(Rect {
        left: r.left,
        top: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
    })
}
pub fn scale(window: &Window) -> f64 {
    hwnd(window)
        .map(|h| unsafe { GetDpiForWindow(h) }.max(96) as f64 / 96.)
        .unwrap_or(1.)
}
pub fn work_area(anchor: Rect) -> Result<Rect> {
    let r = RECT {
        left: anchor.left,
        top: anchor.top,
        right: anchor.right(),
        bottom: anchor.bottom(),
    };
    let monitor = unsafe { MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    anyhow::ensure!(
        unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool(),
        "Unable to read monitor work area"
    );
    Ok(Rect {
        left: info.rcWork.left,
        top: info.rcWork.top,
        width: info.rcWork.right - info.rcWork.left,
        height: info.rcWork.bottom - info.rcWork.top,
    })
}
pub fn move_resize(window: &Window, r: Rect) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd(window)?,
            None,
            r.left,
            r.top,
            r.width,
            r.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
    }
    Ok(())
}
pub fn configure(window: &Window, no_activate: bool, topmost: bool) -> Result<()> {
    let hwnd = hwnd(window)?;
    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | WS_EX_TOOLWINDOW.0 as isize;
        style &= !(WS_EX_APPWINDOW.0 as isize);
        if no_activate {
            style |= WS_EX_NOACTIVATE.0 as isize;
        } else {
            style &= !(WS_EX_NOACTIVATE.0 as isize);
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        SetWindowPos(
            hwnd,
            Some(if topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            }),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )?;
    }
    Ok(())
}
pub fn show_without_activation(window: &Window) -> Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd(window)?, SW_SHOWNOACTIVATE);
    }
    Ok(())
}
pub fn tooltip(window: &Window, text: &str) -> Result<()> {
    // The accessible title also supplies a useful label to system window tools.
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        SetWindowTextW(hwnd(window)?, windows::core::PCWSTR(text.as_ptr()))?;
    }
    Ok(())
}
pub fn cursor() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe {
        GetCursorPos(&mut p).ok()?;
    }
    Some((p.x, p.y))
}
pub fn start_move(hwnd_value: isize) {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::Input::KeyboardAndMouse::ReleaseCapture,
    };
    unsafe {
        let _ = ReleaseCapture();
        SendMessageW(
            HWND(hwnd_value as *mut _),
            WM_NCLBUTTONDOWN,
            Some(WPARAM(HTCAPTION as usize)),
            Some(LPARAM(0)),
        );
    }
}
pub fn capture(window: &Window) {
    if let Ok(hwnd) = hwnd(window) {
        unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(hwnd);
        }
    }
}
pub fn release_capture() {
    unsafe {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
    }
}
pub fn valid_orb_style(window: &Window) -> bool {
    hwnd(window).is_ok_and(|hwnd| unsafe {
        let s = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        s & WS_EX_TOOLWINDOW.0 != 0 && s & WS_EX_NOACTIVATE.0 != 0 && s & WS_EX_APPWINDOW.0 == 0
    })
}
pub fn place_panel(window: &Window, anchor: Rect) -> Result<()> {
    move_resize(
        window,
        geometry::panel(anchor, work_area(anchor)?, scale(window)),
    )
}
pub fn initialize_orb(window: &Window, x: Option<i32>, y: Option<i32>) -> Result<Rect> {
    let current = rect(window)?;
    let area = work_area(current)?;
    let scale = scale(window);
    let rect = Rect {
        left: x.unwrap_or(area.right() - (108. * scale) as i32),
        top: y.unwrap_or(area.top + area.height / 2 - (44. * scale) as i32),
        width: (geometry::BALL_SIZE * scale).round() as i32,
        height: (geometry::BALL_SIZE * scale).round() as i32,
    }
    .clamp(area, 4);
    move_resize(window, rect)?;
    Ok(rect)
}
pub fn show_context(
    menu: tray_icon::menu::Menu,
    window: &Window,
    cx: &mut gpui_kit::App,
) -> Result<()> {
    use tray_icon::menu::ContextMenu;
    let hwnd = hwnd(window)?.0 as isize;
    // Track the native popup without retaining a mutable GPUI App/Entity borrow.
    cx.foreground_executor()
        .spawn(async move {
            unsafe {
                menu.show_context_menu_for_hwnd(hwnd, None);
            }
        })
        .detach();
    Ok(())
}
pub fn startup_enabled() -> bool {
    use windows::Win32::System::Registry::*;
    use windows::core::w;
    let mut size = 0u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            w!("KrillDesktop"),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    }
    .is_ok()
        && size > 2
}
