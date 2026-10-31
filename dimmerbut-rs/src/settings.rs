//! Persistent settings. The file location and format match the original VB.NET app
//! (`%APPDATA%\DimmerBut\settings.json`, PascalCase keys), so an existing configuration is picked up as-is.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_LEVEL: i32 = 90;
pub const STEP: i32 = 10;
pub const MIN_KELVIN: i32 = 1200;
pub const MAX_KELVIN: i32 = 6500;

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "PascalCase")]
pub struct AppSettings {
    // Dimming
    pub dimming_enabled: bool,
    pub startup_level: i32,

    // Night mode (f.lux style)
    pub night_enabled: bool,
    pub day_kelvin: i32,
    pub sunset_kelvin: i32,
    pub bed_kelvin: i32,
    pub wake_minutes: i32,
    pub transition_minutes: i32,

    // Location (looked up once by city name, or typed in)
    pub has_location: bool,
    pub location_name: String,
    pub latitude: f64,
    pub longitude: f64,

    // Flyout & monitors
    pub show_hardware_brightness: bool,
    pub show_power_control: bool,
    pub flyout_acrylic: bool,
    pub suppress_power_warning: bool,
    pub auto_poll_monitors: bool,
    pub poll_interval_seconds: i32,

    // Hotkeys (modifiers are MOD_* flags, keys are Windows virtual-key codes)
    pub hotkeys_enabled: bool,
    pub increase_mods: u32,
    pub increase_key: i32,
    pub decrease_mods: u32,
    pub decrease_key: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dimming_enabled: true,
            startup_level: 0,
            night_enabled: false,
            day_kelvin: 6500,
            sunset_kelvin: 3400,
            bed_kelvin: 2700,
            wake_minutes: 450,
            transition_minutes: 60,
            has_location: false,
            location_name: String::new(),
            latitude: 0.0,
            longitude: 0.0,
            show_hardware_brightness: true,
            show_power_control: true,
            flyout_acrylic: true,
            suppress_power_warning: false,
            auto_poll_monitors: true,
            poll_interval_seconds: 15,
            hotkeys_enabled: false,
            increase_mods: MOD_CONTROL | MOD_ALT,
            increase_key: 0x26, // VK_UP
            decrease_mods: MOD_CONTROL | MOD_ALT,
            decrease_key: 0x28, // VK_DOWN
        }
    }
}

pub fn data_folder() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("DimmerBut")
}

fn file_path() -> PathBuf {
    data_folder().join("settings.json")
}

impl AppSettings {
    pub fn load() -> Self {
        let mut s: AppSettings = std::fs::read_to_string(file_path())
            .ok()
            .map(|t| t.trim_start_matches('\u{feff}').to_string())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        s.sanitize();
        s
    }

    pub fn sanitize(&mut self) {
        self.startup_level = self.startup_level.clamp(0, MAX_LEVEL);
        self.day_kelvin = self.day_kelvin.clamp(MIN_KELVIN, MAX_KELVIN);
        self.sunset_kelvin = self.sunset_kelvin.clamp(MIN_KELVIN, MAX_KELVIN);
        self.bed_kelvin = self.bed_kelvin.clamp(MIN_KELVIN, MAX_KELVIN);
        self.wake_minutes = self.wake_minutes.clamp(0, 1439);
        self.transition_minutes = self.transition_minutes.clamp(1, 240);
        self.poll_interval_seconds = self.poll_interval_seconds.clamp(5, 120);
        self.latitude = self.latitude.clamp(-90.0, 90.0);
        self.longitude = self.longitude.clamp(-180.0, 180.0);
    }

    /// Writes via a temp file so a crash mid-write can't leave a truncated settings file.
    pub fn save(&self) -> bool {
        let dir = data_folder();
        let _ = std::fs::create_dir_all(&dir);
        let Ok(json) = serde_json::to_string_pretty(self) else { return false };
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, file_path()).is_ok()
    }
}

/// "Ctrl + Alt + Up" style text for a hotkey.
pub fn format_hotkey(mods: u32, key: i32) -> String {
    if key == 0 {
        return "None".into();
    }
    let mut parts: Vec<String> = Vec::new();
    if mods & MOD_CONTROL != 0 {
        parts.push("Ctrl".into());
    }
    if mods & MOD_ALT != 0 {
        parts.push("Alt".into());
    }
    if mods & MOD_SHIFT != 0 {
        parts.push("Shift".into());
    }
    parts.push(key_name(key));
    parts.join(" + ")
}

fn key_name(vk: i32) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x20 => "Space".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 | 0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x6A => "Num *".into(),
        0x6B => "Num +".into(),
        0x6D => "Num -".into(),
        0x6E => "Num .".into(),
        0x6F => "Num /".into(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Key {vk}"),
    }
}
