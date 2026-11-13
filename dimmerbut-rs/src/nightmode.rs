//! f.lux-style schedule: Daytime -> Sunset -> Bedtime -> back to Daytime in the morning.
//! Sunrise/sunset are calculated offline from the saved latitude/longitude.
//!
//!   morning ease : Bedtime K -> Daytime K   (starts at the later of sunrise and wake time)
//!   daytime      : Daytime K
//!   sunset ease  : Daytime K -> Sunset K    (starts at sunset)
//!   evening      : Sunset K
//!   bed ease     : Sunset K -> Bedtime K    (starts 8 hours before wake time)
//!   night        : Bedtime K

use crate::settings::AppSettings;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use std::cell::RefCell;

const SLEEP_MINUTES: f64 = 480.0;
const ZENITH: f64 = 90.833;

type SunCache = Option<(NaiveDate, f64, f64, f64, f64)>;

thread_local! {
    static CACHE: RefCell<SunCache> = const { RefCell::new(None) };
}

fn minutes_of(t: DateTime<Local>) -> f64 {
    t.hour() as f64 * 60.0 + t.minute() as f64 + t.second() as f64 / 60.0
}

pub fn get_kelvin(s: &AppSettings, now: DateTime<Local>) -> f64 {
    let (rise, sset) = sun_minutes_cached(s, now.date_naive());

    let tr = (s.transition_minutes as f64).max(1.0);
    let wake = s.wake_minutes as f64;

    let m0 = rise.max(wake).clamp(0.0, 1439.0);
    let m1 = m0 + tr;
    let s0 = sset.max(m1);
    let s1 = s0 + tr;

    let mut bed = ((wake - SLEEP_MINUTES) % 1440.0 + 1440.0) % 1440.0;
    if bed < m0 {
        bed += 1440.0;
    }
    let end_frame = m0 + 1440.0;
    let b0 = bed.max(s1).min(end_frame - tr);
    let b1 = b0 + tr;

    let mut t = minutes_of(now);
    if t < m0 {
        t += 1440.0;
    }

    let (day_k, sun_k, bed_k) = (s.day_kelvin as f64, s.sunset_kelvin as f64, s.bed_kelvin as f64);

    if t < m1 {
        return ease(bed_k, day_k, (t - m0) / tr);
    }
    if t < s0 {
        return day_k;
    }
    if t < s1 {
        return ease(day_k, sun_k, (t - s0) / tr);
    }
    if t < b0 {
        return sun_k;
    }
    if t < b1 {
        return ease(sun_k, bed_k, (t - b0) / tr);
    }
    bed_k
}

/// Dim schedule position: 0.0 = daytime brightness, 1.0 = night brightness. Eases in over the ramp after
/// the start time and back out over the ramp after the end time (the window may wrap past midnight).
pub fn schedule_factor(s: &AppSettings, now: DateTime<Local>) -> f64 {
    let ramp = (s.sched_ramp_minutes as f64).max(1.0);
    let start = s.sched_start_minutes as f64;
    let len = ((s.sched_end_minutes as f64 - start) % 1440.0 + 1440.0) % 1440.0;
    let d = ((minutes_of(now) - start) % 1440.0 + 1440.0) % 1440.0;
    if d < len {
        ease(0.0, 1.0, d / ramp)
    } else if d < len + ramp {
        ease(0.0, 1.0, len / ramp) * (1.0 - ease(0.0, 1.0, (d - len) / ramp))
    } else {
        0.0
    }
}

fn ease(a: f64, b: f64, x: f64) -> f64 {
    let u = x.clamp(0.0, 1.0);
    let u = u * u * (3.0 - 2.0 * u);
    a + (b - a) * u
}

/// Per-channel gains (0..1) for a colour temperature, relative to 6500K = no change.
pub fn get_gains(kelvin: f64) -> [f32; 3] {
    let c = kelvin_to_rgb(kelvin);
    let w = kelvin_to_rgb(6500.0);
    [(c[0] / w[0]).min(1.0) as f32, (c[1] / w[1]).min(1.0) as f32, (c[2] / w[2]).min(1.0) as f32]
}

fn kelvin_to_rgb(kelvin: f64) -> [f64; 3] {
    let t = kelvin.clamp(1000.0, 6500.0) / 100.0;
    let (r, g);
    if t <= 66.0 {
        r = 255.0;
        g = 99.4708025861 * t.ln() - 161.1195681661;
    } else {
        r = 329.698727446 * (t - 60.0).powf(-0.1332047592);
        g = 288.1221695283 * (t - 60.0).powf(-0.0755148492);
    }
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.5177312231 * (t - 10.0).ln() - 305.0447927307
    };
    [r.clamp(0.0, 255.0) / 255.0, g.clamp(0.0, 255.0) / 255.0, b.clamp(0.0, 255.0) / 255.0]
}

fn sun_minutes_cached(s: &AppSettings, d: NaiveDate) -> (f64, f64) {
    if !s.has_location {
        return (7.0 * 60.0, 19.0 * 60.0);
    }
    if let Some((cd, la, lo, r, st)) = CACHE.with(|c| *c.borrow()) {
        if cd == d && la == s.latitude && lo == s.longitude {
            return (r, st);
        }
    }
    let r = sun_minutes(d, s.latitude, s.longitude, true);
    let st = sun_minutes(d, s.latitude, s.longitude, false);
    let (rise, sset) = match (r, st) {
        (Some(r), Some(st)) if r < st => (r, st),
        _ => (6.0 * 60.0, 18.0 * 60.0),
    };
    CACHE.with(|c| *c.borrow_mut() = Some((d, s.latitude, s.longitude, rise, sset)));
    (rise, sset)
}

fn norm(x: f64, m: f64) -> f64 {
    ((x % m) + m) % m
}

/// NOAA / Almanac algorithm; returns local minutes after midnight.
fn sun_minutes(d: NaiveDate, lat: f64, lon: f64, rising: bool) -> Option<f64> {
    let n = d.ordinal() as f64;
    let lng_hour = lon / 15.0;
    let t = n + ((if rising { 6.0 } else { 18.0 }) - lng_hour) / 24.0;
    let m = 0.9856 * t - 3.289;

    let l = norm(m + 1.916 * m.to_radians().sin() + 0.02 * (2.0 * m).to_radians().sin() + 282.634, 360.0);

    let mut ra = norm((0.91764 * l.to_radians().tan()).atan().to_degrees(), 360.0);
    let l_quad = (l / 90.0).floor() * 90.0;
    let ra_quad = (ra / 90.0).floor() * 90.0;
    ra = (ra + (l_quad - ra_quad)) / 15.0;

    let sin_dec = 0.39782 * l.to_radians().sin();
    let cos_dec = sin_dec.asin().cos();

    let cos_h = (ZENITH.to_radians().cos() - sin_dec * lat.to_radians().sin()) / (cos_dec * lat.to_radians().cos());
    if !(-1.0..=1.0).contains(&cos_h) {
        return None;
    }

    let h = if rising { (360.0 - cos_h.acos().to_degrees()) / 15.0 } else { cos_h.acos().to_degrees() / 15.0 };
    let tt = h + ra - 0.06571 * t - 6.622;
    let ut = norm(tt - lng_hour, 24.0);

    let midnight_utc = d.and_hms_opt(0, 0, 0)?.and_utc();
    let utc = midnight_utc + Duration::milliseconds((ut * 3_600_000.0) as i64);
    let local = Local.from_utc_datetime(&utc.naive_utc());
    Some(minutes_of(local))
}
