//! Applies dimming and the night-mode tint together as ONE colour matrix using the Windows Magnification
//! API full-screen colour effect (works on video/MPO planes, taskbar and Start menu).
//! Each channel gain = night_gain(channel) * dim_factor.

use crate::nightmode;
use crate::settings::{AppSettings, MAX_LEVEL};
use windows::Win32::UI::Magnification::{MAGCOLOREFFECT, MagInitialize, MagSetFullscreenColorEffect, MagUninitialize};

pub struct DimController {
    level: i32,
    mag_ready: bool,
    kelvin: f64,
    last_raised_kelvin: f64,
}

fn build_matrix(r: f32, g: f32, b: f32) -> MAGCOLOREFFECT {
    let mut m = MAGCOLOREFFECT { transform: [0.0; 25] };
    m.transform[0] = r;
    m.transform[6] = g;
    m.transform[12] = b;
    m.transform[18] = 1.0;
    m.transform[24] = 1.0;
    m
}

impl DimController {
    pub fn new(s: &AppSettings) -> Self {
        let mut c = Self { level: s.startup_level.clamp(0, MAX_LEVEL), mag_ready: false, kelvin: 0.0, last_raised_kelvin: 0.0 };
        c.apply(s);
        c
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn effective_level(&self, s: &AppSettings) -> i32 {
        if s.dimming_enabled { self.level } else { 0 }
    }

    /// Current night-mode colour temperature, or 0 when night mode is off.
    pub fn night_kelvin(&self, s: &AppSettings) -> f64 {
        if s.night_enabled { self.kelvin } else { 0.0 }
    }

    /// Sets the dimming level. Returns true if anything visible changed.
    pub fn set_level(&mut self, s: &mut AppSettings, value: i32) -> bool {
        let v = value.clamp(0, MAX_LEVEL);
        let mut changed = v != self.level;
        self.level = v;
        if v > 0 && !s.dimming_enabled {
            s.dimming_enabled = true;
            s.save();
            changed = true;
        }
        if changed {
            self.apply(s);
        }
        changed
    }

    /// Toggles night mode. Returns true if it changed.
    pub fn set_night(&mut self, s: &mut AppSettings, on: bool) -> bool {
        if on == s.night_enabled {
            return false;
        }
        s.night_enabled = on;
        s.save();
        self.apply(s);
        true
    }

    /// Recomputes and applies the colour matrix. Returns true when the night temperature drifted far enough
    /// that the UI (tray tooltip, flyout) should refresh.
    pub fn apply(&mut self, s: &AppSettings) -> bool {
        let dim_f = if s.dimming_enabled && self.level > 0 { 1.0 - self.level as f32 / 100.0 } else { 1.0 };

        let mut gains = [1.0f32; 3];
        if s.night_enabled {
            self.kelvin = nightmode::get_kelvin(s, chrono::Local::now());
            gains = nightmode::get_gains(self.kelvin);
        } else {
            self.kelvin = 0.0;
        }

        let (r, g, b) = (gains[0] * dim_f, gains[1] * dim_f, gains[2] * dim_f);
        let active = r < 0.9999 || g < 0.9999 || b < 0.9999;

        unsafe {
            if !self.mag_ready {
                if !active {
                    return false;
                }
                self.mag_ready = MagInitialize().as_bool();
                if !self.mag_ready {
                    return false;
                }
            }

            let fx = build_matrix(r, g, b);
            let _ = MagSetFullscreenColorEffect(&fx);

            if !active {
                let _ = MagUninitialize();
                self.mag_ready = false;
            }
        }

        if s.night_enabled && (self.kelvin - self.last_raised_kelvin).abs() >= 25.0 {
            self.last_raised_kelvin = self.kelvin;
            return true;
        }
        false
    }

    /// Restores the screen to normal.
    pub fn shutdown(&mut self) {
        if self.mag_ready {
            unsafe {
                let _ = MagSetFullscreenColorEffect(&build_matrix(1.0, 1.0, 1.0));
                let _ = MagUninitialize();
            }
            self.mag_ready = false;
        }
    }
}

impl Drop for DimController {
    fn drop(&mut self) {
        self.shutdown();
    }
}
