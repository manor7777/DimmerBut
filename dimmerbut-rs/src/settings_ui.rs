//! Settings window: a modern Windows 11 style page, custom drawn with Direct2D on a Mica backdrop
//! (sidebar navigation, card sections, toggle switches, sliders, steppers, hotkey recorders).
//! Only the text inputs are native EDIT controls, placed inside painted rounded fields.

use crate::geocode::{self, GeoResult};
use crate::gfx::Surface;
use crate::settings::{self, AppSettings, MAX_KELVIN, MAX_LEVEL, MIN_KELVIN, STEP, format_hotkey};
use crate::ui::*;
use crate::{autostart, message_box, wide};
use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};
use windows_numerics::Matrix3x2;

const CLIENT_W: f32 = 720.0;
const CLIENT_H: f32 = 580.0;
const FOOTER_Y: f32 = 520.0;
const FIELD_BG: Rgb = (50, 50, 54);

// layout of the content column
const CX: f32 = 214.0;
const CW: f32 = 478.0;
const RX: f32 = CX + CW - 16.0; // right edge for row controls

const WM_GEO_DONE: u32 = WM_APP + 20;
const ID_EDIT: i32 = 500;

// pages
const P_DIM: usize = 0;
const P_NIGHT: usize = 1;
const P_LOC: usize = 2;
const P_FLYOUT: usize = 3;
const P_KEYS: usize = 4;

// toggles
const T_DIM: usize = 0;
const T_START: usize = 1;
const T_NIGHT: usize = 2;
const T_HW: usize = 3;
const T_POWER: usize = 4;
const T_WARN: usize = 5;
const T_POLL: usize = 6;
const T_ACRYLIC: usize = 7;
const T_KEYS: usize = 8;
const TOGGLES: usize = 9;

// numeric boxes
const N_KDAY: usize = 0;
const N_KSUN: usize = 1;
const N_KBED: usize = 2;
const N_WAKE: usize = 3;
const N_TRANS: usize = 4;
const N_LAT: usize = 5;
const N_LON: usize = 6;
const N_POLL: usize = 7;

const PRESETS: [(i32, &str); 8] = [
    (1200, "Ember"),
    (1900, "Candle"),
    (2300, "Dim incandescent"),
    (2700, "Incandescent"),
    (3400, "Halogen"),
    (4200, "Fluorescent"),
    (5500, "Sunlight"),
    (6500, "Daylight"),
];

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Save,
    Cancel,
    Find,
    ClearKey(usize),
}

#[derive(Clone, Copy)]
enum Status {
    Kelvin(usize),
    Location,
}

enum Kind {
    SidebarTitle,
    Nav(usize, &'static str, &'static str),
    Title(&'static str),
    Heading(&'static str),
    Hint(&'static str),
    Card,
    Divider,
    Label(&'static str, &'static str),
    Toggle(usize),
    Slider,
    Button(&'static str, bool, Act),
    Stepper(usize),
    Field,
    Hotkey(usize),
    Status(Status),
}

struct Item {
    page: Option<usize>,
    r: R4,
    kind: Kind,
    /// Toggles that must all be on for the item to be usable.
    gate: [Option<usize>; 2],
}

struct EditRef {
    hwnd: HWND,
    item: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Fmt {
    Num(usize),
    Time,
}

struct NumBox {
    edit: usize,
    min: f64,
    max: f64,
    inc: f64,
    fmt: Fmt,
    value: f64,
}

struct Fmts {
    body: IDWriteTextFormat,
    body_v: IDWriteTextFormat,
    right: IDWriteTextFormat,
    caption: IDWriteTextFormat,
    wrap: IDWriteTextFormat,
    title: IDWriteTextFormat,
    heading: IDWriteTextFormat,
    sidebar_title: IDWriteTextFormat,
    center: IDWriteTextFormat,
    center_bold: IDWriteTextFormat,
    icon: IDWriteTextFormat,
    glyph: IDWriteTextFormat,
}

type GeoSlot = Arc<Mutex<Option<Result<Option<GeoResult>, String>>>>;

struct Dlg {
    hwnd: HWND,
    dpi: u32,
    font: HFONT,
    edit_brush: HBRUSH,
    mica: bool,

    surf: Surface,
    dwrite: IDWriteFactory,
    fmts: Option<Fmts>,

    settings: AppSettings,

    items: Vec<Item>,
    page: usize,
    hover: Option<(usize, u8)>,
    pressed: Option<(usize, u8)>,
    dragging: bool,
    tracking: bool,
    focus_edit: Option<HWND>,

    edits: Vec<EditRef>,
    city_edit: usize,
    nums: Vec<NumBox>,
    toggles: [bool; TOGGLES],
    startup_level: i32,
    hotkeys: [(u32, i32); 2],
    capturing: Option<usize>,

    has_loc: bool,
    loc_name: String,
    loc_message: Option<String>,
    geo: GeoSlot,
    searching: bool,
    suppress: bool,

    done: bool,
    accepted: bool,
}

/// Shows the dialog modally. Returns the edited settings if the user pressed Save.
pub fn show(owner: Option<HWND>, settings: AppSettings) -> Option<AppSettings> {
    unsafe {
        let hi = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        static CLASS: OnceLock<()> = OnceLock::new();
        CLASS.get_or_init(|| {
            let icon = LoadIconW(Some(hi), PCWSTR(1 as *const u16)).unwrap_or_default();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(dlg_proc),
                hInstance: hi,
                hIcon: icon,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("DimmerButSettings"),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let ex = if owner.is_none() { WS_EX_TOPMOST } else { WINDOW_EX_STYLE(0) };
        let Ok(hwnd) = CreateWindowExW(ex, w!("DimmerButSettings"), w!("DimmerBut Settings"), style, 0, 0, 100, 100, owner, None, Some(hi), None)
        else {
            return None;
        };
        let dpi = GetDpiForWindow(hwnd).max(96);

        let (Ok(surf), Ok(dwrite)) = (Surface::new(), DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)) else {
            let _ = DestroyWindow(hwnd);
            return None;
        };

        let mut dlg = Box::new(Dlg {
            hwnd,
            dpi,
            font: make_font(14, dpi),
            edit_brush: CreateSolidBrush(colorref(FIELD_BG)),
            mica: false,
            surf,
            dwrite,
            fmts: None,
            settings,
            items: Vec::new(),
            page: 0,
            hover: None,
            pressed: None,
            dragging: false,
            tracking: false,
            focus_edit: None,
            edits: Vec::new(),
            city_edit: 0,
            nums: Vec::new(),
            toggles: [false; TOGGLES],
            startup_level: 0,
            hotkeys: [(0, 0); 2],
            capturing: None,
            has_loc: false,
            loc_name: String::new(),
            loc_message: None,
            geo: Arc::new(Mutex::new(None)),
            searching: false,
            suppress: true,
            done: false,
            accepted: false,
        });
        let ptr: *mut Dlg = &mut *dlg;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);

        // Dark title bar + Mica backdrop (falls back to a solid background where unsupported).
        (*ptr).mica = backdrop_supported() && apply_backdrop(hwnd, 2, true);
        if !(*ptr).mica {
            apply_backdrop(hwnd, 1, true);
        }

        (*ptr).build();

        let (cw, ch) = ((*ptr).px(CLIENT_W), (*ptr).px(CLIENT_H));
        let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, ex, dpi);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        // Centre on the work area of the monitor under the cursor (never partly off-screen).
        let mut cur = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut cur);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let mon = MonitorFromPoint(cur, MONITOR_DEFAULTTONEAREST);
        let area = if GetMonitorInfoW(mon, &mut mi).as_bool() {
            mi.rcWork
        } else {
            RECT { left: 0, top: 0, right: GetSystemMetrics(SM_CXSCREEN), bottom: GetSystemMetrics(SM_CYSCREEN) }
        };
        let x = (area.left + ((area.right - area.left) - ww) / 2).clamp(area.left, (area.right - ww).max(area.left));
        let y = (area.top + ((area.bottom - area.top) - wh) / 2).clamp(area.top, (area.bottom - wh).max(area.top));
        let _ = SetWindowPos(hwnd, None, x, y, ww, wh, SWP_NOZORDER | SWP_NOACTIVATE);
        (*ptr).place_edits();
        (*ptr).show_page_edits();
        (*ptr).refresh_gates();

        if let Some(o) = owner {
            let _ = EnableWindow(o, false);
        }
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while !(*ptr).done && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // While recording a hotkey every key belongs to the recorder (Esc/Enter/Tab included).
            if (*ptr).capturing.is_some() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                continue;
            }
            if msg.message == WM_KEYDOWN && GetAncestor(msg.hwnd, GA_ROOT) == hwnd {
                if msg.wParam.0 as u16 == VK_ESCAPE.0 {
                    (*ptr).done = true;
                    continue;
                }
                if msg.wParam.0 as u16 == VK_RETURN.0 {
                    if (*ptr).focus_edit == Some(msg.hwnd) && (&(*ptr).edits)[(*ptr).city_edit].hwnd == msg.hwnd {
                        (*ptr).find();
                    } else {
                        (*ptr).save();
                    }
                    continue;
                }
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if let Some(o) = owner {
            let _ = EnableWindow(o, true);
            let _ = SetForegroundWindow(o);
        }
        let _ = DestroyWindow(hwnd);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);

        let d = &mut *ptr;
        let _ = DeleteObject(d.font.into());
        let _ = DeleteObject(d.edit_brush.into());
        if d.accepted { Some(d.settings.clone()) } else { None }
    }
}

fn colorref(c: Rgb) -> COLORREF {
    COLORREF(c.0 as u32 | (c.1 as u32) << 8 | (c.2 as u32) << 16)
}

fn make_font(px: i32, dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -(px * dpi as i32 / 96),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

fn text_of(h: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(h);
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..got as usize])
    }
}

fn set_text(h: HWND, s: &str) {
    let t = wide(s);
    unsafe {
        let _ = SetWindowTextW(h, PCWSTR(t.as_ptr()));
    }
}

fn fmt_value(v: f64, f: Fmt) -> String {
    match f {
        Fmt::Num(d) => format!("{v:.d$}"),
        Fmt::Time => format!("{:02}:{:02}", (v as i32) / 60, (v as i32) % 60),
    }
}

fn parse_value(t: &str, f: Fmt) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    match f {
        Fmt::Num(_) => t.parse::<f64>().ok().filter(|v| v.is_finite()),
        Fmt::Time => {
            let (h, m) = match t.split_once(':') {
                Some((h, m)) => (h.trim().parse::<i32>().ok()?, m.trim().parse::<i32>().ok()?),
                None => {
                    let n = t.parse::<i32>().ok()?;
                    if n < 100 { (n, 0) } else { (n / 100, n % 100) }
                }
            };
            ((0..24).contains(&h) && (0..60).contains(&m)).then_some((h * 60 + m) as f64)
        }
    }
}

fn preset_name(k: i32) -> String {
    let best = PRESETS.iter().min_by_key(|p| (p.0 - k).abs()).unwrap();
    format!("~ {}", best.1)
}

impl Dlg {
    fn px(&self, dip: f32) -> i32 {
        (dip * self.dpi as f32 / 96.0).round() as i32
    }

    // ---------------------------------------------------------------------------------------------
    // Construction
    // ---------------------------------------------------------------------------------------------

    fn push(&mut self, page: Option<usize>, r: R4, kind: Kind) -> usize {
        self.items.push(Item { page, r, kind, gate: [None; 2] });
        self.items.len() - 1
    }

    fn gate(&mut self, item: usize, g: &[usize]) {
        for (i, t) in g.iter().take(2).enumerate() {
            self.items[item].gate[i] = Some(*t);
        }
    }

    /// Title + optional caption on the left of a 52px row starting at `y`.
    fn row_label(&mut self, page: usize, y: f32, title: &'static str, sub: &'static str) -> usize {
        let ty = if sub.is_empty() { y + 16.0 } else { y + 7.0 };
        self.push(Some(page), (CX + 16.0, ty, 200.0, 40.0), Kind::Label(title, sub))
    }

    fn divider(&mut self, page: usize, y: f32) {
        self.push(Some(page), (CX + 16.0, y, CW - 32.0, 1.0), Kind::Divider);
    }

    fn toggle_row(&mut self, page: usize, y: f32, t: usize, title: &'static str, sub: &'static str, gate: &[usize]) {
        let l = self.row_label(page, y, title, sub);
        // toggle rows have room for long captions
        self.items[l].r.2 = 360.0;
        let it = self.push(Some(page), (RX - 40.0, y + 16.0, 40.0, 20.0), Kind::Toggle(t));
        self.gate(l, gate);
        self.gate(it, gate);
    }

    fn add_edit(&mut self, item: usize, center: bool, text: &str) -> usize {
        let style = WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32 | if center { ES_CENTER as u32 } else { 0 };
        let t = wide(text);
        let idx = self.edits.len();
        unsafe {
            let hi = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                PCWSTR(t.as_ptr()),
                WINDOW_STYLE(style),
                0,
                0,
                10,
                10,
                Some(self.hwnd),
                Some(HMENU((ID_EDIT + idx as i32) as usize as *mut _)),
                Some(hi),
                None,
            )
            .unwrap_or_default();
            SendMessageW(h, WM_SETFONT, Some(WPARAM(self.font.0 as usize)), Some(LPARAM(1)));
            // GDI paints with alpha 0, which on the Mica (glass) surface would *add* to the backdrop instead
            // of covering it. A layered child with constant alpha 255 composites as fully opaque.
            SetWindowLongPtrW(h, GWL_EXSTYLE, GetWindowLongPtrW(h, GWL_EXSTYLE) | WS_EX_LAYERED.0 as isize);
            let _ = SetLayeredWindowAttributes(h, COLORREF(0), 255, LWA_ALPHA);
            self.edits.push(EditRef { hwnd: h, item });
        }
        idx
    }

    fn stepper(&mut self, page: usize, x: f32, y: f32, w_: f32, n: (f64, f64, f64, Fmt), value: f64) -> usize {
        let idx = self.nums.len();
        let item = self.push(Some(page), (x, y, w_, 32.0), Kind::Stepper(idx));
        let (min, max, inc, fmt) = n;
        let value = value.clamp(min, max);
        let edit = self.add_edit(item, true, &fmt_value(value, fmt));
        self.nums.push(NumBox { edit, min, max, inc, fmt, value });
        item
    }

    fn build(&mut self) {
        let s = self.settings.clone();
        self.toggles[T_DIM] = s.dimming_enabled;
        self.toggles[T_START] = autostart::is_enabled();
        self.toggles[T_NIGHT] = s.night_enabled;
        self.toggles[T_HW] = s.show_hardware_brightness;
        self.toggles[T_POWER] = s.show_power_control;
        self.toggles[T_WARN] = !s.suppress_power_warning;
        self.toggles[T_POLL] = s.auto_poll_monitors;
        self.toggles[T_ACRYLIC] = s.flyout_acrylic;
        self.toggles[T_KEYS] = s.hotkeys_enabled;
        self.startup_level = s.startup_level.clamp(0, MAX_LEVEL);
        self.hotkeys = [(s.increase_mods, s.increase_key), (s.decrease_mods, s.decrease_key)];
        self.has_loc = s.has_location;
        self.loc_name = s.location_name.clone();

        // --- chrome ---
        self.push(None, (24.0, 22.0, 150.0, 32.0), Kind::SidebarTitle);
        let nav = [("\u{E706}", "Dimming"), ("\u{E708}", "Night mode"), ("\u{E707}", "Location"), ("\u{E7F4}", "Flyout"), ("\u{E765}", "Hotkeys")];
        for (i, (icon, name)) in nav.iter().enumerate() {
            self.push(None, (12.0, 76.0 + i as f32 * 44.0, 166.0, 40.0), Kind::Nav(i, icon, name));
        }
        self.push(None, (CLIENT_W - 16.0 - 96.0, 534.0, 96.0, 32.0), Kind::Button("Save", true, Act::Save));
        self.push(None, (CLIENT_W - 16.0 - 96.0 * 2.0 - 8.0, 534.0, 96.0, 32.0), Kind::Button("Cancel", false, Act::Cancel));

        // --- Dimming ---
        self.push(Some(P_DIM), (CX, 24.0, CW, 36.0), Kind::Title("Dimming"));
        self.push(Some(P_DIM), (CX, 72.0, CW, 104.0), Kind::Card);
        self.toggle_row(P_DIM, 72.0, T_DIM, "Enable dimming", "Software dimming below your monitor's minimum", &[]);
        self.divider(P_DIM, 124.0);
        self.row_label(P_DIM, 124.0, "Startup level", "Applied when the app starts");
        self.push(Some(P_DIM), (RX - 250.0, 134.0, 250.0, 32.0), Kind::Slider);
        self.push(Some(P_DIM), (CX, 196.0, CW, 20.0), Kind::Heading("Startup"));
        self.push(Some(P_DIM), (CX, 220.0, CW, 56.0), Kind::Card);
        self.toggle_row(P_DIM, 220.0, T_START, "Start with Windows", "Launch DimmerBut automatically when you sign in", &[]);

        // --- Night mode ---
        self.push(Some(P_NIGHT), (CX, 24.0, CW, 36.0), Kind::Title("Night mode"));
        self.push(Some(P_NIGHT), (CX, 72.0, CW, 56.0), Kind::Card);
        self.toggle_row(P_NIGHT, 72.0, T_NIGHT, "Enable night mode", "f.lux-style colour tint that follows the sun", &[]);

        self.push(Some(P_NIGHT), (CX, 144.0, CW, 20.0), Kind::Heading("Colour temperature (Kelvin)"));
        self.push(Some(P_NIGHT), (CX, 168.0, CW, 156.0), Kind::Card);
        let kelvin = (MIN_KELVIN as f64, MAX_KELVIN as f64, 100.0, Fmt::Num(0));
        let rows = [("Daytime", "During the day", s.day_kelvin), ("Sunset", "From sunset", s.sunset_kelvin), ("Bedtime", "Before you sleep", s.bed_kelvin)];
        for (i, (title, sub, k)) in rows.iter().enumerate() {
            let y = 168.0 + i as f32 * 52.0;
            if i > 0 {
                self.divider(P_NIGHT, y);
            }
            self.row_label(P_NIGHT, y, title, sub);
            self.push(Some(P_NIGHT), (RX - 132.0 - 150.0, y + 10.0, 142.0, 32.0), Kind::Status(Status::Kelvin(i)));
            self.stepper(P_NIGHT, RX - 132.0, y + 10.0, 132.0, kelvin, *k as f64);
        }

        self.push(Some(P_NIGHT), (CX, 340.0, CW, 20.0), Kind::Heading("Schedule"));
        self.push(Some(P_NIGHT), (CX, 364.0, CW, 104.0), Kind::Card);
        self.row_label(P_NIGHT, 364.0, "Earliest wake time", "Colours ease back after this");
        self.stepper(P_NIGHT, RX - 132.0, 374.0, 132.0, (0.0, 1439.0, 5.0, Fmt::Time), s.wake_minutes as f64);
        self.divider(P_NIGHT, 416.0);
        self.row_label(P_NIGHT, 416.0, "Transition length", "Minutes to ease between colours");
        self.stepper(P_NIGHT, RX - 132.0, 426.0, 132.0, (1.0, 240.0, 5.0, Fmt::Num(0)), s.transition_minutes as f64);

        // --- Location ---
        self.push(Some(P_LOC), (CX, 24.0, CW, 36.0), Kind::Title("Location"));
        self.push(Some(P_LOC), (CX, 72.0, CW, 204.0), Kind::Card);
        self.row_label(P_LOC, 72.0, "City", "Look up coordinates");
        let f = self.push(Some(P_LOC), (RX - 80.0 - 240.0, 82.0, 240.0, 32.0), Kind::Field);
        self.city_edit = self.add_edit(f, false, &s.location_name);
        self.push(Some(P_LOC), (RX - 72.0, 82.0, 72.0, 32.0), Kind::Button("Find", false, Act::Find));
        self.divider(P_LOC, 124.0);
        self.push(Some(P_LOC), (CX + 16.0, 130.0, CW - 32.0, 36.0), Kind::Status(Status::Location));
        self.divider(P_LOC, 172.0);
        self.row_label(P_LOC, 172.0, "Latitude", "");
        self.stepper(P_LOC, RX - 168.0, 182.0, 168.0, (-90.0, 90.0, 0.1, Fmt::Num(4)), s.latitude);
        self.divider(P_LOC, 224.0);
        self.row_label(P_LOC, 224.0, "Longitude", "");
        self.stepper(P_LOC, RX - 168.0, 234.0, 168.0, (-180.0, 180.0, 0.1, Fmt::Num(4)), s.longitude);
        self.push(
            Some(P_LOC),
            (CX, 292.0, CW, 40.0),
            Kind::Hint("Only the city name is sent (to Open-Meteo, once). Sunrise and sunset are then calculated offline from the saved coordinates."),
        );

        // --- Flyout ---
        self.push(Some(P_FLYOUT), (CX, 24.0, CW, 36.0), Kind::Title("Flyout"));
        self.push(Some(P_FLYOUT), (CX, 72.0, CW, 260.0), Kind::Card);
        self.toggle_row(P_FLYOUT, 72.0, T_HW, "Monitor brightness sliders", "DDC/CI monitors and laptop panels", &[]);
        self.divider(P_FLYOUT, 124.0);
        self.toggle_row(P_FLYOUT, 124.0, T_POWER, "Power buttons", "Show a power-off button for each monitor", &[T_HW]);
        self.divider(P_FLYOUT, 176.0);
        self.toggle_row(P_FLYOUT, 176.0, T_WARN, "Warn before turning off", "Ask for confirmation first", &[T_HW, T_POWER]);
        self.divider(P_FLYOUT, 228.0);
        self.toggle_row(P_FLYOUT, 228.0, T_POLL, "Rescan monitors periodically", "Picks up displays that were power-cycled", &[T_HW]);
        self.divider(P_FLYOUT, 280.0);
        let l = self.row_label(P_FLYOUT, 280.0, "Rescan interval", "Seconds between scans");
        let st = self.stepper(P_FLYOUT, RX - 132.0, 290.0, 132.0, (5.0, 120.0, 5.0, Fmt::Num(0)), s.poll_interval_seconds as f64);
        self.gate(l, &[T_HW, T_POLL]);
        self.gate(st, &[T_HW, T_POLL]);
        self.push(Some(P_FLYOUT), (CX, 348.0, CW, 20.0), Kind::Heading("Appearance"));
        self.push(Some(P_FLYOUT), (CX, 372.0, CW, 56.0), Kind::Card);
        self.toggle_row(P_FLYOUT, 372.0, T_ACRYLIC, "Acrylic background", "Windows 11 blur behind the flyout", &[]);
        self.push(Some(P_FLYOUT), (CX, 444.0, CW, 20.0), Kind::Hint("Left-click the tray icon for the flyout; right-click for the menu."));

        // --- Hotkeys ---
        self.push(Some(P_KEYS), (CX, 24.0, CW, 36.0), Kind::Title("Hotkeys"));
        self.push(Some(P_KEYS), (CX, 72.0, CW, 156.0), Kind::Card);
        self.toggle_row(P_KEYS, 72.0, T_KEYS, "Enable hotkeys", "Change dimming from any app", &[]);
        self.divider(P_KEYS, 124.0);
        for (i, (title, sub)) in [("Increase dimming", "Dimmer by 10%"), ("Decrease dimming", "Brighter by 10%")].iter().enumerate() {
            let y = 124.0 + i as f32 * 52.0;
            if i > 0 {
                self.divider(P_KEYS, y);
            }
            let l = self.row_label(P_KEYS, y, title, sub);
            let hk = self.push(Some(P_KEYS), (RX - 72.0 - 8.0 - 190.0, y + 10.0, 190.0, 32.0), Kind::Hotkey(i));
            let b = self.push(Some(P_KEYS), (RX - 72.0, y + 10.0, 72.0, 32.0), Kind::Button("Clear", false, Act::ClearKey(i)));
            for it in [l, hk, b] {
                self.gate(it, &[T_KEYS]);
            }
        }
        self.push(Some(P_KEYS), (CX, 244.0, CW, 20.0), Kind::Hint("Click a box, then press a combination. Delete clears it."));

        self.suppress = false;
    }

    fn place_edits(&self) {
        for e in &self.edits {
            let it = &self.items[e.item];
            let (x, y, w_, _) = it.r;
            let r = match it.kind {
                Kind::Stepper(_) => (x + 6.0, y + 7.0, w_ - 56.0 - 12.0, 18.0),
                _ => (x + 10.0, y + 7.0, w_ - 20.0, 18.0),
            };
            unsafe {
                let _ = SetWindowPos(e.hwnd, None, self.px(r.0), self.px(r.1), self.px(r.2), self.px(r.3), SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }
    }

    fn show_page_edits(&self) {
        for e in &self.edits {
            let vis = self.items[e.item].page == Some(self.page);
            unsafe {
                let _ = ShowWindow(e.hwnd, if vis { SW_SHOWNA } else { SW_HIDE });
            }
        }
    }

    fn enabled(&self, it: &Item) -> bool {
        it.gate.iter().flatten().all(|g| self.toggles[*g])
    }

    fn refresh_gates(&self) {
        for e in &self.edits {
            unsafe {
                let _ = EnableWindow(e.hwnd, self.enabled(&self.items[e.item]));
            }
        }
    }

    fn set_page(&mut self, p: usize) {
        if p == self.page {
            return;
        }
        self.page = p;
        self.capturing = None;
        self.show_page_edits();
        unsafe {
            let _ = SetFocus(Some(self.hwnd));
        }
        self.invalidate();
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Numeric boxes
    // ---------------------------------------------------------------------------------------------

    fn set_value(&mut self, n: usize, v: f64) {
        let nb = &mut self.nums[n];
        let v = match nb.fmt {
            Fmt::Num(d) => {
                let p = 10f64.powi(d as i32);
                (v.clamp(nb.min, nb.max) * p).round() / p
            }
            Fmt::Time => v.clamp(nb.min, nb.max).round(),
        };
        let changed = (v - nb.value).abs() > f64::EPSILON;
        nb.value = v;
        set_text(self.edits[nb.edit].hwnd, &fmt_value(v, nb.fmt));
        if changed && !self.suppress && (n == N_LAT || n == N_LON) {
            self.has_loc = true;
            self.loc_name = "Custom coordinates".into();
            self.loc_message = None;
        }
    }

    /// Commits typed text (called when focus leaves a box and on Save).
    fn commit(&mut self, n: usize) {
        let nb = &self.nums[n];
        let v = parse_value(&text_of(self.edits[nb.edit].hwnd), nb.fmt).unwrap_or(nb.value);
        self.set_value(n, v);
    }

    fn step(&mut self, n: usize, dir: f64) {
        self.commit(n);
        let v = self.nums[n].value + dir * self.nums[n].inc;
        self.set_value(n, v);
    }

    // ---------------------------------------------------------------------------------------------
    // Location lookup
    // ---------------------------------------------------------------------------------------------

    fn find(&mut self) {
        let q = text_of(self.edits[self.city_edit].hwnd).trim().to_string();
        if q.is_empty() || self.searching {
            return;
        }
        self.searching = true;
        self.loc_message = Some("Searching...".into());
        self.invalidate();
        let (slot, hwnd) = (self.geo.clone(), self.hwnd.0 as isize);
        std::thread::spawn(move || {
            let r = geocode::find(&q);
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
            unsafe {
                let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_GEO_DONE, WPARAM(0), LPARAM(0));
            }
        });
    }

    fn geo_done(&mut self) {
        self.searching = false;
        let Some(r) = self.geo.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        match r {
            Ok(Some(g)) => {
                self.suppress = true;
                self.set_value(N_LAT, (g.latitude * 1e4).round() / 1e4);
                self.set_value(N_LON, (g.longitude * 1e4).round() / 1e4);
                self.suppress = false;
                self.has_loc = true;
                self.loc_name = g.display_name;
                self.loc_message = None;
            }
            Ok(None) => self.loc_message = Some("No match found. Try \"City, Country\" or enter coordinates.".into()),
            Err(_) => self.loc_message = Some("Lookup failed (offline?). Enter latitude and longitude manually.".into()),
        }
        self.invalidate();
    }

    fn location_text(&self) -> String {
        if let Some(m) = &self.loc_message {
            return m.clone();
        }
        if self.has_loc {
            format!("{}  ({:.4}, {:.4})", self.loc_name, self.nums[N_LAT].value, self.nums[N_LON].value)
        } else {
            "No location set. Type a city (e.g. \"Skegness, United Kingdom\") and press Find, or enter coordinates.".into()
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Actions
    // ---------------------------------------------------------------------------------------------

    fn save(&mut self) {
        for n in 0..self.nums.len() {
            self.commit(n); // commit any number the user typed but hasn't tabbed out of yet
        }
        if self.toggles[T_NIGHT] && !self.has_loc {
            self.set_page(P_LOC);
            message_box(
                Some(self.hwnd),
                "Night mode needs a location. Type a city and press Find, or enter coordinates.",
                "DimmerBut",
                MB_OK | MB_ICONINFORMATION,
            );
            return;
        }

        let t = self.toggles;
        let n = |i: usize| self.nums[i].value;
        let s = &mut self.settings;
        s.dimming_enabled = t[T_DIM];
        s.startup_level = self.startup_level;
        s.night_enabled = t[T_NIGHT];
        s.day_kelvin = n(N_KDAY) as i32;
        s.sunset_kelvin = n(N_KSUN) as i32;
        s.bed_kelvin = n(N_KBED) as i32;
        s.wake_minutes = n(N_WAKE) as i32;
        s.transition_minutes = n(N_TRANS) as i32;
        s.has_location = self.has_loc;
        s.location_name = self.loc_name.clone();
        s.latitude = n(N_LAT);
        s.longitude = n(N_LON);
        s.show_hardware_brightness = t[T_HW];
        s.show_power_control = t[T_POWER];
        s.suppress_power_warning = !t[T_WARN];
        s.auto_poll_monitors = t[T_POLL];
        s.poll_interval_seconds = n(N_POLL) as i32;
        s.flyout_acrylic = t[T_ACRYLIC];
        s.hotkeys_enabled = t[T_KEYS];
        (s.increase_mods, s.increase_key) = self.hotkeys[0];
        (s.decrease_mods, s.decrease_key) = self.hotkeys[1];

        if t[T_START] != autostart::is_enabled() && !autostart::set(t[T_START]) {
            message_box(Some(self.hwnd), "Windows would not let this app change its start-up setting.", "Start with Windows", MB_OK | MB_ICONWARNING);
        }
        self.accepted = true;
        self.done = true;
    }

    fn run(&mut self, act: Act) {
        match act {
            Act::Save => self.save(),
            Act::Cancel => self.done = true,
            Act::Find => self.find(),
            Act::ClearKey(i) => self.hotkeys[i] = (0, 0),
        }
    }

    /// Records a key press while a hotkey box is armed. Returns true if it was consumed.
    fn key_down(&mut self, vk: u32) -> bool {
        let Some(i) = self.capturing else { return false };
        if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5) {
            return true; // a bare modifier: keep waiting for the real key
        }
        let down = |k: VIRTUAL_KEY| unsafe { GetKeyState(k.0 as i32) < 0 };
        let mut mods = 0;
        if down(VK_CONTROL) {
            mods |= settings::MOD_CONTROL;
        }
        if down(VK_MENU) {
            mods |= settings::MOD_ALT;
        }
        if down(VK_SHIFT) {
            mods |= settings::MOD_SHIFT;
        }
        if mods == 0 {
            if matches!(vk, 0x2E | 0x08 | 0x1B) {
                self.hotkeys[i] = (0, 0);
                self.capturing = None;
            }
            // anything else without a modifier would hijack a plain key system-wide: ignore it
        } else {
            self.hotkeys[i] = (mods, vk as i32);
            self.capturing = None;
        }
        self.invalidate();
        true
    }

    // ---------------------------------------------------------------------------------------------
    // Mouse
    // ---------------------------------------------------------------------------------------------

    fn visible(&self, it: &Item) -> bool {
        it.page.is_none_or(|p| p == self.page)
    }

    /// Returns (item index, sub part). Sub: stepper 1 = minus / 2 = plus.
    fn hit(&self, x: f32, y: f32) -> Option<(usize, u8)> {
        for (i, it) in self.items.iter().enumerate().rev() {
            if !self.visible(it) || !self.enabled(it) {
                continue;
            }
            let (ix, iy, iw, ih) = it.r;
            match &it.kind {
                Kind::Toggle(_) if inside((ix - 6.0, iy - 6.0, iw + 12.0, ih + 12.0), x, y) => return Some((i, 0)),
                Kind::Button(..) | Kind::Nav(..) | Kind::Slider | Kind::Hotkey(_) if inside(it.r, x, y) => return Some((i, 0)),
                Kind::Stepper(_) => {
                    if inside((ix + iw - 54.0, iy, 26.0, ih), x, y) {
                        return Some((i, 1));
                    }
                    if inside((ix + iw - 26.0, iy, 26.0, ih), x, y) {
                        return Some((i, 2));
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn slider_from_x(&mut self, i: usize, x: f32) {
        let (ix, _, _, _) = self.items[i].r;
        let t = ((x - (ix + 10.0)) / 180.0).clamp(0.0, 1.0);
        let v = (t * MAX_LEVEL as f32 / STEP as f32).round() as i32 * STEP;
        if v != self.startup_level {
            self.startup_level = v;
            self.invalidate();
        }
    }

    fn mouse_move(&mut self, x: f32, y: f32) {
        if self.dragging {
            if let Some((i, _)) = self.pressed {
                self.slider_from_x(i, x);
            }
            return;
        }
        let h = self.hit(x, y);
        if h != self.hover {
            self.hover = h;
            self.invalidate();
        }
        if !self.tracking {
            self.tracking = true;
            let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: self.hwnd, dwHoverTime: 0 };
            unsafe {
                let _ = TrackMouseEvent(&mut tme);
            }
        }
    }

    fn mouse_down(&mut self, x: f32, y: f32) {
        unsafe {
            let _ = SetFocus(Some(self.hwnd));
        }
        self.capturing = None;
        self.pressed = self.hit(x, y);
        if let Some((i, _)) = self.pressed {
            if matches!(self.items[i].kind, Kind::Slider) {
                self.dragging = true;
                unsafe {
                    SetCapture(self.hwnd);
                }
                self.slider_from_x(i, x);
            }
        }
        self.invalidate();
    }

    fn mouse_up(&mut self, x: f32, y: f32) {
        let was_dragging = self.dragging;
        self.dragging = false;
        if was_dragging {
            unsafe {
                let _ = ReleaseCapture();
            }
        }
        let pressed = self.pressed.take();
        if let (Some(p), false) = (pressed, was_dragging) {
            if self.hit(x, y) == Some(p) {
                self.activate(p.0, p.1);
            }
        }
        self.hover = self.hit(x, y);
        self.invalidate();
    }

    fn activate(&mut self, i: usize, sub: u8) {
        match self.items[i].kind {
            Kind::Toggle(t) => {
                self.toggles[t] = !self.toggles[t];
                self.refresh_gates();
            }
            Kind::Button(_, _, act) => self.run(act),
            Kind::Nav(p, _, _) => self.set_page(p),
            Kind::Stepper(n) => self.step(n, if sub == 1 { -1.0 } else { 1.0 }),
            Kind::Hotkey(k) => self.capturing = Some(k),
            _ => {}
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
        let right = f(14.0, DWRITE_FONT_WEIGHT_NORMAL, false, true)?;
        let wrap = f(12.0, DWRITE_FONT_WEIGHT_NORMAL, false, false)?;
        unsafe {
            right.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            wrap.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
        }
        self.fmts = Some(Fmts {
            body: f(14.0, DWRITE_FONT_WEIGHT_NORMAL, false, false)?,
            body_v: f(14.0, DWRITE_FONT_WEIGHT_NORMAL, false, true)?,
            right,
            caption: f(12.0, DWRITE_FONT_WEIGHT_NORMAL, false, false)?,
            wrap,
            title: f(26.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, false)?,
            heading: f(13.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, false)?,
            sidebar_title: f(20.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, false)?,
            center: f(13.0, DWRITE_FONT_WEIGHT_NORMAL, true, true)?,
            center_bold: f(13.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, true, true)?,
            icon: make_format(dw, icon_family(dw), 16.0, DWRITE_FONT_WEIGHT_NORMAL, true, true)?,
            glyph: f(18.0, DWRITE_FONT_WEIGHT_NORMAL, true, true)?,
        });
        Ok(())
    }

    fn paint(&mut self, hdc: HDC) {
        let mut cr = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut cr);
        }
        let (w_, h_) = (cr.right.max(1), cr.bottom.max(1));
        if self.surf.ensure(w_, h_).is_err() || self.ensure_fmts().is_err() || self.surf.bind(w_, h_).is_err() {
            self.surf.reset_target();
            return;
        }
        let rt = self.surf.rt.clone().unwrap();
        let br = self.surf.brush.clone().unwrap();
        let k = self.dpi as f32 / 96.0;
        unsafe {
            rt.BeginDraw();
            rt.SetTransform(&Matrix3x2 { M11: k, M12: 0.0, M21: 0.0, M22: k, M31: 0.0, M32: 0.0 });
            if self.mica {
                rt.Clear(Some(&CLEAR));
            } else {
                rt.Clear(Some(&col((32, 32, 34), 255)));
            }
        }
        self.draw(&Canvas { rt: &rt, br: &br });
        unsafe {
            if rt.EndDraw(None, None).is_err() {
                self.surf.reset_target();
                return;
            }
        }
        self.surf.present(hdc, w_, h_);
    }

    fn draw(&self, cv: &Canvas) {
        let rt = cv.rt;
        let layer = unsafe { rt.CreateLayer(None).ok() };
        let primary = col(WHITE, 235);
        let secondary = col(WHITE, 145);

        // footer band
        cv.fill_rr((0.0, FOOTER_Y, CLIENT_W, CLIENT_H - FOOTER_Y), 0.0, col(BLACK, 38));

        for (i, it) in self.items.iter().enumerate() {
            if !self.visible(it) {
                continue;
            }
            let enabled = self.enabled(it);
            if !enabled {
                if let Some(l) = &layer {
                    let params = D2D1_LAYER_PARAMETERS {
                        contentBounds: D2D_RECT_F { left: -f32::MAX, top: -f32::MAX, right: f32::MAX, bottom: f32::MAX },
                        geometricMask: ManuallyDrop::new(None),
                        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                        maskTransform: Matrix3x2::identity(),
                        opacity: 0.4,
                        opacityBrush: ManuallyDrop::new(None),
                        layerOptions: D2D1_LAYER_OPTIONS_NONE,
                    };
                    unsafe { rt.PushLayer(&params, l) };
                }
            }
            self.draw_item(cv, i, it, primary, secondary);
            if !enabled && layer.is_some() {
                unsafe { rt.PopLayer() };
            }
        }
    }

    fn draw_item(&self, cv: &Canvas, i: usize, it: &Item, primary: D2D1_COLOR_F, secondary: D2D1_COLOR_F) {
        let f = self.fmts.as_ref().unwrap();
        let r = it.r;
        let hovered = self.hover.is_some_and(|h| h.0 == i);
        let pressed = self.pressed.is_some_and(|p| p.0 == i) && hovered;
        match &it.kind {
            Kind::SidebarTitle => cv.txt("Settings", &f.sidebar_title, r, primary),
            Kind::Nav(p, icon, name) => {
                let selected = *p == self.page;
                if selected {
                    cv.fill_rr(r, 6.0, col(WHITE, 17));
                    cv.fill_rr((r.0, r.1 + 12.0, 3.0, 16.0), 1.5, col(ACCENT, 255));
                } else if hovered {
                    cv.fill_rr(r, 6.0, col(WHITE, 9));
                }
                cv.txt(icon, &f.icon, (r.0 + 10.0, r.1, 24.0, r.3), if selected { col(ACCENT, 255) } else { primary });
                cv.txt(name, &f.body_v, (r.0 + 44.0, r.1, r.2 - 44.0, r.3), primary);
            }
            Kind::Title(t) => cv.txt(t, &f.title, r, primary),
            Kind::Heading(t) => cv.txt(t, &f.heading, r, secondary),
            Kind::Hint(t) => cv.txt(t, &f.wrap, r, secondary),
            Kind::Card => {
                cv.fill_rr(r, 8.0, col(WHITE, 11));
                cv.stroke_rr(r, 8.0, col(WHITE, 20));
            }
            Kind::Divider => cv.fill_rr(r, 0.0, col(WHITE, 16)),
            Kind::Label(title, sub) => {
                cv.txt(title, &f.body, r, primary);
                if !sub.is_empty() {
                    cv.txt(sub, &f.caption, (r.0, r.1 + 21.0, r.2, 18.0), secondary);
                }
            }
            Kind::Status(Status::Kelvin(n)) => {
                let k = self.nums[N_KDAY + n].value as i32;
                cv.txt(&preset_name(k), &f.right, r, secondary);
            }
            Kind::Status(Status::Location) => {
                let c = if self.loc_message.is_none() && self.has_loc { primary } else { secondary };
                cv.txt(&self.location_text(), &f.wrap, r, c);
            }
            Kind::Toggle(t) => {
                let on = self.toggles[*t];
                let (x, y, w_, h) = r;
                if on {
                    cv.fill_rr(r, 10.0, col(ACCENT, if hovered { 230 } else { 255 }));
                    cv.circle(x + w_ - 10.0, y + h / 2.0, if hovered { 7.0 } else { 6.0 }, col(BLACK, 215));
                } else {
                    cv.fill_rr(r, 10.0, col(WHITE, if hovered { 14 } else { 4 }));
                    cv.stroke_rr(r, 10.0, col(WHITE, 140));
                    cv.circle(x + 10.0, y + h / 2.0, if hovered { 7.0 } else { 6.0 }, col(WHITE, 200));
                }
            }
            Kind::Button(label, accent, _) => {
                if *accent {
                    let a = if pressed { 200 } else if hovered { 235 } else { 255 };
                    cv.fill_rr(r, 5.0, col(ACCENT, a));
                    cv.txt(label, &f.center_bold, r, col(BLACK, 235));
                } else {
                    let a = if pressed { 8 } else if hovered { 26 } else { 15 };
                    cv.fill_rr(r, 5.0, col(WHITE, a));
                    cv.stroke_rr(r, 5.0, col(WHITE, 22));
                    let text = if matches!(it.kind, Kind::Button(_, _, Act::Find)) && self.searching { "..." } else { label };
                    cv.txt(text, &f.center, r, primary);
                }
            }
            Kind::Field => self.draw_field(cv, r, self.edits.iter().any(|e| e.item == i && Some(e.hwnd) == self.focus_edit)),
            Kind::Hotkey(k) => {
                let armed = self.capturing == Some(*k);
                self.draw_field(cv, r, armed);
                let (mods, key) = self.hotkeys[*k];
                let text = if armed { "Press a key combination...".to_string() } else { format_hotkey(mods, key) };
                let c = if armed || key == 0 { secondary } else { primary };
                cv.txt(&text, &f.body_v, (r.0 + 12.0, r.1, r.2 - 24.0, r.3), c);
            }
            Kind::Stepper(_) => {
                let focused = self.edits.iter().any(|e| e.item == i && Some(e.hwnd) == self.focus_edit);
                self.draw_field(cv, (r.0, r.1, r.2 - 56.0, r.3), focused);
                for (sub, bx, glyph) in [(1u8, r.0 + r.2 - 54.0, "\u{2212}"), (2u8, r.0 + r.2 - 26.0, "+")] {
                    let bh = self.hover.is_some_and(|h| h.0 == i && h.1 == sub);
                    let bp = bh && self.pressed.is_some_and(|p| p.0 == i && p.1 == sub);
                    let a = if bp { 8 } else if bh { 28 } else { 15 };
                    let rect = (bx, r.1, 26.0, r.3);
                    cv.fill_rr(rect, 5.0, col(WHITE, a));
                    cv.stroke_rr(rect, 5.0, col(WHITE, 22));
                    cv.txt(glyph, &f.glyph, rect, primary);
                }
            }
            Kind::Slider => {
                let (x, y, _, h) = r;
                let cy = y + h / 2.0;
                let t = self.startup_level as f32 / MAX_LEVEL as f32;
                let (tx0, tw) = (x + 10.0, 180.0);
                cv.fill_rr((tx0, cy - 2.0, tw, 4.0), 2.0, col(WHITE, 60));
                cv.fill_rr((tx0, cy - 2.0, tw * t, 4.0), 2.0, col(ACCENT, 255));
                let big = hovered || self.dragging;
                cv.circle(tx0 + tw * t, cy, if big { 10.0 } else { 9.0 }, col(WHITE, 255));
                cv.circle(tx0 + tw * t, cy, if big { 6.0 } else { 5.0 }, col(ACCENT, 255));
                cv.txt(&format!("{}%", self.startup_level), &f.body_v, (x + 206.0, y, 44.0, h), primary);
            }
        }
    }

    fn draw_field(&self, cv: &Canvas, r: R4, focused: bool) {
        cv.fill_rr(r, 5.0, col(FIELD_BG, 255));
        cv.stroke_rr(r, 5.0, col(WHITE, 24));
        if focused {
            cv.fill_rr((r.0 + 3.0, r.1 + r.3 - 2.0, r.2 - 6.0, 2.0), 1.0, col(ACCENT, 255));
        }
    }
}

extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Dlg;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let d = &mut *ptr;
        let k = d.dpi as f32 / 96.0;
        let (mx, my) = (((lp.0 & 0xffff) as i16) as f32 / k, (((lp.0 >> 16) & 0xffff) as i16) as f32 / k);
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                d.paint(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                d.mouse_move(mx, my);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                d.tracking = false;
                if d.hover.take().is_some() {
                    d.invalidate();
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                d.mouse_down(mx, my);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                d.mouse_up(mx, my);
                LRESULT(0)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if d.key_down(wp.0 as u32) {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_GEO_DONE => {
                d.geo_done();
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wp.0 & 0xffff) as i32;
                let code = ((wp.0 >> 16) & 0xffff) as u32;
                let idx = id - ID_EDIT;
                if idx >= 0 && (idx as usize) < d.edits.len() {
                    if code == EN_SETFOCUS {
                        d.focus_edit = Some(d.edits[idx as usize].hwnd);
                        d.capturing = None;
                        d.invalidate();
                    } else if code == EN_KILLFOCUS {
                        d.focus_edit = None;
                        if let Some(n) = d.nums.iter().position(|n| n.edit == idx as usize) {
                            d.commit(n);
                        }
                        d.invalidate();
                    }
                }
                LRESULT(0)
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
                let hdc = HDC(wp.0 as *mut _);
                SetTextColor(hdc, colorref(if msg == WM_CTLCOLORSTATIC { (130, 130, 134) } else { WHITE }));
                SetBkColor(hdc, colorref(FIELD_BG));
                LRESULT(d.edit_brush.0 as isize)
            }
            WM_CLOSE => {
                d.done = true;
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
