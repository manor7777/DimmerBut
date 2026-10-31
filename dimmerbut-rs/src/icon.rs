//! Tray icon: a moon that wanes as the dimming level rises (0% = full moon, 90% = thin crescent,
//! lit side on the right). Rendered on the CPU with 4x4 supersampling into a 32bpp icon.

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

const DARK: [f32; 3] = [45.0, 48.0, 62.0];
const LIT: [f32; 3] = [250.0, 244.0, 205.0];
const RING: [f32; 3] = [150.0, 150.0, 160.0];

/// RGB colour of the moon at a point relative to its centre (in units of the moon's radius), or None outside.
fn sample(x: f32, y: f32, lit: f32, ring_w: f32) -> Option<[f32; 3]> {
    let d = (x * x + y * y).sqrt();
    if d > 1.0 {
        return None;
    }
    if d > 1.0 - ring_w {
        return Some(RING);
    }
    let mut is_lit = x >= 0.0;
    if lit > 0.5 {
        let rx = 2.0 * lit - 1.0;
        if rx >= 0.03 && x < 0.0 && (x / rx).powi(2) + y * y <= 1.0 {
            is_lit = true;
        }
    } else if lit < 0.5 {
        let rx = 1.0 - 2.0 * lit;
        if rx >= 0.03 && x > 0.0 && (x / rx).powi(2) + y * y <= 1.0 {
            is_lit = false;
        }
    }
    Some(if is_lit { LIT } else { DARK })
}

pub fn moon_icon(level: i32, size: i32) -> Option<HICON> {
    let n = size.max(16);
    let lit = (1.0 - level as f32 / 100.0).clamp(0.0, 1.0);
    let radius = n as f32 * 14.0 / 32.0;
    let ring_w = 1.0 / radius * (n as f32 / 32.0).max(1.0);
    let centre = n as f32 / 2.0;
    const SS: i32 = 4;

    let mut pixels = vec![0u32; (n * n) as usize];
    for py in 0..n {
        for px in 0..n {
            let (mut r, mut g, mut b, mut cover) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32 - centre) / radius;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32 - centre) / radius;
                    if let Some(c) = sample(x, y, lit, ring_w) {
                        r += c[0];
                        g += c[1];
                        b += c[2];
                        cover += 1.0;
                    }
                }
            }
            if cover > 0.0 {
                let total = (SS * SS) as f32;
                let a = cover / total;
                // premultiplied BGRA
                let (pr, pg, pb) = (r / total, g / total, b / total);
                pixels[(py * n + px) as usize] =
                    ((a * 255.0).round() as u32) << 24 | (pr.round() as u32) << 16 | (pg.round() as u32) << 8 | pb.round() as u32;
            }
        }
    }

    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: n,
                biHeight: -n,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(color.into());
            return None;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());
        let mask = CreateBitmap(n, n, 1, 1, None);
        let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}
