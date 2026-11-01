//! The tray flyout: monitor brightness sliders, extra dimming, night-mode switch and a settings button.
//!
//! It is a borderless popup that is custom drawn with Direct2D into a premultiplied-alpha DIB (see
//! `gfx::Surface`), with a DWM acrylic backdrop behind it. Because the window's frame is extended over the
//! whole client area, pixels we leave at alpha 0 show the blurred desktop - this is what makes the acrylic
//! actually visible.

use crate::app::{AppCore, WM_MONITORS_DONE};
use crate::gfx::Surface;
use crate::monitors::{self, MonitorInfo};
use crate::settings::MAX_LEVEL;
use crate::ui::*;
use std::sync::Mutex;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};
use windows_numerics::Matrix3x2;

const CLASS: PCWSTR = w!("DimmerButFlyout");
const W_DIP: f32 = 320.0;
const PAD: f32 = 12.0;
const CONFIRM_H: f32 = 196.0;

/// `--flyout` test mode: don't auto-hide when the window loses activation (a freshly launched process
/// isn't allowed to take the foreground, so it would vanish instantly).
pub static PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const G_MONITOR: &str = "\u{E7F4}";
const G_BRIGHTNESS: &str = "\u{E706}";
const G_MOON: &str = "\u{E708}";
const G_SETTINGS: &str = "\u{E713}";
const G_POWER: &str = "\u{E7E8}";

/// What the app should do after the flyout handled an input event.
#[derive(Default)]
pub struct Fx {
    pub state_changed: bool,
    pub open_settings: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    MonSlider(usize),
    MonPower(usize),
    DimSlider,
    Night,
    Settings,
    ConfirmCheck,
    ConfirmYes,
    ConfirmNo,
}

#[derive(Clone, Copy)]
enum RowKind {
    Monitor(usize),
    Detecting,
    NoMonitors,
    Dim,
    Night,
    Settings,
}

struct Confirm {
    mon: usize,
    dont_ask: bool,
}

struct Fmts {
    body: IDWriteTextFormat,
    right: IDWriteTextFormat,
    title: IDWriteTextFormat,
    center: IDWriteTextFormat,
    center_bold: IDWriteTextFormat,
    wrap: IDWriteTextFormat,
    icon: IDWriteTextFormat,
}

/// Monitors found by the background scan, handed to the UI thread.
static PENDING: Mutex<Option<(u32, Vec<MonitorInfo>)>> = Mutex::new(None);

pub struct Flyout {
    pub hwnd: HWND,
    pub monitors: Vec<MonitorInfo>,
    pub detecting: bool,
    pub token: u32,
    pub last_hidden: u64,
    dpi: u32,
    anchor: POINT,
    acrylic: bool,
    hover: Option<Hit>,
    drag: Option<Hit>,
    confirm: Option<Confirm>,
    tracking: bool,
    surf: Surface,
    dwrite: IDWriteFactory,
    fmts: Option<Fmts>,
}

impl Flyout {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let hi = HINSTANCE(GetModuleHandleW(None)?.0);
            let wc = WNDCLASSW {
                lpfnWndProc: Some(flyout_proc),
                hInstance: hi,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS,
                w!("DimmerBut"),
                WS_POPUP,
                0,
                0,
                320,
                200,
                None,
                None,
                Some(hi),
                None,
            )?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            Ok(Self {
                hwnd,
                monitors: Vec::new(),
                detecting: false,
                token: 0,
                last_hidden: 0,
                dpi: 96,
                anchor: POINT::default(),
                acrylic: false,
                hover: None,
                drag: None,
                confirm: None,
                tracking: false,
                surf: Surface::new()?,
                dwrite,
                fmts: None,
            })
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }

    fn k(&self) -> f32 {
        self.dpi as f32 / 96.0
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Show / hide / position
    // ---------------------------------------------------------------------------------------------

    pub fn show(&mut self, core: &AppCore, main: HWND) {
        unsafe {
            let _ = GetCursorPos(&mut self.anchor);
        }
        self.confirm = None;
        self.drag = None;
        self.hover = None;

        // Dark-tinted acrylic; elsewhere (or if switched off) we paint an opaque background.
        apply_backdrop(self.hwnd, 1, true);
        self.acrylic = core.settings.flyout_acrylic && apply_accent_acrylic(self.hwnd, 0x99_10_0c_0c);
        if !self.acrylic {
            // clear any previous accent
            unsafe { let _ = SetWindowPos(self.hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED); }
        }

        self.detecting = self.monitors.is_empty() && core.settings.show_hardware_brightness;
        self.fit(core);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
        if core.settings.show_hardware_brightness {
            self.begin_detect(main);
        }
    }

    pub fn hide(&mut self) {
        unsafe {
            if self.drag.is_some() {
                let _ = ReleaseCapture();
            }
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            self.last_hidden = GetTickCount64();
        }
        self.drag = None;
        self.hover = None;
        self.confirm = None;
    }

    pub fn dpi_changed(&mut self, dpi: u32, core: &AppCore) {
        self.dpi = dpi.max(96);
        if self.is_visible() {
            self.fit(core);
        }
    }

    /// Sizes the window to its content and tucks it into the corner of the work area nearest the cursor.
    pub fn fit(&mut self, core: &AppCore) {
        unsafe {
            let mon = MonitorFromPoint(self.anchor, MONITOR_DEFAULTTONEAREST);
            let (mut dx, mut dy) = (96u32, 96u32);
            if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy).is_ok() && dx > 0 {
                self.dpi = dx;
            }
            let k = self.k();
            let (wd, hd) = self.size_dip(core);
            let (w, h) = ((wd * k).round() as i32, (hd * k).round() as i32);

            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            let wa = if GetMonitorInfoW(mon, &mut mi).as_bool() {
                mi.rcWork
            } else {
                RECT { left: 0, top: 0, right: GetSystemMetrics(SM_CXSCREEN), bottom: GetSystemMetrics(SM_CYSCREEN) }
            };
            let margin = (8.0 * k).round() as i32;
            let mr = mi.rcMonitor;
            // Hug the taskbar edge (work area vs. monitor area tells us where it is); slide along it to the click.
            let (x, y) = if wa.bottom < mr.bottom {
                (self.anchor.x - w / 2, wa.bottom - h - margin)
            } else if wa.top > mr.top {
                (self.anchor.x - w / 2, wa.top + margin)
            } else if wa.left > mr.left {
                (wa.left + margin, self.anchor.y - h / 2)
            } else if wa.right < mr.right {
                (wa.right - w - margin, self.anchor.y - h / 2)
            } else {
                // no taskbar edge (auto-hidden): nearest corner, as before
                let x = if self.anchor.x > wa.left + (wa.right - wa.left) / 2 { wa.right - w - margin } else { wa.left + margin };
                let y = if self.anchor.y > wa.top + (wa.bottom - wa.top) / 2 { wa.bottom - h - margin } else { wa.top + margin };
                (x, y)
            };
            let x = x.clamp(wa.left + margin, (wa.right - w - margin).max(wa.left + margin));
            let y = y.clamp(wa.top + margin, (wa.bottom - h - margin).max(wa.top + margin));
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE);
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Layout (device independent pixels)
    // ---------------------------------------------------------------------------------------------

    fn rows(&self, core: &AppCore) -> (Vec<(RowKind, R4)>, f32) {
        let (x, w) = (PAD, W_DIP - 2.0 * PAD);
        let mut y = PAD;
        let mut rows = Vec::new();
        if core.settings.show_hardware_brightness {
            if self.monitors.is_empty() {
                rows.push((if self.detecting { RowKind::Detecting } else { RowKind::NoMonitors }, (x, y, w, 32.0)));
                y += 34.0;
            } else {
                for i in 0..self.monitors.len() {
                    rows.push((RowKind::Monitor(i), (x, y, w, 56.0)));
                    y += 58.0;
                }
            }
        }
        rows.push((RowKind::Dim, (x, y, w, 56.0)));
        y += 58.0;
        rows.push((RowKind::Night, (x, y, w, 40.0)));
        y += 42.0;
        rows.push((RowKind::Settings, (x, y, w, 38.0)));
        y += 38.0;
        (rows, y + PAD)
    }

    fn size_dip(&self, core: &AppCore) -> (f32, f32) {
        if self.confirm.is_some() { (W_DIP, CONFIRM_H) } else { (W_DIP, self.rows(core).1) }
    }

    /// Title, body, checkbox, "Turn off", "Cancel".
    fn confirm_layout() -> (R4, R4, R4, R4, R4) {
        let (x, w) = (PAD, W_DIP - 2.0 * PAD);
        let by = PAD + 138.0;
        (
            (x, PAD, w, 28.0),
            (x, PAD + 34.0, w, 60.0),
            (x, PAD + 98.0, w, 32.0),
            (x + w - 228.0, by, 110.0, 34.0),
            (x + w - 110.0, by, 110.0, 34.0),
        )
    }

    fn power_rect(r: R4) -> R4 {
        (r.0 + r.2 - 36.0, r.1 + 2.0, 34.0, 24.0)
    }

    fn hit(&self, core: &AppCore, x: f32, y: f32) -> Option<Hit> {
        if self.confirm.is_some() {
            let (_, _, chk, yes, no) = Self::confirm_layout();
            return if inside(yes, x, y) {
                Some(Hit::ConfirmYes)
            } else if inside(no, x, y) {
                Some(Hit::ConfirmNo)
            } else if inside(chk, x, y) {
                Some(Hit::ConfirmCheck)
            } else {
                None
            };
        }
        for (kind, r) in self.rows(core).0 {
            match kind {
                RowKind::Monitor(i) => {
                    if core.settings.show_power_control && self.monitors[i].supports_power() && inside(Self::power_rect(r), x, y) {
                        return Some(Hit::MonPower(i));
                    }
                    if inside(r, x, y) && y >= r.1 + 20.0 {
                        return Some(Hit::MonSlider(i));
                    }
                }
                RowKind::Dim if inside(r, x, y) && y >= r.1 + 20.0 => return Some(Hit::DimSlider),
                RowKind::Night if inside(r, x, y) => return Some(Hit::Night),
                RowKind::Settings if inside(r, x, y) => return Some(Hit::Settings),
                _ => {}
            }
        }
        None
    }

    fn row_rect(&self, core: &AppCore, hit: Hit) -> Option<R4> {
        self.rows(core).0.into_iter().find_map(|(kind, r)| match (kind, hit) {
            (RowKind::Monitor(i), Hit::MonSlider(j)) if i == j => Some(r),
            (RowKind::Dim, Hit::DimSlider) => Some(r),
            _ => None,
        })
    }

    fn slider_value(r: R4, x: f32, max: i32) -> i32 {
        let (x0, x1) = (r.0 + 14.0, r.0 + r.2 - 14.0);
        (((x - x0) / (x1 - x0)).clamp(0.0, 1.0) * max as f32).round() as i32
    }

    // ---------------------------------------------------------------------------------------------
    // Input
    // ---------------------------------------------------------------------------------------------

    fn to_dip(&self, x: i32, y: i32) -> (f32, f32) {
        (x as f32 / self.k(), y as f32 / self.k())
    }

    fn drag_to(&mut self, core: &mut AppCore, hit: Hit, x: f32) -> Fx {
        let mut fx = Fx::default();
        let Some(r) = self.row_rect(core, hit) else { return fx };
        match hit {
            Hit::MonSlider(i) => {
                if let Some(m) = self.monitors.get_mut(i) {
                    m.request_brightness(Self::slider_value(r, x, 100));
                }
            }
            Hit::DimSlider => {
                fx.state_changed = core.dim.set_level(&mut core.settings, Self::slider_value(r, x, MAX_LEVEL));
            }
            _ => {}
        }
        self.invalidate();
        fx
    }

    pub fn mouse_down(&mut self, core: &mut AppCore, px: i32, py: i32) -> Fx {
        let (x, y) = self.to_dip(px, py);
        let mut fx = Fx::default();
        match self.hit(core, x, y) {
            Some(h @ (Hit::MonSlider(_) | Hit::DimSlider)) => {
                self.drag = Some(h);
                unsafe {
                    SetCapture(self.hwnd);
                }
                fx = self.drag_to(core, h, x);
            }
            Some(Hit::MonPower(i)) => {
                if core.settings.suppress_power_warning {
                    self.power_off(i);
                } else {
                    self.confirm = Some(Confirm { mon: i, dont_ask: false });
                    self.hover = None;
                    self.fit(core);
                    self.invalidate();
                }
            }
            Some(Hit::Night) => {
                if !core.settings.night_enabled && !core.settings.has_location {
                    self.hide();
                    fx.open_settings = true;
                } else {
                    let on = !core.settings.night_enabled;
                    fx.state_changed = core.dim.set_night(&mut core.settings, on);
                    self.invalidate();
                }
            }
            Some(Hit::Settings) => {
                self.hide();
                fx.open_settings = true;
            }
            Some(Hit::ConfirmCheck) => {
                if let Some(c) = &mut self.confirm {
                    c.dont_ask = !c.dont_ask;
                }
                self.invalidate();
            }
            Some(Hit::ConfirmYes) => {
                if let Some(c) = self.confirm.take() {
                    if c.dont_ask {
                        core.settings.suppress_power_warning = true;
                        core.settings.save();
                    }
                    self.power_off(c.mon);
                }
            }
            Some(Hit::ConfirmNo) => {
                self.confirm = None;
                self.hover = None;
                self.fit(core);
                self.invalidate();
            }
            None => {}
        }
        fx
    }

    fn power_off(&mut self, i: usize) {
        if let Some(m) = self.monitors.get(i) {
            m.set_power(4); // DPM off
        }
        self.hide();
    }

    pub fn mouse_move(&mut self, core: &mut AppCore, px: i32, py: i32) -> Fx {
        let (x, y) = self.to_dip(px, py);
        if let Some(h) = self.drag {
            return self.drag_to(core, h, x);
        }
        let h = self.hit(core, x, y);
        if h != self.hover {
            self.hover = h;
            let cursor = if matches!(h, Some(Hit::MonPower(_) | Hit::Night | Hit::Settings | Hit::ConfirmCheck | Hit::ConfirmYes | Hit::ConfirmNo)) {
                IDC_HAND
            } else {
                IDC_ARROW
            };
            unsafe {
                if let Ok(c) = LoadCursorW(None, cursor) {
                    SetCursor(Some(c));
                }
            }
            self.invalidate();
        }
        if !self.tracking {
            self.tracking = true;
            let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: self.hwnd, dwHoverTime: 0 };
            unsafe {
                let _ = TrackMouseEvent(&mut tme);
            }
        }
        Fx::default()
    }

    pub fn mouse_up(&mut self) {
        if self.drag.take().is_some() {
            unsafe {
                let _ = ReleaseCapture();
            }
        }
        self.invalidate();
    }

    pub fn mouse_leave(&mut self) {
        self.tracking = false;
        if self.hover.take().is_some() {
            self.invalidate();
        }
    }

    pub fn mouse_wheel(&mut self, core: &mut AppCore, sx: i32, sy: i32, delta: i16) -> Fx {
        let mut fx = Fx::default();
        if self.confirm.is_some() {
            return fx;
        }
        let mut pt = POINT { x: sx, y: sy };
        unsafe {
            let _ = ScreenToClient(self.hwnd, &mut pt);
        }
        let (x, y) = self.to_dip(pt.x, pt.y);
        let step = if delta > 0 { 5 } else { -5 };
        for (kind, r) in self.rows(core).0 {
            if !inside(r, x, y) {
                continue;
            }
            match kind {
                RowKind::Monitor(i) => {
                    let m = &mut self.monitors[i];
                    let p = m.percent + step;
                    m.request_brightness(p);
                }
                RowKind::Dim => {
                    let v = core.dim.level() + step;
                    fx.state_changed = core.dim.set_level(&mut core.settings, v);
                }
                _ => {}
            }
            self.invalidate();
            break;
        }
        fx
    }

    // ---------------------------------------------------------------------------------------------
    // Monitor detection
    // ---------------------------------------------------------------------------------------------

    /// Rescans displays on a worker thread; the result arrives as WM_MONITORS_DONE on the main window.
    pub fn begin_detect(&mut self, main: HWND) {
        self.token = self.token.wrapping_add(1);
        let (token, main) = (self.token, main.0 as isize);
        std::thread::spawn(move || {
            let found = monitors::enumerate();
            *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((token, found));
            unsafe {
                let _ = PostMessageW(Some(HWND(main as *mut _)), WM_MONITORS_DONE, WPARAM(0), LPARAM(0));
            }
        });
    }

    pub fn monitors_done(&mut self, core: &AppCore) {
        let Some((token, found)) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        // Stale scan, or the user is mid-drag / mid-confirmation: drop the result (dropping releases the handles).
        if token != self.token || self.drag.is_some() || self.confirm.is_some() {
            return;
        }
        self.monitors = found;
        self.detecting = false;
        if self.is_visible() {
            self.fit(core);
            self.invalidate();
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Painting
    // ---------------------------------------------------------------------------------------------

    fn ensure_fmts(&mut self) -> windows::core::Result<()> {
        if self.fmts.is_some() {
            return Ok(());
        }
        let dw = &self.dwrite;
        let f = |size, weight, center, vc| make_format(dw, w!("Segoe UI"), size, weight, center, vc);
        let right = f(13.0, DWRITE_FONT_WEIGHT_NORMAL, false, true)?;
        unsafe { right.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)? };
        let wrap = f(13.0, DWRITE_FONT_WEIGHT_NORMAL, false, false)?;
        unsafe { wrap.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)? };
        self.fmts = Some(Fmts {
            body: f(13.0, DWRITE_FONT_WEIGHT_NORMAL, false, true)?,
            right,
            title: f(16.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, true)?,
            center: f(13.0, DWRITE_FONT_WEIGHT_NORMAL, true, true)?,
            center_bold: f(13.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, true, true)?,
            wrap,
            icon: make_format(dw, icon_family(dw), 16.0, DWRITE_FONT_WEIGHT_NORMAL, true, true)?,
        });
        Ok(())
    }

    pub fn paint(&mut self, core: &AppCore, hdc: HDC) {
        let mut cr = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut cr);
        }
        let (w, h) = (cr.right.max(1), cr.bottom.max(1));
        if self.surf.ensure(w, h).is_err() || self.ensure_fmts().is_err() || self.surf.bind(w, h).is_err() {
            self.surf.reset_target();
            return;
        }
        let rt = self.surf.rt.clone().unwrap();
        let br = self.surf.brush.clone().unwrap();
        let k = self.k();
        unsafe {
            rt.BeginDraw();
            rt.SetTransform(&Matrix3x2 { M11: k, M12: 0.0, M21: 0.0, M22: k, M31: 0.0, M32: 0.0 });
            if self.acrylic {
                // Transparent: DWM paints the acrylic. A faint dark wash keeps text legible over bright wallpapers.
                rt.Clear(Some(&CLEAR));
            } else {
                rt.Clear(Some(&col((32, 32, 36), 255)));
            }
        }
        let cv = Canvas { rt: &rt, br: &br };
        let (wd, hd) = (w as f32 / k, h as f32 / k);
        if self.acrylic {
            
        }
        if self.confirm.is_some() {
            self.draw_confirm(&cv);
        } else {
            self.draw_rows(core, &cv);
        }
        cv.stroke_rr((0.0, 0.0, wd, hd), 8.0, col(WHITE, 34));
        unsafe {
            if rt.EndDraw(None, None).is_err() {
                self.surf.reset_target();
                return;
            }
        }
        self.surf.present(hdc, w, h);
    }

    fn draw_rows(&self, core: &AppCore, cv: &Canvas) {
        let f = self.fmts.as_ref().unwrap();
        let primary = col(WHITE, 235);
        let secondary = col(WHITE, 150);

        for (kind, r) in self.rows(core).0 {
            match kind {
                RowKind::Detecting => cv.txt("Detecting displays...", &f.body, (r.0 + 6.0, r.1, r.2 - 12.0, r.3), secondary),
                RowKind::NoMonitors => cv.txt("No controllable displays found (DDC/CI).", &f.body, (r.0 + 6.0, r.1, r.2 - 12.0, r.3), secondary),
                RowKind::Monitor(i) => {
                    let m = &self.monitors[i];
                    let power = core.settings.show_power_control && m.supports_power();
                    let power_hover = self.hover == Some(Hit::MonPower(i));
                    let active = matches!(self.hover, Some(Hit::MonSlider(j)) if j == i) || self.drag == Some(Hit::MonSlider(i));
                    self.draw_slider(cv, r, &m.name, G_MONITOR, m.percent, 100, power.then_some(G_POWER), power_hover, active);
                }
                RowKind::Dim => {
                    let active = self.hover == Some(Hit::DimSlider) || self.drag == Some(Hit::DimSlider);
                    self.draw_slider(cv, r, "Extra dimming", G_BRIGHTNESS, core.dim.effective_level(&core.settings), MAX_LEVEL, None, false, active);
                }
                RowKind::Night => {
                    let on = core.settings.night_enabled;
                    let hov = self.hover == Some(Hit::Night);
                    if hov {
                        cv.fill_rr((r.0, r.1 + 2.0, r.2, r.3 - 4.0), 6.0, col(WHITE, 16));
                    }
                    cv.txt(G_MOON, &f.icon, (r.0 + 4.0, r.1, 26.0, r.3), primary);
                    let (sw_w, sw_h) = (40.0, 20.0);
                    let (sw_x, sw_y) = (r.0 + r.2 - sw_w - 12.0, r.1 + (r.3 - sw_h) / 2.0);
                    cv.txt("Night mode", &f.body, (r.0 + 36.0, r.1, sw_x - r.0 - 36.0 - 70.0, r.3), primary);
                    let status = if on { format!("{}K", core.dim.night_kelvin(&core.settings) as i32) } else { "Off".into() };
                    cv.txt(&status, &f.right, (sw_x - 70.0, r.1, 62.0, r.3), secondary);
                    let sw = (sw_x, sw_y, sw_w, sw_h);
                    if on {
                        cv.fill_rr(sw, 10.0, col(ACCENT, if hov { 230 } else { 255 }));
                        cv.circle(sw_x + sw_w - 10.0, sw_y + sw_h / 2.0, if hov { 7.0 } else { 6.0 }, col(BLACK, 215));
                    } else {
                        cv.fill_rr(sw, 10.0, col(WHITE, if hov { 14 } else { 4 }));
                        cv.stroke_rr(sw, 10.0, col(WHITE, 140));
                        cv.circle(sw_x + 10.0, sw_y + sw_h / 2.0, if hov { 7.0 } else { 6.0 }, col(WHITE, 200));
                    }
                }
                RowKind::Settings => {
                    if self.hover == Some(Hit::Settings) {
                        cv.fill_rr((r.0, r.1 + 2.0, r.2, r.3 - 4.0), 6.0, col(WHITE, 16));
                    }
                    cv.txt(G_SETTINGS, &f.icon, (r.0 + 4.0, r.1, 26.0, r.3), primary);
                    cv.txt("Settings...", &f.body, (r.0 + 36.0, r.1, r.2 - 36.0, r.3), primary);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_slider(&self, cv: &Canvas, r: R4, caption: &str, glyph: &str, value: i32, max: i32, action: Option<&str>, action_hover: bool, active: bool) {
        let f = self.fmts.as_ref().unwrap();
        let primary = col(WHITE, 235);
        let secondary = col(WHITE, 150);
        let top_h = 28.0;
        let action_w = if action.is_some() { 36.0 } else { 0.0 };
        let value_w = 52.0;

        cv.txt(glyph, &f.icon, (r.0 + 4.0, r.1, 26.0, top_h), primary);
        cv.txt(caption, &f.body, (r.0 + 36.0, r.1, r.2 - 36.0 - action_w - value_w, top_h), primary);
        cv.txt(&format!("{value}%"), &f.right, (r.0 + r.2 - action_w - value_w, r.1, value_w - 4.0, top_h), secondary);

        if let Some(g) = action {
            let ar = Self::power_rect(r);
            if action_hover {
                cv.fill_rr(ar, 6.0, col(WHITE, 34));
            }
            cv.txt(g, &f.icon, ar, primary);
        }

        let (x0, x1) = (r.0 + 14.0, r.0 + r.2 - 14.0);
        let ty = r.1 + 42.0;
        let t = (value as f32 / max as f32).clamp(0.0, 1.0);
        let tx = x0 + t * (x1 - x0);
        cv.fill_rr((x0, ty - 2.0, x1 - x0, 4.0), 2.0, col(WHITE, 70));
        if tx > x0 {
            cv.fill_rr((x0, ty - 2.0, tx - x0, 4.0), 2.0, col(ACCENT, 255));
        }
        let dragging = matches!(self.drag, Some(Hit::MonSlider(_) | Hit::DimSlider)) && active;
        cv.circle(tx, ty, if active { 11.0 } else { 10.0 }, col((240, 240, 245), 255));
        cv.circle(tx, ty, if dragging { 4.0 } else if active { 7.0 } else { 6.0 }, col(ACCENT, 255));
    }

    fn draw_confirm(&self, cv: &Canvas) {
        let f = self.fmts.as_ref().unwrap();
        let primary = col(WHITE, 235);
        let secondary = col(WHITE, 170);
        let (title, body, chk, yes, no) = Self::confirm_layout();
        let dont_ask = self.confirm.as_ref().is_some_and(|c| c.dont_ask);

        cv.txt("Turn off this display?", &f.title, title, primary);
        cv.txt(
            "This puts the display into DPM power-save mode. You can turn it back on with the monitor's own power button.",
            &f.wrap,
            body,
            secondary,
        );

        // checkbox
        if self.hover == Some(Hit::ConfirmCheck) {
            cv.fill_rr((chk.0, chk.1 + 2.0, chk.2, chk.3 - 4.0), 6.0, col(WHITE, 16));
        }
        let bx = (chk.0 + 6.0, chk.1 + (chk.3 - 18.0) / 2.0, 18.0, 18.0);
        if dont_ask {
            cv.fill_rr(bx, 4.0, col(ACCENT, 255));
            cv.txt("\u{E73E}", &f.icon, (bx.0, bx.1, bx.2, bx.3), col(BLACK, 235)); // check mark
        } else {
            cv.fill_rr(bx, 4.0, col(WHITE, 8));
            cv.stroke_rr(bx, 4.0, col(WHITE, 140));
        }
        cv.txt("Don't show this warning again", &f.body, (chk.0 + 34.0, chk.1, chk.2 - 34.0, chk.3), primary);

        let yes_hover = self.hover == Some(Hit::ConfirmYes);
        cv.fill_rr(yes, 5.0, col(ACCENT, if yes_hover { 230 } else { 255 }));
        cv.txt("Turn off", &f.center_bold, yes, col(BLACK, 235));
        let no_hover = self.hover == Some(Hit::ConfirmNo);
        cv.fill_rr(no, 5.0, col(WHITE, if no_hover { 26 } else { 15 }));
        cv.stroke_rr(no, 5.0, col(WHITE, 22));
        cv.txt("Cancel", &f.center, no, primary);
    }
}

impl Drop for Flyout {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

extern "system" fn flyout_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    use crate::window::{apply_fx, with_app};
    unsafe {
        let x = (lp.0 & 0xffff) as i16 as i32;
        let y = ((lp.0 >> 16) & 0xffff) as i16 as i32;
        match msg {
            WM_ERASEBKGND => return LRESULT(1),
            WM_MOUSEACTIVATE => return LRESULT(MA_ACTIVATE as isize),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let painted = with_app(|a| a.flyout.paint(&a.core, hdc));
                let _ = EndPaint(hwnd, &ps);
                if painted.is_none() {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                return LRESULT(0);
            }
            WM_ACTIVATE => {
                if (wp.0 & 0xffff) as u32 == WA_INACTIVE && !PINNED.load(std::sync::atomic::Ordering::Relaxed) {
                    with_app(|a| a.flyout.hide());
                }
                return LRESULT(0);
            }
            WM_KEYDOWN if wp.0 as u16 == VK_ESCAPE.0 => {
                with_app(|a| a.flyout.hide());
                return LRESULT(0);
            }
            WM_MOUSEMOVE => {
                if let Some(fx) = with_app(|a| a.flyout.mouse_move(&mut a.core, x, y)) {
                    apply_fx(fx);
                }
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                with_app(|a| a.flyout.mouse_leave());
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                if let Some(fx) = with_app(|a| a.flyout.mouse_down(&mut a.core, x, y)) {
                    apply_fx(fx);
                }
                return LRESULT(0);
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED => {
                with_app(|a| a.flyout.mouse_up());
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp.0 >> 16) & 0xffff) as i16;
                if let Some(fx) = with_app(|a| a.flyout.mouse_wheel(&mut a.core, x, y, delta)) {
                    apply_fx(fx);
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                let dpi = (wp.0 & 0xffff) as u32;
                with_app(|a| a.flyout.dpi_changed(dpi, &a.core));
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}
