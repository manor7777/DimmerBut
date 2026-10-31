//! Shared Direct2D drawing surface: a 32bpp premultiplied-alpha DIB rendered through a DC render target
//! and BitBlt'd to the window, so DWM backdrops (acrylic / Mica) show through pixels with alpha 0.

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::core::Result;

pub struct Surface {
    pub d2d: ID2D1Factory,
    pub mem_dc: HDC,
    pub rt: Option<ID2D1DCRenderTarget>,
    pub brush: Option<ID2D1SolidColorBrush>,
    dib: HBITMAP,
    old_bmp: HGDIOBJ,
    capacity: (i32, i32),
}

impl Surface {
    pub fn new() -> Result<Self> {
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let screen = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            Ok(Self { d2d, mem_dc, rt: None, brush: None, dib: HBITMAP::default(), old_bmp: HGDIOBJ::default(), capacity: (0, 0) })
        }
    }

    /// Makes sure a DIB of at least `w` x `h` and a render target exist.
    pub fn ensure(&mut self, w: i32, h: i32) -> Result<()> {
        self.ensure_dib(w, h)?;
        self.ensure_target()
    }

    fn ensure_dib(&mut self, w: i32, h: i32) -> Result<()> {
        if self.capacity.0 >= w && self.capacity.1 >= h && !self.dib.is_invalid() {
            return Ok(());
        }
        let (cw, ch) = (w.max(self.capacity.0) + 32, h.max(self.capacity.1) + 32);
        unsafe {
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: cw,
                    biHeight: -ch, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let new_dib = CreateDIBSection(Some(self.mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let old = SelectObject(self.mem_dc, new_dib.into());
            if self.old_bmp.is_invalid() {
                self.old_bmp = old;
            }
            if !self.dib.is_invalid() {
                let _ = DeleteObject(self.dib.into());
            }
            self.dib = new_dib;
            self.capacity = (cw, ch);
        }
        Ok(())
    }

    fn ensure_target(&mut self) -> Result<()> {
        if self.rt.is_some() {
            return Ok(());
        }
        unsafe {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let rt = self.d2d.CreateDCRenderTarget(&props)?;
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            self.brush = Some(rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }, None)?);
            self.rt = Some(rt);
        }
        Ok(())
    }

    /// Binds the DIB to the render target for a frame of `w` x `h` pixels.
    pub fn bind(&self, w: i32, h: i32) -> Result<()> {
        unsafe { self.rt.as_ref().unwrap().BindDC(self.mem_dc, &RECT { left: 0, top: 0, right: w, bottom: h }) }
    }

    pub fn present(&self, hdc: HDC, w: i32, h: i32) {
        unsafe {
            let _ = BitBlt(hdc, 0, 0, w, h, Some(self.mem_dc), 0, 0, SRCCOPY);
        }
    }

    /// Device lost / D2DERR_RECREATE_TARGET: rebuild on the next frame.
    pub fn reset_target(&mut self) {
        self.rt = None;
        self.brush = None;
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.old_bmp.is_invalid() {
                SelectObject(self.mem_dc, self.old_bmp);
            }
            if !self.dib.is_invalid() {
                let _ = DeleteObject(self.dib.into());
            }
            let _ = DeleteDC(self.mem_dc);
        }
    }
}
