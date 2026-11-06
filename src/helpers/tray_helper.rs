//! System tray (Herd-style): close hides to tray; click opens the window again.
//!
//! - Left click → open the main window
//! - Right click → context menu only (does **not** open the app)
//!
//! The popup is tracked against a dedicated hidden host HWND so the main window
//! can stay hidden in the tray.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use eframe::egui;
use tray_icon::menu::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, POINT, WPARAM,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW,
    GetCursorPos, PostMessageW, RegisterClassW, SetForegroundWindow, ShowWindow, TrackPopupMenu,
    ASFW_ANY, CS_HREDRAW, CS_VREDRAW, SW_HIDE, SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, WM_NULL, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::utils::constants::WINDOW_TITLE;

static PENDING: Mutex<Vec<TrayAction>> = Mutex::new(Vec::new());
static MENU_HOST_HWND: AtomicIsize = AtomicIsize::new(0);
static MENU_HMENU: AtomicIsize = AtomicIsize::new(0);
#[cfg(windows)]
static SHOW_EVENT: AtomicIsize = AtomicIsize::new(0);

#[cfg(windows)]
static SINGLETON_MUTEX: Mutex<Option<isize>> = Mutex::new(None);

/// Returns `false` if another BeAced GUI is already running (asks it to show instead).
///
/// Never kill the existing tray instance on relaunch — that race left users with
/// no window when opening the exe again.
pub fn try_become_singleton() -> bool {
    #[cfg(windows)]
    {
        ensure_show_event(true);
        unsafe {
            let Ok(handle) = CreateMutexW(None, true, w!("Global\\BeAcedSingleton")) else {
                return true;
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                signal_show_request();
                let _ = AllowSetForegroundWindow(ASFW_ANY);
                show_window();
                std::thread::sleep(Duration::from_millis(400));
                false
            } else {
                if let Ok(mut slot) = SINGLETON_MUTEX.lock() {
                    *slot = Some(handle.0 as isize);
                }
                true
            }
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(windows)]
fn ensure_show_event(create: bool) {
    unsafe {
        if SHOW_EVENT.load(Ordering::SeqCst) != 0 {
            return;
        }
        // SYNCHRONIZE (0x00100000) | EVENT_MODIFY_STATE
        const ACCESS: u32 = 0x0010_0000 | 0x0002;
        let handle = if create {
            CreateEventW(None, false, false, w!("Global\\BeAcedShowWindow")).ok()
        } else {
            OpenEventW(
                windows::Win32::System::Threading::SYNCHRONIZATION_ACCESS_RIGHTS(ACCESS),
                false,
                w!("Global\\BeAcedShowWindow"),
            )
            .ok()
            .or_else(|| CreateEventW(None, false, false, w!("Global\\BeAcedShowWindow")).ok())
        };
        if let Some(h) = handle {
            SHOW_EVENT.store(h.0 as isize, Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
fn signal_show_request() {
    ensure_show_event(false);
    let raw = SHOW_EVENT.load(Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = SetEvent(windows::Win32::Foundation::HANDLE(raw as *mut _));
        }
    }
}

/// Polled by the running GUI — another launch asked us to leave the tray.
pub fn take_external_show_request() -> bool {
    #[cfg(windows)]
    {
        ensure_show_event(true);
        let raw = SHOW_EVENT.load(Ordering::SeqCst);
        if raw == 0 {
            return false;
        }
        unsafe {
            WaitForSingleObject(windows::Win32::Foundation::HANDLE(raw as *mut _), 0)
                == windows::Win32::Foundation::WAIT_OBJECT_0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Drop the tray icon (NIM_DELETE) and give the shell a moment to remove it.
pub fn dispose(tray: AppTray) {
    drop(tray);
    destroy_menu_host();
    std::thread::sleep(Duration::from_millis(200));
}

/// Kill every `segotep-be-aced.exe` / `sensor-bridge.exe` (including this process).
pub fn kill_all_app_processes() {
    crate::services::sensor_service::kill_all_sensor_bridges();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/IM", "segotep-be-aced.exe", "/F", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

pub struct AppTray {
    _tray: TrayIcon,
    _menu: Menu,
}

#[derive(Clone, Copy, Debug)]
pub enum TrayAction {
    Show,
    Quit,
}

fn push_action(action: TrayAction) {
    if let Ok(mut q) = PENDING.lock() {
        if matches!(action, TrayAction::Show) && q.iter().any(|a| matches!(a, TrayAction::Show)) {
            return;
        }
        q.push(action);
    }
}

pub fn install(ctx: &egui::Context) -> anyhow::Result<AppTray> {
    let host = ensure_menu_host()?;

    let show_item = MenuItem::with_id("show", "Open BeAced", true, None);
    let quit_item = MenuItem::with_id("quit", "Quit", true, None);
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&show_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    // Route WM_COMMAND from TrackPopupMenu into MenuEvent on the host HWND.
    #[cfg(windows)]
    unsafe {
        menu.attach_menu_subclass_for_hwnd(host);
    }
    MENU_HMENU.store(menu.hpopupmenu(), Ordering::SeqCst);

    let tray = TrayIconBuilder::new()
        .with_tooltip(WINDOW_TITLE)
        .with_icon(make_icon())
        .with_menu_on_left_click(false)
        .build()?;

    let ctx_tray = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                show_window();
                push_action(TrayAction::Show);
                ctx_tray.request_repaint();
            }
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } => {
                // Popup only — never open the main window.
                show_raw_context_menu();
            }
            _ => {}
        }
    }));

    let ctx_menu = ctx.clone();
    let show_id_menu = show_id.clone();
    let quit_id_menu = quit_id.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == show_id_menu {
            show_window();
            push_action(TrayAction::Show);
            ctx_menu.request_repaint();
        } else if event.id == quit_id_menu {
            push_action(TrayAction::Quit);
            ctx_menu.request_repaint();
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(400));
                kill_all_app_processes();
            });
        }
    }));

    Ok(AppTray {
        _tray: tray,
        _menu: menu,
    })
}

/// Call once per egui frame to drain tray actions.
pub fn poll_actions(_tray: &AppTray) -> Option<TrayAction> {
    let mut q = PENDING.lock().ok()?;
    if q.is_empty() {
        return None;
    }
    if let Some(i) = q.iter().position(|a| matches!(a, TrayAction::Quit)) {
        return Some(q.remove(i));
    }
    Some(q.remove(0))
}

/// Show the tray menu at the cursor without revealing the main window.
fn show_raw_context_menu() {
    #[cfg(windows)]
    unsafe {
        let host = MENU_HOST_HWND.load(Ordering::SeqCst);
        let hmenu = MENU_HMENU.load(Ordering::SeqCst);
        if host == 0 || hmenu == 0 {
            return;
        }
        let hwnd = HWND(host as *mut _);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            windows::Win32::UI::WindowsAndMessaging::HMENU(hmenu as *mut _),
            TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    }
}

#[cfg(windows)]
unsafe extern "system" fn menu_host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn ensure_menu_host() -> anyhow::Result<isize> {
    #[cfg(windows)]
    {
        let existing = MENU_HOST_HWND.load(Ordering::SeqCst);
        if existing != 0 {
            return Ok(existing);
        }
        unsafe {
            use windows::Win32::Foundation::HINSTANCE;
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            let module = GetModuleHandleW(None)?;
            let hinstance = HINSTANCE(module.0);
            let class_name = w!("BeAcedTrayMenuHost");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(menu_host_wnd_proc),
                hInstance: hinstance,
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name,
                w!("BeAcedTrayMenuHost"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinstance),
                None,
            )?;
            if hwnd.is_invalid() {
                anyhow::bail!("CreateWindowExW failed for tray menu host");
            }
            let raw = hwnd.0 as isize;
            MENU_HOST_HWND.store(raw, Ordering::SeqCst);
            Ok(raw)
        }
    }
    #[cfg(not(windows))]
    {
        Ok(0)
    }
}

fn destroy_menu_host() {
    #[cfg(windows)]
    {
        let host = MENU_HOST_HWND.swap(0, Ordering::SeqCst);
        MENU_HMENU.store(0, Ordering::SeqCst);
        if host != 0 {
            unsafe {
                let _ = DestroyWindow(HWND(host as *mut _));
            }
        }
    }
}

pub fn hide_window() {
    #[cfg(windows)]
    unsafe {
        if let Ok(hwnd) = FindWindowW(None, w!("BeAced")) {
            if !hwnd.is_invalid() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

pub fn show_window() {
    #[cfg(windows)]
    unsafe {
        if let Ok(hwnd) = FindWindowW(None, w!("BeAced")) {
            if !hwnd.is_invalid() {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

fn make_icon() -> tray_icon::Icon {
    crate::helpers::logo_helper::tray_icon()
}
