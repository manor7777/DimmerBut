//! Hidden main window (tray, timers, hotkeys, system events) and the tray context menu.

use crate::app::*;
use crate::flyout::Fx;
use crate::settings::{MAX_LEVEL, STEP};
use crate::{log_line, message_box, settings_ui, wide};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCSTR, PCWSTR, w};

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static SETTINGS_OPEN: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` on the app state. `None` if the state is currently borrowed (a re-entrant message sent from
/// inside one of the app's own window calls) - such messages are safely ignored.
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|c| {
        let mut g = c.try_borrow_mut().ok()?;
        Some(f(g.as_mut()?))
    })
}

pub fn install_app(app: App) {
    APP.with(|c| *c.borrow_mut() = Some(app));
}

pub fn remove_app() {
    APP.with(|c| *c.borrow_mut() = None);
}

const CLASS: PCWSTR = w!("DimmerButMainWindow");

const ID_PRESET_BASE: usize = 1000; // + dimming level
const ID_NIGHT: usize = 2000;
const ID_SETTINGS: usize = 2001;
const ID_ABOUT: usize = 2002;
const ID_EXIT: usize = 2003;

pub fn create_main_window() -> windows::core::Result<HWND> {
    unsafe {
        let hi = HINSTANCE(GetModuleHandleW(None)?.0);
        let wc = WNDCLASSW { lpfnWndProc: Some(wnd_proc), hInstance: hi, lpszClassName: CLASS, ..Default::default() };
        RegisterClassW(&wc);
        TASKBAR_CREATED.with(|t| t.set(RegisterWindowMessageW(w!("TaskbarCreated"))));
        // A real (never shown) top-level window: message-only windows don't receive broadcasts such as
        // TaskbarCreated, WM_DISPLAYCHANGE or WM_POWERBROADCAST.
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, CLASS, w!("DimmerBut"), WS_POPUP, 0, 0, 0, 0, None, None, Some(hi), None)?;
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        Ok(hwnd)
    }
}

/// Dark popup menus (Windows 10 1903+; undocumented uxtheme ordinals, harmless if unavailable).
pub fn enable_dark_menus() {
    unsafe {
        let Ok(h) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        if let Some(set_mode) = GetProcAddress(h, PCSTR(135 as *const u8)) {
            let f: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(set_mode);
            f(2); // ForceDark
        }
        if let Some(flush) = GetProcAddress(h, PCSTR(136 as *const u8)) {
            let f: unsafe extern "system" fn() = std::mem::transmute(flush);
            f();
        }
    }
}

/// Acts on what a flyout input event asked for.
pub fn apply_fx(fx: Fx) {
    if fx.state_changed {
        with_app(|a| a.refresh_ui());
    }
    if fx.open_settings {
        if let Some(hwnd) = with_app(|a| a.hwnd) {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_RUN, WPARAM(CMD_SETTINGS), LPARAM(0));
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Commands
// -------------------------------------------------------------------------------------------------

fn toggle_flyout() {
    with_app(|a| {
        if a.flyout.is_visible() {
            a.flyout.hide();
        } else if unsafe { windows::Win32::System::SystemInformation::GetTickCount64() } - a.flyout.last_hidden > 300 {
            // (the tray click that deactivated the open flyout also arrives here; ignore it)
            let hwnd = a.hwnd;
            a.flyout.show(&a.core, hwnd);
        }
    });
}

fn toggle_night() {
    let Some((on, has_loc)) = with_app(|a| (a.core.settings.night_enabled, a.core.settings.has_location)) else { return };
    if !on && !has_loc {
        message_box(None, "Night mode needs a location first. Open Settings > Night mode.", "DimmerBut", MB_OK | MB_ICONINFORMATION);
        open_settings();
        return;
    }
    with_app(|a| a.set_night(!on));
}

fn show_about() {
    message_box(
        None,
        "DimmerBut\n\nSoftware dimming below your monitor's minimum, an optional f.lux-style night tint, and hardware brightness / power control for your displays.\n\nDimming and tint use the Windows Magnification colour effect, so video, PiP, the taskbar and Start menu are covered too.",
        "About DimmerBut",
        MB_OK | MB_ICONINFORMATION,
    );
}

pub fn open_settings() {
    if SETTINGS_OPEN.with(|s| s.replace(true)) {
        return;
    }
    with_app(|a| a.flyout.hide());
    let Some(current) = with_app(|a| a.core.settings.clone()) else {
        SETTINGS_OPEN.with(|s| s.set(false));
        return;
    };

    // The dialog runs its own message loop, so no borrow of the app state may be held here.
    let result = settings_ui::show(None, current);
    SETTINGS_OPEN.with(|s| s.set(false));

    if let Some(new) = result {
        let failures = with_app(|a| {
            a.core.settings = new;
            a.core.settings.sanitize();
            if !a.core.settings.save() {
                log_line("could not save settings.json");
            }
            a.settings_changed();
            a.reload_hotkeys()
        })
        .unwrap_or_default();
        if !failures.is_empty() {
            message_box(None, &failures.join("\n"), "DimmerBut", MB_OK | MB_ICONWARNING);
        }
    }
}

fn run_command(hwnd: HWND, cmd: usize) {
    match cmd {
        c if (ID_PRESET_BASE..ID_PRESET_BASE + 1000).contains(&c) => {
            with_app(|a| a.set_level((c - ID_PRESET_BASE) as i32));
        }
        ID_NIGHT => toggle_night(),
        ID_SETTINGS => open_settings(),
        ID_ABOUT => show_about(),
        ID_EXIT => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        _ => {}
    }
}

fn show_menu(hwnd: HWND) {
    let Some((lvl, night)) = with_app(|a| (a.core.dim.effective_level(&a.core.settings), a.core.settings.night_enabled)) else { return };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let add = |id: usize, text: &str, flags: MENU_ITEM_FLAGS| {
            let t = wide(text);
            let _ = AppendMenuW(menu, MF_STRING | flags, id, PCWSTR(t.as_ptr()));
        };
        let sep = || {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        };
        add(0, "DimmerBut", MF_GRAYED);
        sep();
        for v in (0..=MAX_LEVEL).step_by(STEP as usize) {
            add(ID_PRESET_BASE + v as usize, &format!("{v}%"), if v == lvl { MF_CHECKED } else { MF_UNCHECKED });
        }
        sep();
        add(ID_NIGHT, "Night mode", if night { MF_CHECKED } else { MF_UNCHECKED });
        add(ID_SETTINGS, "Settings...", MENU_ITEM_FLAGS(0));
        add(ID_ABOUT, "About...", MENU_ITEM_FLAGS(0));
        add(ID_EXIT, "Exit", MENU_ITEM_FLAGS(0));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        if cmd.0 != 0 {
            run_command(hwnd, cmd.0 as usize);
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Window procedure
// -------------------------------------------------------------------------------------------------

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // A panic in one handler must not take the dimmer down: log it and carry on.
    match catch_unwind(AssertUnwindSafe(|| wnd_proc_inner(hwnd, msg, wp, lp))) {
        Ok(r) => r,
        Err(_) => {
            log_line("[UI thread] panic in window procedure");
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
    }
}

fn wnd_proc_inner(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg != 0 && msg == TASKBAR_CREATED.with(|t| t.get()) {
            with_app(|a| a.tray_add());
            return LRESULT(0);
        }
        match msg {
            WM_TIMER => {
                match wp.0 {
                    TIMER_APPLY => {
                        with_app(|a| a.reapply());
                    }
                    TIMER_POLL => {
                        with_app(|a| a.flyout.begin_detect(a.hwnd));
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_TRAY => {
                match (lp.0 & 0xffff) as u32 {
                    WM_LBUTTONUP => toggle_flyout(),
                    WM_RBUTTONUP => show_menu(hwnd),
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_MONITORS_DONE => {
                with_app(|a| a.flyout.monitors_done(&a.core));
                return LRESULT(0);
            }
            WM_RUN => {
                match wp.0 {
                    CMD_SETTINGS => open_settings(),
                    CMD_SHOW_FLYOUT => {
                        with_app(|a| {
                            let h = a.hwnd;
                            a.flyout.show(&a.core, h)
                        });
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_HOTKEY => {
                with_app(|a| a.on_hotkey(wp.0 as i32));
                return LRESULT(0);
            }
            // The colour effect can be lost when displays, sessions or power state change: re-apply it.
            WM_DISPLAYCHANGE | WM_WTSSESSION_CHANGE | WM_POWERBROADCAST => {
                with_app(|a| a.reapply());
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            }
            WM_DESTROY => {
                with_app(|a| a.on_close());
                let _ = WTSUnRegisterSessionNotification(hwnd);
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}
