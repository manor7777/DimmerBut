//! Application state: settings + dimming controller (`AppCore`), the flyout, the tray icon and hotkeys.

use crate::dim::DimController;
use crate::flyout::Flyout;
use crate::icon;
use crate::settings::{AppSettings, format_hotkey};
use crate::wide;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY: u32 = WM_APP + 1;
pub const WM_MONITORS_DONE: u32 = WM_APP + 2;
/// wParam = one of the `CMD_*` constants; lets other windows ask the main loop to do something.
pub const WM_RUN: u32 = WM_APP + 3;

pub const CMD_SETTINGS: usize = 1;
pub const CMD_SHOW_FLYOUT: usize = 2;

pub const TIMER_APPLY: usize = 1;
pub const TIMER_POLL: usize = 2;

const HK_INCREASE: i32 = 1;
const HK_DECREASE: i32 = 2;
const MOD_NOREPEAT: u32 = 0x4000;

pub struct AppCore {
    pub settings: AppSettings,
    pub dim: DimController,
}

pub struct App {
    pub hwnd: HWND,
    pub core: AppCore,
    pub flyout: Flyout,
    tray_icon: Option<HICON>,
    /// Dim schedule state: last position applied, whether a monitor scan was started for it, and the last
    /// (brightness, monitor count) written to hardware.
    sched_f: f64,
    sched_scanned: bool,
    sched_hw: Option<(i32, usize)>,
}

impl App {
    pub fn new(hwnd: HWND, settings: AppSettings) -> windows::core::Result<Self> {
        let dim = DimController::new(&settings);
        Ok(Self { hwnd, core: AppCore { settings, dim }, flyout: Flyout::new()?, tray_icon: None, sched_f: 0.0, sched_scanned: false, sched_hw: None })
    }

    // ---------------------------------------------------------------------------------------------
    // Dimming / night mode
    // ---------------------------------------------------------------------------------------------

    pub fn set_level(&mut self, v: i32) {
        if self.core.dim.set_level(&mut self.core.settings, v) {
            self.refresh_ui();
        }
    }

    pub fn step_level(&mut self, delta: i32) {
        let v = self.core.dim.level() + delta;
        self.set_level(v);
    }

    pub fn set_night(&mut self, on: bool) {
        if self.core.dim.set_night(&mut self.core.settings, on) {
            self.refresh_ui();
        }
    }

    /// Periodic / system-event refresh of the colour matrix.
    pub fn reapply(&mut self) {
        let mut changed = self.schedule_tick();
        changed |= self.core.dim.apply(&self.core.settings);
        if changed {
            self.refresh_ui();
        }
    }

    /// Eases brightness along the dim schedule: monitor brightness when a controller exists, otherwise
    /// extra dimming. Idle (and leaves manual changes alone) while outside the schedule.
    /// Returns true if the UI should refresh.
    fn schedule_tick(&mut self) -> bool {
        let s = &self.core.settings;
        let f = if s.sched_enabled { crate::nightmode::schedule_factor(s, chrono::Local::now()) } else { 0.0 };
        if f <= 0.0 && self.sched_f <= 0.0 {
            return false;
        }
        if self.flyout.monitors.is_empty() && !self.sched_scanned {
            // No scan has run yet (the flyout hasn't been opened): look for controllers first.
            self.sched_scanned = true;
            self.flyout.begin_detect(self.hwnd);
            return false;
        }
        let (day, night) = (s.sched_day_brightness, s.sched_night_brightness);
        let target = day as f64 + (night - day) as f64 * f;
        let mut changed;
        if self.flyout.monitors.is_empty() {
            let dim = ((1.0 - target / day as f64) * 100.0) as f32;
            changed = self.core.dim.set_schedule_dim(&self.core.settings, dim);
        } else {
            changed = self.core.dim.set_schedule_dim(&self.core.settings, 0.0);
            let b = target.round() as i32;
            let key = (b, self.flyout.monitors.len());
            if self.sched_hw != Some(key) {
                self.sched_hw = Some(key);
                for m in &mut self.flyout.monitors {
                    if m.percent != b {
                        m.request_brightness(b);
                    }
                }
                changed = true;
            }
        }
        self.sched_f = f;
        if f <= 0.0 {
            self.sched_hw = None;
        }
        changed
    }

    /// Called after the settings dialog changed `core.settings`.
    pub fn settings_changed(&mut self) {
        self.sched_scanned = false;
        self.sched_hw = None;
        if !self.core.settings.sched_enabled {
            // switched off mid-schedule: drop any software dimming it added
            self.sched_f = 0.0;
            self.core.dim.set_schedule_dim(&self.core.settings, 0.0);
        }
        self.core.dim.apply(&self.core.settings);
        self.configure_poll_timer();
        self.refresh_ui();
    }

    pub fn configure_poll_timer(&self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_POLL);
            let s = &self.core.settings;
            if s.auto_poll_monitors && s.show_hardware_brightness {
                SetTimer(Some(self.hwnd), TIMER_POLL, s.poll_interval_seconds.max(5) as u32 * 1000, None);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Tray
    // ---------------------------------------------------------------------------------------------

    fn tray_data(&self) -> NOTIFYICONDATAW {
        let lvl = self.core.dim.effective_level(&self.core.settings);
        let mut tip = format!("DimmerBut - {lvl}%");
        if self.core.settings.night_enabled {
            tip.push_str(&format!(" | Night {}K", self.core.dim.night_kelvin(&self.core.settings) as i32));
        }
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: self.tray_icon.unwrap_or_default(),
            ..Default::default()
        };
        for (i, c) in wide(&tip).iter().take(127).enumerate() {
            nid.szTip[i] = *c;
        }
        nid
    }

    fn build_icon(&mut self) {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) };
        let lvl = self.core.dim.effective_level(&self.core.settings);
        if let Some(new) = icon::moon_icon(lvl, size) {
            if let Some(old) = self.tray_icon.replace(new) {
                unsafe {
                    let _ = DestroyIcon(old);
                }
            }
        }
    }

    pub fn tray_add(&mut self) {
        self.build_icon();
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &self.tray_data());
        }
    }

    pub fn refresh_ui(&mut self) {
        self.build_icon();
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.tray_data());
        }
        if self.flyout.is_visible() {
            unsafe {
                let _ = InvalidateRect(Some(self.flyout.hwnd), None, false);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Hotkeys
    // ---------------------------------------------------------------------------------------------

    /// Re-registers hotkeys from settings. Returns descriptions of any that failed.
    pub fn reload_hotkeys(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), HK_INCREASE);
            let _ = UnregisterHotKey(Some(self.hwnd), HK_DECREASE);
        }
        let s = &self.core.settings;
        if !s.hotkeys_enabled {
            return failures;
        }
        for (id, mods, key, name) in [
            (HK_INCREASE, s.increase_mods, s.increase_key, "Increase"),
            (HK_DECREASE, s.decrease_mods, s.decrease_key, "Decrease"),
        ] {
            if key == 0 {
                continue;
            }
            let ok = unsafe { RegisterHotKey(Some(self.hwnd), id, HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT), key as u32).is_ok() };
            if !ok {
                failures.push(format!("{name} ({}) is already in use by another program.", format_hotkey(mods, key)));
            }
        }
        failures
    }

    pub fn on_hotkey(&mut self, id: i32) {
        match id {
            HK_INCREASE => self.step_level(crate::settings::STEP),
            HK_DECREASE => self.step_level(-crate::settings::STEP),
            _ => {}
        }
    }

    pub fn on_close(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_APPLY);
            let _ = KillTimer(Some(self.hwnd), TIMER_POLL);
            let _ = UnregisterHotKey(Some(self.hwnd), HK_INCREASE);
            let _ = UnregisterHotKey(Some(self.hwnd), HK_DECREASE);
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.tray_data());
            if let Some(i) = self.tray_icon.take() {
                let _ = DestroyIcon(i);
            }
        }
        self.flyout.hide();
        self.core.dim.shutdown();
    }
}
