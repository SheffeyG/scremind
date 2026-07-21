use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::*;

use crate::config::Rgba;

use super::types::{OverlayViewState, OverlayWindowState};

#[derive(Debug, Default)]
pub struct OverlayRenderer {
    font: Option<OwnedFont>,
    cache: Option<SurfaceCache>,
}

impl OverlayRenderer {
    fn ensure_font(&mut self, view: &OverlayViewState) -> windows::core::Result<()> {
        if self.font.is_none() {
            self.font = Some(OwnedFont::new(view.font_size, &view.font_name_wide)?);
        }
        Ok(())
    }

    fn font_handle(&self) -> Option<HFONT> {
        self.font.as_ref().map(|font| font.handle())
    }

    fn ensure_cache(
        &mut self,
        source_hdc: HDC,
        view: &OverlayViewState,
    ) -> windows::core::Result<()> {
        if self.cache.is_none() {
            let cache = SurfaceCache::new(source_hdc, view.bounds)?;
            let font = self.font_handle();
            paint_background(cache.background.hdc(), &cache.rect, view.bg_color);
            paint_text_layer(cache.text.hdc(), &cache.rect, view, font)?;
            self.cache = Some(cache);
        }

        Ok(())
    }

    fn frame_hdc(&self) -> Option<HDC> {
        self.cache.as_ref().map(|cache| cache.frame.hdc())
    }

    fn background_hdc(&self) -> Option<HDC> {
        self.cache.as_ref().map(|cache| cache.background.hdc())
    }

    fn text_hdc(&self) -> Option<HDC> {
        self.cache.as_ref().map(|cache| cache.text.hdc())
    }

    fn rect(&self) -> Option<RECT> {
        self.cache.as_ref().map(|cache| cache.rect)
    }
}

pub unsafe fn initialize_renderer(
    hwnd: HWND,
    state: &mut OverlayWindowState,
) -> windows::core::Result<()> {
    let window_dc = WindowDc::acquire(hwnd)?;
    state.renderer.ensure_font(&state.view)?;
    state.renderer.ensure_cache(window_dc.hdc(), &state.view)
}

pub unsafe fn paint_overlay(hwnd: HWND, state: &mut OverlayWindowState) {
    let paint = PaintSession::begin(hwnd);
    let rect = state.view.bounds;

    if let Err(e) = state.renderer.ensure_font(&state.view) {
        log::error!(
            "Failed to initialize overlay font '{}': {}",
            state.view.font_name,
            e
        );
        return;
    }

    if let Err(e) = state.renderer.ensure_cache(paint.hdc(), &state.view) {
        log::error!(
            "Failed to initialize overlay surface cache: {}x{} ({})",
            rect.right - rect.left,
            rect.bottom - rect.top,
            e
        );
        return;
    }

    let Some(frame_hdc) = state.renderer.frame_hdc() else {
        log::error!("Overlay frame cache missing after initialization");
        return;
    };
    let Some(background_hdc) = state.renderer.background_hdc() else {
        log::error!("Overlay background cache missing after initialization");
        return;
    };
    let Some(text_hdc) = state.renderer.text_hdc() else {
        log::error!("Overlay text cache missing after initialization");
        return;
    };
    let Some(cache_rect) = state.renderer.rect() else {
        log::error!("Overlay cache rect missing after initialization");
        return;
    };

    reset_frame(frame_hdc, background_hdc, &cache_rect);
    compose_frame(frame_hdc, text_hdc, &cache_rect, state.view.fg_color.3);
    blit_frame(paint.hdc(), frame_hdc, &cache_rect);
}

fn paint_background(hdc: HDC, rect: &RECT, bg_color: Rgba) {
    let brush = OwnedBrush::solid(rgb_color(bg_color.0, bg_color.1, bg_color.2));
    unsafe {
        let _ = FillRect(hdc, rect, brush.handle());
    }
}

fn paint_text_layer(
    hdc: HDC,
    rect: &RECT,
    view: &OverlayViewState,
    font: Option<HFONT>,
) -> windows::core::Result<()> {
    let black_brush = OwnedBrush::solid(COLORREF(0));
    unsafe {
        let _ = FillRect(hdc, rect, black_brush.handle());
    }

    let Some(font) = font else {
        return Ok(());
    };

    let _font_guard = unsafe { SelectedFontGuard::select(hdc, font)? };
    unsafe {
        let _ = SetTextColor(
            hdc,
            rgb_color(view.fg_color.0, view.fg_color.1, view.fg_color.2),
        );
        let _ = SetBkMode(hdc, TRANSPARENT);

        let mut text_rect = *rect;
        let mut text_wide = view.time_wide.clone();
        let _ = DrawTextW(
            hdc,
            &mut text_wide,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }

    Ok(())
}

fn reset_frame(frame_hdc: HDC, background_hdc: HDC, rect: &RECT) {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    unsafe {
        let _ = BitBlt(
            frame_hdc,
            0,
            0,
            width,
            height,
            background_hdc,
            0,
            0,
            SRCCOPY,
        );
    }
}

fn compose_frame(frame_hdc: HDC, text_hdc: HDC, rect: &RECT, alpha: u8) {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let blend_fn = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: 0,
    };

    unsafe {
        let _ = AlphaBlend(
            frame_hdc, 0, 0, width, height, text_hdc, 0, 0, width, height, blend_fn,
        );
    }
}

fn blit_frame(target_hdc: HDC, frame_hdc: HDC, rect: &RECT) {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    unsafe {
        let _ = BitBlt(target_hdc, 0, 0, width, height, frame_hdc, 0, 0, SRCCOPY);
    }
}

fn rgb_color(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

struct PaintSession {
    hwnd: HWND,
    ps: PAINTSTRUCT,
    hdc: HDC,
}

struct WindowDc {
    hwnd: HWND,
    hdc: HDC,
}

impl PaintSession {
    unsafe fn begin(hwnd: HWND) -> Self {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        Self { hwnd, ps, hdc }
    }

    fn hdc(&self) -> HDC {
        self.hdc
    }
}

impl Drop for PaintSession {
    fn drop(&mut self) {
        unsafe {
            let _ = EndPaint(self.hwnd, &self.ps);
        }
    }
}

impl WindowDc {
    unsafe fn acquire(hwnd: HWND) -> windows::core::Result<Self> {
        let hdc = GetDC(hwnd);
        if hdc.0.is_null() {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self { hwnd, hdc })
        }
    }

    fn hdc(&self) -> HDC {
        self.hdc
    }
}

impl Drop for WindowDc {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseDC(self.hwnd, self.hdc);
        }
    }
}

#[derive(Debug)]
struct SurfaceCache {
    rect: RECT,
    frame: PaintSurface,
    background: PaintSurface,
    text: PaintSurface,
}

impl SurfaceCache {
    fn new(source_hdc: HDC, rect: RECT) -> windows::core::Result<Self> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        Ok(Self {
            rect,
            frame: PaintSurface::new(source_hdc, width, height)
                .ok_or_else(windows::core::Error::from_win32)?,
            background: PaintSurface::new(source_hdc, width, height)
                .ok_or_else(windows::core::Error::from_win32)?,
            text: PaintSurface::new(source_hdc, width, height)
                .ok_or_else(windows::core::Error::from_win32)?,
        })
    }
}

#[derive(Debug)]
struct PaintSurface {
    _selection: SelectedBitmapGuard,
    _bitmap: OwnedBitmap,
    dc: OwnedCompatibleDc,
}

impl PaintSurface {
    fn new(source_hdc: HDC, width: i32, height: i32) -> Option<Self> {
        let dc = OwnedCompatibleDc::new(source_hdc)?;
        let bitmap = OwnedBitmap::new(source_hdc, width, height)?;
        let selection = unsafe { SelectedBitmapGuard::select(dc.handle(), bitmap.handle()).ok()? };
        Some(Self {
            _selection: selection,
            _bitmap: bitmap,
            dc,
        })
    }

    fn hdc(&self) -> HDC {
        self.dc.handle()
    }
}

#[derive(Debug)]
struct OwnedCompatibleDc(HDC);

impl OwnedCompatibleDc {
    fn new(source_hdc: HDC) -> Option<Self> {
        let hdc = unsafe { CreateCompatibleDC(source_hdc) };
        if hdc.0.is_null() {
            None
        } else {
            Some(Self(hdc))
        }
    }

    fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for OwnedCompatibleDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

#[derive(Debug)]
struct OwnedBitmap(HBITMAP);

impl OwnedBitmap {
    fn new(source_hdc: HDC, width: i32, height: i32) -> Option<Self> {
        let bitmap = unsafe { CreateCompatibleBitmap(source_hdc, width, height) };
        if bitmap.0.is_null() {
            None
        } else {
            Some(Self(bitmap))
        }
    }

    fn handle(&self) -> HBITMAP {
        self.0
    }
}

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

#[derive(Debug)]
struct OwnedBrush(HBRUSH);

impl OwnedBrush {
    fn solid(color: COLORREF) -> Self {
        Self(unsafe { CreateSolidBrush(color) })
    }

    fn handle(&self) -> HBRUSH {
        self.0
    }
}

impl Drop for OwnedBrush {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

#[derive(Debug)]
pub struct OwnedFont(HFONT);

impl OwnedFont {
    fn new(font_size: i32, font_name_wide: &[u16]) -> windows::core::Result<Self> {
        let font = unsafe {
            CreateFontW(
                font_size,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                DEFAULT_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR(font_name_wide.as_ptr()),
            )
        };

        if font.0.is_null() {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self(font))
        }
    }

    fn handle(&self) -> HFONT {
        self.0
    }
}

impl Drop for OwnedFont {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

#[derive(Debug)]
struct SelectedBitmapGuard {
    hdc: HDC,
    old: HGDIOBJ,
}

impl SelectedBitmapGuard {
    unsafe fn select(hdc: HDC, bitmap: HBITMAP) -> windows::core::Result<Self> {
        let old = SelectObject(hdc, HGDIOBJ(bitmap.0));
        if old.0.is_null() || old.0 as isize == GDI_ERROR as isize {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self { hdc, old })
        }
    }
}

impl Drop for SelectedBitmapGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.old);
        }
    }
}

#[derive(Debug)]
struct SelectedFontGuard {
    hdc: HDC,
    old: HGDIOBJ,
}

impl SelectedFontGuard {
    unsafe fn select(hdc: HDC, font: HFONT) -> windows::core::Result<Self> {
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        if old.0.is_null() || old.0 as isize == GDI_ERROR as isize {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self { hdc, old })
        }
    }
}

impl Drop for SelectedFontGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.old);
        }
    }
}
