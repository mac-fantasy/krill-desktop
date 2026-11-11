//! Small native OAuth popup owners. The dashboard itself is rendered by GPUI;
//! popup browser windows retain their original WebKit / WebView2 relationship.
use anyhow::Result;
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

#[cfg(target_os = "macos")]
pub struct PopupWindow(objc2::rc::Retained<objc2_app_kit::NSWindow>);
#[cfg(target_os = "windows")]
pub struct PopupWindow(windows::Win32::Foundation::HWND);

impl PopupWindow {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            use objc2::MainThreadOnly;
            use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask as Mask};
            use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
            let mtm = objc2::MainThreadMarker::new()
                .ok_or_else(|| anyhow::anyhow!("OAuth popup requires the main thread"))?;
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    NSRect::new(NSPoint::new(0., 0.), NSSize::new(720., 680.)),
                    Mask::Titled | Mask::Closable | Mask::Resizable,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe {
                window.setReleasedWhenClosed(false);
            }
            window.setTitle(&NSString::from_str("Krill · 第三方登录"));
            window.center();
            Ok(Self(window))
        }
        #[cfg(target_os = "windows")]
        {
            use windows::{
                Win32::{
                    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
                    System::LibraryLoader::GetModuleHandleW,
                    UI::WindowsAndMessaging::*,
                },
                core::w,
            };
            unsafe extern "system" fn procedure(
                hwnd: HWND,
                message: u32,
                w: WPARAM,
                l: LPARAM,
            ) -> LRESULT {
                unsafe { DefWindowProcW(hwnd, message, w, l) }
            }
            let instance = unsafe { GetModuleHandleW(None)? };
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: w!("KrillOAuthPopup"),
                ..Default::default()
            };
            unsafe {
                RegisterClassW(&class);
            }
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    w!("KrillOAuthPopup"),
                    w!("Krill · 第三方登录"),
                    WS_OVERLAPPEDWINDOW,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    760,
                    720,
                    None,
                    None,
                    Some(HINSTANCE(instance.0)),
                    None,
                )?
            };
            Ok(Self(hwnd))
        }
    }
    pub fn show(&self) {
        #[cfg(target_os = "macos")]
        self.0.makeKeyAndOrderFront(None);
        #[cfg(target_os = "windows")]
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                self.0,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
        }
    }
    pub fn is_open(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.0.isVisible()
        }
        #[cfg(target_os = "windows")]
        {
            unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(self.0)) }.as_bool()
        }
    }
    pub fn size(&self) -> (f64, f64) {
        #[cfg(target_os = "macos")]
        {
            let r = self.0.contentView().map(|v| v.bounds()).unwrap_or_default();
            (r.size.width, r.size.height)
        }
        #[cfg(target_os = "windows")]
        {
            let mut r = windows::Win32::Foundation::RECT::default();
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(self.0, &mut r);
            }
            (r.right as f64, r.bottom as f64)
        }
    }
    pub fn close(&self) {
        #[cfg(target_os = "macos")]
        self.0.close();
        #[cfg(target_os = "windows")]
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.0);
        }
    }
}
impl Drop for PopupWindow {
    fn drop(&mut self) {
        self.close();
    }
}
impl HasWindowHandle for PopupWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        #[cfg(target_os = "macos")]
        let raw = {
            let view = self.0.contentView().ok_or(HandleError::Unavailable)?;
            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(
                std::ptr::NonNull::from(&*view).cast(),
            ))
        };
        #[cfg(target_os = "windows")]
        let raw = RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
            std::num::NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?,
        ));
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}
