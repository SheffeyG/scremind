use std::ptr::null_mut;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::config::Rgba;

use super::types::{OverlayViewState, OverlayWindowState};

#[derive(Debug, Default)]
pub struct OverlayRenderer {
    font: Option<OwnedFont>,
}

impl OverlayRenderer {
    fn ensure_font(&mut self, view: &OverlayViewState) -> windows::core::Result<()> {
        if self.font.is_none() {
            self.font = Some(OwnedFont::new(view.font_size, &view.font_name_wide)?);
        }
        Ok(())
    }

    fn font(&self) -> Option<&OwnedFont> {
        self.font.as_ref()
    }
}

pub unsafe fn paint_overlay(hwnd: HWND, state: &mut OverlayWindowState) {
    let paint = PaintSession::begin(hwnd);
    let rect = client_rect(hwnd);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }

    if let Err(e) = state.renderer.ensure_font(&state.view) {
        log::error!(
            "Failed to initialize overlay font '{}': {}",
            state.view.font_name,
            e
        );
        return;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let Some(frame_surface) = PaintSurface::new(paint.hdc(), width, height) else {
        log::error!(
            "Failed to create overlay frame surface: {}x{}",
            width,
            height
        );
        return;
    };
    let Some(text_surface) = PaintSurface::new(paint.hdc(), width, height) else {
        log::error!(
            "Failed to create overlay text surface: {}x{}",
            width,
            height
        );
        return;
    };

    paint_background(frame_surface.hdc(), &rect, state.view.bg_color);
    paint_text_layer(
        text_surface.hdc(),
        &rect,
        &state.view,
        state.renderer.font(),
    );
    compose_frame(
        frame_surface.hdc(),
        text_surface.hdc(),
        &rect,
        state.view.fg_color.3,
    );
    blit_frame(paint.hdc(), frame_surface.hdc(), &rect);
}

fn paint_background(hdc: HDC, rect: &RECT, bg_color: Rgba) {
    let brush = OwnedBrush::solid(rgb_color(bg_color.0, bg_color.1, bg_color.2));
    unsafe {
        let _ = FillRect(hdc, rect, brush.handle());
    }
}

fn paint_text_layer(hdc: HDC, rect: &RECT, view: &OverlayViewState, font: Option<&OwnedFont>) {
    let black_brush = OwnedBrush::solid(COLORREF(0));
    unsafe {
        let _ = FillRect(hdc, rect, black_brush.handle());
    }

    let Some(font) = font else {
        return;
    };

    let _font_guard = unsafe { SelectedFontGuard::select(hdc, font.handle()) };
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

fn client_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    }
    rect
}

fn rgb_color(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

struct PaintSession {
    hwnd: HWND,
    ps: PAINTSTRUCT,
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

struct PaintSurface {
    dc: OwnedCompatibleDc,
    _bitmap: OwnedBitmap,
    _selection: SelectedBitmapGuard,
}

impl PaintSurface {
    fn new(source_hdc: HDC, width: i32, height: i32) -> Option<Self> {
        let dc = OwnedCompatibleDc::new(source_hdc)?;
        let bitmap = OwnedBitmap::new(source_hdc, width, height)?;
        let selection = unsafe { SelectedBitmapGuard::select(dc.handle(), bitmap.handle()) };
        Some(Self {
            dc,
            _bitmap: bitmap,
            _selection: selection,
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
        if hdc.0 == null_mut() {
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
        if bitmap.0 == null_mut() {
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

        if font.0 == null_mut() {
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

struct SelectedBitmapGuard {
    hdc: HDC,
    old: HGDIOBJ,
}

impl SelectedBitmapGuard {
    unsafe fn select(hdc: HDC, bitmap: HBITMAP) -> Self {
        let old = SelectObject(hdc, HGDIOBJ(bitmap.0));
        Self { hdc, old }
    }
}

impl Drop for SelectedBitmapGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.old);
        }
    }
}

struct SelectedFontGuard {
    hdc: HDC,
    old: HGDIOBJ,
}

impl SelectedFontGuard {
    unsafe fn select(hdc: HDC, font: HFONT) -> Self {
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        Self { hdc, old }
    }
}

impl Drop for SelectedFontGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.old);
        }
    }
}
