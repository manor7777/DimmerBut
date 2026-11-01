#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod dim;
mod flyout;
mod geocode;
mod gfx;
mod icon;
mod monitors;
mod nightmode;
mod settings;
mod settings_ui;
mod ui;
mod window;

use settings::AppSettings;
use std::io::Write;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::{ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn message_box(owner: Option<HWND>, text: &str, title: &str, style: MESSAGEBOX_STYLE) {
    let (t, c) = (wide(text), wide(title));
    unsafe {
        MessageBoxW(owner, PCWSTR(t.as_ptr()), PCWSTR(c.as_ptr()), style);
    }
}

/// Appends to `error.log` in the data folder (capped at 512 KB). Never panics.
pub fn log_line(text: &str) {
    let _ = (|| -> std::io::Result<()> {
        let dir = settings::data_folder();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("error.log");
        if std::fs::metadata(&path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
            let _ = std::fs::remove_file(&path);
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(f, "[{}] {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), text)
    })();
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        log_line(&format!("panic: {info}\n{}", std::backtrace::Backtrace::force_capture()));
    }));

    unsafe {
        // Named mutex so only one instance runs at a time.
        let _mutex = CreateMutexW(None, true, w!("Local\\DimmerBut.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        if std::panic::catch_unwind(run).is_err() {
            message_box(
                None,
                &format!("DimmerBut has crashed and will close. Details were written to:\n{}", settings::data_folder().join("error.log").display()),
                "DimmerBut stopped",
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn run() {
    unsafe {
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_STANDARD_CLASSES };
        let _ = InitCommonControlsEx(&icc);
    }
    window::enable_dark_menus();

    let settings = AppSettings::load();
    let hwnd = window::create_main_window().expect("could not create the main window");
    let app = app::App::new(hwnd, settings).expect("could not create the flyout");
    window::install_app(app);
    let failures = window::with_app(|a| {
        a.tray_add();
        a.configure_poll_timer();
        a.reload_hotkeys()
    })
    .unwrap_or_default();
    autostart::refresh_path();
    unsafe {
        SetTimer(Some(hwnd), app::TIMER_APPLY, 2000, None);
    }
    if !failures.is_empty() {
        message_box(None, &failures.join("\n"), "DimmerBut", MB_OK | MB_ICONWARNING);
    }

    // Handy for testing: `dimmerbut --flyout` / `--settings` open the window straight away.
    for arg in std::env::args().skip(1) {
        let cmd = match arg.as_str() {
            "--flyout" => {
                flyout::PINNED.store(true, std::sync::atomic::Ordering::Relaxed);
                app::CMD_SHOW_FLYOUT
            }
            "--settings" => app::CMD_SETTINGS,
            _ => continue,
        };
        unsafe {
            let _ = PostMessageW(Some(hwnd), app::WM_RUN, windows::Win32::Foundation::WPARAM(cmd), windows::Win32::Foundation::LPARAM(0));
        }
    }

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    window::remove_app();
}
