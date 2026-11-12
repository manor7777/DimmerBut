//! Hardware brightness / power: DDC/CI for external monitors (dxva2) and WMI for laptop panels.
//! Every monitor owns a small writer thread that debounces slider drags, so the UI never blocks on
//! slow DDC/CI traffic.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::{HANDLE, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::core::BOOL;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MonitorKind {
    Ddc,
    Wmi,
}

enum Cmd {
    Brightness(i32),
    Power(u32),
}

pub struct MonitorInfo {
    pub kind: MonitorKind,
    pub name: String,
    pub tag: String,
    pub percent: i32,
    tx: Sender<Cmd>,
}

impl MonitorInfo {
    fn new(kind: MonitorKind, name: String, tag: String, percent: i32, handle: isize, min: u32, max: u32) -> Self {
        let (tx, rx) = channel();
        std::thread::spawn(move || worker(kind, handle, min, max, rx));
        Self { kind, name, tag, percent, tx }
    }

    pub fn supports_power(&self) -> bool {
        self.kind == MonitorKind::Ddc
    }

    /// Updates local state immediately for UI responsiveness; the hardware write is debounced.
    pub fn request_brightness(&mut self, percent: i32) {
        let p = percent.clamp(0, 100);
        self.percent = p;
        let _ = self.tx.send(Cmd::Brightness(p));
    }

    /// VCP 0xD6 power mode (4 = DPM off).
    pub fn set_power(&self, mode: u32) {
        if self.supports_power() {
            let _ = self.tx.send(Cmd::Power(mode));
        }
    }
}

fn worker(kind: MonitorKind, handle: isize, min: u32, max: u32, rx: Receiver<Cmd>) {
    let wmi_session = if kind == MonitorKind::Wmi { wmi::Session::open() } else { None };
    let h = HANDLE(handle as *mut _);

    let write_brightness = |p: i32| match kind {
        MonitorKind::Ddc => unsafe {
            let raw = (min as f64 + ((max - min) as f64 * p as f64 / 100.0).round()) as u32;
            let _ = SetMonitorBrightness(h, raw);
        },
        MonitorKind::Wmi => {
            if let Some(s) = &wmi_session {
                s.set_brightness(p);
            }
        }
    };
    let write_power = |mode: u32| unsafe {
        let _ = SetVCPFeature(h, 0xD6, mode);
    };

    'outer: while let Ok(first) = rx.recv() {
        let mut pending: Option<i32> = None;
        let mut power: Option<u32> = None;
        let mut disconnected = false;
        match first {
            Cmd::Brightness(p) => pending = Some(p),
            Cmd::Power(m) => power = Some(m),
        }
        // Coalesce a burst of slider moves into one write.
        while power.is_none() && !disconnected {
            match rx.recv_timeout(Duration::from_millis(120)) {
                Ok(Cmd::Brightness(p)) => pending = Some(p),
                Ok(Cmd::Power(m)) => power = Some(m),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => disconnected = true,
            }
        }
        if let Some(p) = pending {
            write_brightness(p);
        }
        if let Some(m) = power {
            write_power(m);
        }
        if disconnected {
            break 'outer;
        }
    }

    if kind == MonitorKind::Ddc && handle != 0 {
        unsafe {
            let _ = DestroyPhysicalMonitor(h);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------------------------------

/// Blocking: call from a worker thread.
pub fn enumerate() -> Vec<MonitorInfo> {
    let mut result: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(&mut result as *mut _ as isize));
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for m in &result {
        *counts.entry(m.name.clone()).or_default() += 1;
    }
    for m in &mut result {
        if counts[&m.name] > 1 && !m.tag.is_empty() {
            m.name = format!("{} ({})", m.name, m.tag);
        }
    }

    if let Some(pct) = wmi::current_brightness() {
        result.push(MonitorInfo::new(MonitorKind::Wmi, "Built-in display".into(), String::new(), pct.clamp(0, 100), 0, 0, 100));
    }
    result
}

unsafe extern "system" fn enum_proc(hmon: HMONITOR, _hdc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
    let result = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
    unsafe { add_from_hmonitor(hmon, result) };
    BOOL(1)
}

fn utf16_str(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

unsafe fn add_from_hmonitor(hmon: HMONITOR, result: &mut Vec<MonitorInfo>) {
    unsafe {
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        let mut device = String::new();
        if GetMonitorInfoW(hmon, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            device = utf16_str(&mi.szDevice);
        }

        let mut friendly = String::new();
        if !device.is_empty() {
            let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
            let dev_w: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();
            if EnumDisplayDevicesW(windows::core::PCWSTR(dev_w.as_ptr()), 0, &mut dd, 0).as_bool() {
                friendly = utf16_str(&dd.DeviceString);
            }
        }
        let tag = device.replace(['\\', '.'], "");

        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_err() || count == 0 {
            return;
        }
        let mut phys = vec![PHYSICAL_MONITOR::default(); count as usize];
        if GetPhysicalMonitorsFromHMONITOR(hmon, &mut phys).is_err() {
            return;
        }

        for p in phys {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
            if GetMonitorBrightness(p.hPhysicalMonitor, &mut min, &mut cur, &mut max) != 0 && max > min {
                let pct = ((cur - min) as f64 * 100.0 / (max - min) as f64).round() as i32;
                let desc_buf = p.szPhysicalMonitorDescription;
                let desc = utf16_str(&desc_buf);
                let name = if !friendly.is_empty() {
                    friendly.clone()
                } else if !desc.is_empty() {
                    desc
                } else {
                    "Display".into()
                };
                result.push(MonitorInfo::new(MonitorKind::Ddc, name, tag.clone(), pct.clamp(0, 100), p.hPhysicalMonitor.0 as isize, min, max));
            } else {
                let _ = DestroyPhysicalMonitor(p.hPhysicalMonitor);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// WMI (laptop panels)
// ---------------------------------------------------------------------------------------------------

mod wmi {
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::System::Wmi::*;
    use windows::core::{BSTR, Interface, w};

    /// Scoped COM apartment + connection to ROOT\WMI.
    pub struct Session {
        // ManuallyDrop: must be released *before* CoUninitialize (see Drop), not after it.
        svc: std::mem::ManuallyDrop<IWbemServices>,
        uninit: bool,
    }

    fn init_com() -> Option<bool> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_ok() {
                Some(true)
            } else if hr == RPC_E_CHANGED_MODE {
                Some(false) // already initialised with another model on this thread; usable
            } else {
                None
            }
        }
    }

    impl Session {
        pub fn open() -> Option<Session> {
            let uninit = init_com()?;
            match Self::connect() {
                Some(svc) => Some(Session { svc: std::mem::ManuallyDrop::new(svc), uninit }),
                None => {
                    if uninit {
                        unsafe { CoUninitialize() };
                    }
                    None
                }
            }
        }

        fn connect() -> Option<IWbemServices> {
            unsafe {
                let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
                let svc = locator
                    .ConnectServer(&BSTR::from("ROOT\\WMI"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)
                    .ok()?;
                let _ = CoSetProxyBlanket(
                    &svc,
                    10, // RPC_C_AUTHN_WINNT
                    0,  // RPC_C_AUTHZ_NONE
                    None,
                    RPC_C_AUTHN_LEVEL_CALL,
                    RPC_C_IMP_LEVEL_IMPERSONATE,
                    None,
                    EOAC_NONE,
                );
                Some(svc)
            }
        }

        fn first(&self, class: &str) -> Option<IWbemClassObject> {
            unsafe {
                let en = self
                    .svc
                    .ExecQuery(
                        &BSTR::from("WQL"),
                        &BSTR::from(format!("SELECT * FROM {class}")),
                        WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                        None,
                    )
                    .ok()?;
                let mut objs: [Option<IWbemClassObject>; 1] = [None];
                let mut returned = 0u32;
                let _ = en.Next(WBEM_INFINITE, &mut objs, &mut returned);
                if returned == 0 { None } else { objs[0].take() }
            }
        }

        pub fn brightness(&self) -> Option<i32> {
            let obj = self.first("WmiMonitorBrightness")?;
            unsafe {
                let mut v = VARIANT::default();
                obj.Get(w!("CurrentBrightness"), 0, &mut v, None, None).ok()?;
                i32::try_from(&v).ok()
            }
        }

        pub fn set_brightness(&self, percent: i32) {
            let _ = (|| -> Option<()> {
                unsafe {
                    let inst = self.first("WmiMonitorBrightnessMethods")?;
                    let mut path = VARIANT::default();
                    inst.Get(w!("__PATH"), 0, &mut path, None, None).ok()?;
                    let path = BSTR::try_from(&path).ok()?;

                    let mut class: Option<IWbemClassObject> = None;
                    self.svc.GetObject(&BSTR::from("WmiMonitorBrightnessMethods"), WBEM_GENERIC_FLAG_TYPE(0), None, Some(&mut class), None).ok()?;
                    let class = class?;
                    let mut in_sig: Option<IWbemClassObject> = None;
                    class.GetMethod(w!("WmiSetBrightness"), 0, &mut in_sig, std::ptr::null_mut()).ok()?;
                    let params = in_sig?.SpawnInstance(0).ok()?;
                    params.Put(w!("Timeout"), 0, &VARIANT::from(1i32), 0).ok()?;
                    params.Put(w!("Brightness"), 0, &VARIANT::from(percent.clamp(0, 100)), 0).ok()?;
                    self.svc
                        .ExecMethod(&path, &BSTR::from("WmiSetBrightness"), WBEM_GENERIC_FLAG_TYPE(0), None, &params, None, None)
                        .ok()?;
                    Some(())
                }
            })();
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            // Release the WMI proxy while COM is still up. Releasing it after CoUninitialize
            // (the default field-drop order) crashes with an access violation on recent Windows builds.
            unsafe { std::mem::ManuallyDrop::drop(&mut self.svc) };
            if self.uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    pub fn current_brightness() -> Option<i32> {
        Session::open()?.brightness()
    }

    // Keep the Interface import used on all cfgs.
    #[allow(dead_code)]
    fn _unused<T: Interface>() {}
}
