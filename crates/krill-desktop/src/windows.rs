use crate::geometry::{self, Rect};
use anyhow::Result;
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect},
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

// Use the same directory-derived names as 0.1.x so a second launch brings the
// existing panel forward, including while an old client is still running.
pub struct Instance {
    mutex: windows::Win32::Foundation::HANDLE,
    event: windows::Win32::Foundation::HANDLE,
    owned: bool,
}
impl Instance {
    pub fn acquire(directory: &std::path::Path) -> Result<Option<Self>> {
        use sha2::{Digest, Sha256};
        use windows::Win32::{
            Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
            System::Threading::*,
        };
        let directory = std::path::absolute(directory)?;
        let key = format!(
            "{:X}",
            Sha256::digest(directory.to_string_lossy().as_bytes())
        );
        let name: Vec<u16> = format!("Local\\KrillDesktop-{}", &key[..16])
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let event_name: Vec<u16> = format!("Local\\KrillDesktop-Show-{}", &key[..16])
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let mutex = CreateMutexW(None, true, windows::core::PCWSTR(name.as_ptr()))?;
            let exists = GetLastError() == ERROR_ALREADY_EXISTS;
            let event = CreateEventW(
                None,
                false,
                false,
                windows::core::PCWSTR(event_name.as_ptr()),
            )?;
            let instance = Self {
                mutex,
                event,
                owned: !exists,
            };
            if exists {
                SetEvent(event)?;
                drop(instance);
                Ok(None)
            } else {
                Ok(Some(instance))
            }
        }
    }
    pub fn watch(&self, mut activate: impl FnMut() + Send + 'static) {
        use windows::Win32::{
            Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, WAIT_OBJECT_0},
            System::Threading::*,
        };
        let mut duplicate = Default::default();
        unsafe {
            if DuplicateHandle(
                GetCurrentProcess(),
                self.event,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
            .is_err()
            {
                return;
            }
        }
        let event = duplicate.0 as isize;
        std::thread::spawn(move || unsafe {
            let event = windows::Win32::Foundation::HANDLE(event as *mut _);
            while WaitForSingleObject(event, INFINITE) == WAIT_OBJECT_0 {
                activate();
            }
            let _ = CloseHandle(event);
        });
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            if self.owned {
                let _ = windows::Win32::System::Threading::ReleaseMutex(self.mutex);
            }
            let _ = windows::Win32::Foundation::CloseHandle(self.mutex);
            let _ = windows::Win32::Foundation::CloseHandle(self.event);
        }
    }
}

pub fn hwnd(window: &Window) -> Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|_| anyhow::anyhow!("Native window handle unavailable"))?;
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
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSENDCHANGING,
        )?;
    }
    Ok(())
}
// GPUI's default Windows procedure keeps an invisible resize frame and returns
// MA_ACTIVATE even for WS_EX_NOACTIVATE. Our borderless utility windows need
// their entire bounds as client area, and the orb must never take activation.
unsafe extern "system" fn utility_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    id: usize,
    no_activate: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::{
        Foundation::LRESULT,
        UI::Shell::{DefSubclassProc, RemoveWindowSubclass},
    };
    unsafe {
        if message == WM_NCCALCSIZE && wparam.0 != 0 {
            return LRESULT(0);
        }
        if no_activate != 0 {
            if message == WM_MOUSEACTIVATE {
                return LRESULT(MA_NOACTIVATE as isize);
            }
            if message == WM_GETMINMAXINFO {
                let limits = &mut *(lparam.0 as *mut MINMAXINFO);
                limits.ptMinTrackSize = POINT { x: 1, y: 1 };
                return LRESULT(0);
            }
        }
        if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(utility_window_proc), id);
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}
pub fn configure(window: &Window, no_activate: bool, topmost: bool) -> Result<()> {
    let hwnd = hwnd(window)?;
    unsafe {
        anyhow::ensure!(
            windows::Win32::UI::Shell::SetWindowSubclass(
                hwnd,
                Some(utility_window_proc),
                0x4b52494c,
                no_activate as usize
            )
            .as_bool(),
            "Unable to configure borderless utility window"
        );
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (style
                & !((WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_THICKFRAME
                    | WS_CAPTION
                    | WS_BORDER
                    | WS_DLGFRAME)
                    .0 as isize))
                | WS_POPUP.0 as isize,
        );
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
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        let s = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        s & WS_EX_TOOLWINDOW.0 != 0
            && s & WS_EX_NOACTIVATE.0 != 0
            && s & WS_EX_APPWINDOW.0 == 0
            && SendMessageW(
                hwnd,
                WM_MOUSEACTIVATE,
                Some(WPARAM(0)),
                Some(LPARAM(((WM_LBUTTONDOWN << 16) | HTCLIENT) as isize)),
            )
            .0 == MA_NOACTIVATE as isize
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
    let requested = Rect {
        left: x.unwrap_or(area.right() - (108. * scale) as i32),
        top: y.unwrap_or(area.top + area.height / 2 - (44. * scale) as i32),
        width: (geometry::BALL_SIZE * scale).round() as i32,
        height: (geometry::BALL_SIZE * scale).round() as i32,
    };
    let rect = requested.clamp(work_area(requested)?, 4);
    move_resize(window, rect)?;
    let scale = self::scale(window);
    let rect = Rect {
        width: (geometry::BALL_SIZE * scale).round() as i32,
        height: (geometry::BALL_SIZE * scale).round() as i32,
        ..rect
    }
    .clamp(work_area(rect)?, 4);
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

// A separate native tooltip can extend beyond the small orb window. A tooltip
// rendered inside the 88px GPUI surface would be clipped to that surface.
pub struct Tooltip {
    window: HWND,
    owner: HWND,
    text: Vec<u16>,
}
impl Tooltip {
    pub fn new(owner: &Window, text: &str) -> Result<Self> {
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::Controls::*,
        };
        let hwnd = hwnd(owner)?;
        unsafe {
            let controls = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES,
            };
            anyhow::ensure!(
                InitCommonControlsEx(&controls).as_bool(),
                "Unable to initialize native controls"
            );
            let window = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                TOOLTIPS_CLASSW,
                None,
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
                0,
                0,
                0,
                0,
                Some(hwnd),
                None,
                None,
                None,
            )?;
            let mut tooltip = Self {
                window,
                owner: hwnd,
                text: text.encode_utf16().chain(Some(0)).collect(),
            };
            let info = tooltip.info();
            if SendMessageW(
                window,
                TTM_ADDTOOLW,
                Some(WPARAM(0)),
                Some(LPARAM(&info as *const _ as isize)),
            )
            .0 == 0
            {
                anyhow::bail!("Unable to register orb tooltip");
            }
            SendMessageW(
                window,
                TTM_SETMAXTIPWIDTH,
                Some(WPARAM(0)),
                Some(LPARAM((360. * scale(owner)) as isize)),
            );
            Ok(tooltip)
        }
    }
    fn info(&mut self) -> windows::Win32::UI::Controls::TTTOOLINFOW {
        use windows::Win32::UI::Controls::*;
        TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_IDISHWND | TTF_SUBCLASS,
            hwnd: self.owner,
            uId: self.owner.0 as usize,
            lpszText: windows::core::PWSTR(self.text.as_mut_ptr()),
            ..Default::default()
        }
    }
    pub fn update(&mut self, text: &str) {
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::Controls::TTM_UPDATETIPTEXTW,
        };
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        if text == self.text {
            return;
        }
        self.text = text;
        let info = self.info();
        unsafe {
            SendMessageW(
                self.window,
                TTM_UPDATETIPTEXTW,
                Some(WPARAM(0)),
                Some(LPARAM(&info as *const _ as isize)),
            );
        }
    }
}
impl Drop for Tooltip {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Instance;
    #[test]
    fn single_instance_is_directory_scoped_and_releases_ownership() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let first = Instance::acquire(a.path()).unwrap().unwrap();
        assert!(Instance::acquire(a.path()).unwrap().is_none());
        let separate = Instance::acquire(b.path()).unwrap().unwrap();
        drop(first);
        assert!(Instance::acquire(a.path()).unwrap().is_some());
        drop(separate);
    }
}
