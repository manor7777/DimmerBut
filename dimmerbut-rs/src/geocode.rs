//! One-off city lookup using the free Open-Meteo geocoding API (no key, no account).
//! Only the city name you type is sent; the coordinates are then stored locally and
//! all sunrise/sunset maths is done offline.

use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

pub struct GeoResult {
    pub display_name: String,
    pub latitude: f64,
    pub longitude: f64,
}

fn agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .build()
            .into()
    })
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `Ok(None)` = no match, `Err` = lookup failed (offline etc.). Blocking: call from a worker thread.
pub fn find(query: &str) -> Result<Option<GeoResult>, String> {
    let mut parts = query.splitn(2, ',');
    let place = parts.next().unwrap_or("").trim();
    let qual = parts.next().unwrap_or("").trim();
    if place.chars().count() < 2 {
        return Ok(None);
    }

    let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=20&language=en&format=json", escape(place));
    let mut resp = agent().get(&url).header("User-Agent", "DimmerBut").call().map_err(|e| e.to_string())?;
    let body = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let doc: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let Some(results) = doc.get("results").and_then(Value::as_array) else { return Ok(None) };
    let s = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();

    for r in results {
        let (country, admin1, admin2, cc) = (s(r, "country"), s(r, "admin1"), s(r, "admin2"), s(r, "country_code"));
        if qual.is_empty() || matches(qual, &[&country, &admin1, &admin2, &cc]) {
            let name = s(r, "name");
            let mut bits = vec![name.clone()];
            if !admin1.is_empty() && admin1 != name {
                bits.push(admin1);
            }
            if !country.is_empty() {
                bits.push(country);
            }
            let (Some(lat), Some(lon)) = (r.get("latitude").and_then(Value::as_f64), r.get("longitude").and_then(Value::as_f64)) else {
                continue;
            };
            return Ok(Some(GeoResult { display_name: bits.join(", "), latitude: lat, longitude: lon }));
        }
    }
    Ok(None)
}

fn matches(qual: &str, candidates: &[&str]) -> bool {
    let mut q = qual.trim().trim_matches('.').replace('.', "");
    let up = q.to_uppercase();
    match up.as_str() {
        "UK" | "GREAT BRITAIN" | "BRITAIN" | "ENGLAND" | "SCOTLAND" | "WALES" | "NORTHERN IRELAND" => {
            if up == "UK" || up.ends_with("BRITAIN") {
                q = "GB".into();
            }
        }
        "USA" | "US" | "UNITED STATES OF AMERICA" => q = "US".into(),
        _ => {}
    }
    candidates.iter().any(|c| !c.is_empty() && c.eq_ignore_ascii_case(&q))
}
