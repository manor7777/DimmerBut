//! "Start with Windows": a value under HKCU\...\Run (no admin rights needed).
//! The value name is stable across versions, so an existing entry is recognised.

use crate::wide;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, w};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("DimmerBut");

fn open(access: REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    unsafe { (RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) == ERROR_SUCCESS).then_some(key) }
}

pub fn is_enabled() -> bool {
    let Some(key) = open(KEY_READ) else { return false };
    let mut size = 0u32;
    let found = unsafe { RegQueryValueExW(key, VALUE, None, None, None, Some(&mut size)) == ERROR_SUCCESS };
    unsafe {
        let _ = RegCloseKey(key);
    }
    found
}

pub fn set(enabled: bool) -> bool {
    let Some(key) = open(KEY_WRITE) else { return false };
    let ok = unsafe {
        if enabled {
            match std::env::current_exe() {
                Ok(exe) => {
                    let cmd = wide(&format!("\"{}\"", exe.display()));
                    let bytes = std::slice::from_raw_parts(cmd.as_ptr() as *const u8, cmd.len() * 2);
                    RegSetValueExW(key, VALUE, None, REG_SZ, Some(bytes)) == ERROR_SUCCESS
                }
                Err(_) => false,
            }
        } else {
            let r = RegDeleteValueW(key, VALUE);
            r == ERROR_SUCCESS || !is_enabled()
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    ok
}

/// If the exe was moved, keep the start-up entry pointing at the current location.
pub fn refresh_path() {
    if is_enabled() {
        set(true);
    }
}
