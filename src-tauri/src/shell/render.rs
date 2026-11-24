//! Software rendering of the taskbar strip and tray icons.
//!
//! System fonts (Segoe UI Bold, Segoe Fluent Icons) are memory-mapped rather than read into the
//! heap, so they cost shared file-backed pages instead of private memory.
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::OnceLock;
use windows_sys::Win32::{
    Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, GetFileSize, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING},
    System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_READ, PAGE_READONLY},
};

pub const GLYPH_MOUSE: char = '\u{E962}';
pub const GLYPH_HEADPHONE: char = '\u{E7F6}';
pub const GLYPH_HEADSET: char = '\u{E95B}';
const GLYPH_BOLT: char = '\u{E945}';

pub struct Fonts {
    text: FontRef<'static>,
    icons: Option<FontRef<'static>>,
}

fn map_font(name: &str) -> Option<&'static [u8]> {
    let path: Vec<u16> = format!(r"C:\Windows\Fonts\{name}").encode_utf16().chain(Some(0)).collect();
    unsafe {
        let file = CreateFileW(path.as_ptr(), GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        if file == INVALID_HANDLE_VALUE {
            return None;
        }
        let size = GetFileSize(file, std::ptr::null_mut()) as usize;
        let mapping = CreateFileMappingW(file, std::ptr::null(), PAGE_READONLY, 0, 0, std::ptr::null());
        CloseHandle(file);
        if mapping.is_null() {
            return None;
        }
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        CloseHandle(mapping); // the view keeps the mapping alive
        if view.Value.is_null() || size == 0 {
            return None;
        }
        // The view is intentionally never unmapped: fonts live for the whole process.
        Some(std::slice::from_raw_parts(view.Value as *const u8, size))
    }
}

pub fn fonts() -> Option<&'static Fonts> {
    static FONTS: OnceLock<Option<Fonts>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let text = ["segoeuib.ttf", "segoeui.ttf"].iter().find_map(|f| map_font(f).and_then(|b| FontRef::try_from_slice(b).ok()));
            let icons = ["SegoeIcons.ttf", "segmdl2.ttf"].iter().find_map(|f| map_font(f).and_then(|b| FontRef::try_from_slice(b).ok()));
            if text.is_none() {
                log::error!("[render] Segoe UI not found; indicators cannot be drawn");
            }
            text.map(|text| Fonts { text, icons })
        })
        .as_ref()
}

/// Premultiplied RGBA canvas in f32.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h, px: vec![[0.0; 4]; (w * h) as usize] }
    }

    fn blend(&mut self, x: i32, y: i32, cov: f32, rgb: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return;
        }
        let a = (cov * alpha).clamp(0.0, 1.0);
        let p = &mut self.px[(y as u32 * self.w + x as u32) as usize];
        for i in 0..3 {
            p[i] = p[i] * (1.0 - a) + f32::from(rgb[i]) / 255.0 * a;
        }
        p[3] = p[3] * (1.0 - a) + a;
    }

    pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgb: [u8; 3], alpha: f32) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, 1.0, rgb, alpha);
            }
        }
    }

    /// Draws `s` with its baseline at `base`; returns the pen position after the last glyph.
    pub fn text(&mut self, font: &FontRef, size: f32, s: &str, mut x: f32, base: f32, rgb: [u8; 3], alpha: f32) -> f32 {
        let scale = PxScale::from(size);
        let sf = font.as_scaled(scale);
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(o) = font.outline_glyph(id.with_scale_and_position(scale, ab_glyph::point(x.round(), base.round()))) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| self.blend(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, cov, rgb, alpha));
            }
            x += sf.h_advance(id);
        }
        x
    }

    /// Premultiplied BGRA for `UpdateLayeredWindow`. `min_alpha` keeps empty pixels hit-testable.
    pub fn to_bgra_premultiplied(&self, min_alpha: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            let a = (p[3] * 255.0).round() as u8;
            if a < min_alpha {
                out.extend_from_slice(&[0, 0, 0, min_alpha]);
            } else {
                out.extend_from_slice(&[(p[2] * 255.0).round() as u8, (p[1] * 255.0).round() as u8, (p[0] * 255.0).round() as u8, a]);
            }
        }
        out
    }

    /// Straight (non-premultiplied) RGBA for tray icons.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            let a = p[3];
            let un = |c: f32| if a > 0.0 { (c / a * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            out.extend_from_slice(&[un(p[0]), un(p[1]), un(p[2]), (a * 255.0).round() as u8]);
        }
        out
    }
}

pub fn measure(font: &FontRef, size: f32, s: &str) -> f32 {
    let sf = font.as_scaled(PxScale::from(size));
    s.chars().map(|c| sf.h_advance(font.glyph_id(c))).sum()
}

/// One device in the taskbar strip.
#[derive(Clone, Debug, PartialEq)]
pub struct StripItem {
    /// Icon glyph, or `None` to draw `label` instead.
    pub glyph: Option<char>,
    pub label: String,
    pub text: String,
    /// `None` = taskbar text colour.
    pub color: Option<[u8; 3]>,
    pub dim: bool,
    pub charging: bool,
    pub tooltip: String,
}

pub struct RenderedStrip {
    pub bgra: Vec<u8>,
    pub w: u32,
    pub h: u32,
    /// Horizontal extent of each item, for tooltips.
    pub regions: Vec<(i32, i32)>,
}

pub fn render_strip(items: &[StripItem], height: u32, scale: f32, fg: [u8; 3]) -> Option<RenderedStrip> {
    let f = fonts()?;
    let size = 15.0 * scale;
    let (gap_icon, gap_item, pad, bolt) = (5.0 * scale, 16.0 * scale, 8.0 * scale, 11.0 * scale);
    let icon_w = |it: &StripItem| match (it.glyph, &f.icons) {
        (Some(g), Some(icons)) => measure(icons, size, &g.to_string()) + 0.7 * scale,
        _ => measure(&f.text, size, &it.label),
    };
    let item_w = |it: &StripItem| icon_w(it) + gap_icon + measure(&f.text, size, &it.text) + if it.charging { bolt + 2.0 * scale } else { 0.0 };
    let width = pad * 2.0 + items.iter().map(item_w).sum::<f32>() + gap_item * items.len().saturating_sub(1) as f32;
    let w = width.ceil().max(1.0) as u32;
    let mut c = Canvas::new(w, height);
    let base = (height as f32 + f.text.as_scaled(PxScale::from(size)).ascent() * 0.70) / 2.0;
    let mut x = pad;
    let mut regions = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            x += gap_item;
        }
        let start = x;
        let alpha = if it.dim { 0.45 } else { 1.0 };
        let color = it.color.unwrap_or(fg);
        match (it.glyph, &f.icons) {
            (Some(g), Some(icons)) => {
                // Fluent icons sit on the baseline; centre optically. They have no bold weight, so
                // overdraw with a sub-pixel offset for a heavier stroke matching the bold text.
                let ib = (height as f32 + icons.as_scaled(PxScale::from(size)).ascent() * 0.80) / 2.0;
                let g = g.to_string();
                c.text(icons, size, &g, x + 0.7 * scale, ib, fg, alpha);
                x = c.text(icons, size, &g, x, ib, fg, alpha) + 0.7 * scale;
            }
            _ => x = c.text(&f.text, size, &it.label, x, base, fg, alpha * 0.7),
        }
        x = c.text(&f.text, size, &it.text, x + gap_icon, base, color, alpha);
        if it.charging {
            if let Some(icons) = &f.icons {
                let ib = (height as f32 + icons.as_scaled(PxScale::from(bolt)).ascent() * 0.80) / 2.0;
                x = c.text(icons, bolt, &GLYPH_BOLT.to_string(), x + 2.0 * scale, ib, color, alpha);
            }
        }
        regions.push((start as i32 - (gap_item / 2.0) as i32, (x + gap_item / 2.0) as i32));
    }
    Some(RenderedStrip { bgra: c.to_bgra_premultiplied(1), w, h: height, regions })
}

/// Tray icon: battery number with a level bar underneath (fallback indicator).
pub fn render_level_icon(size: u32, text: &str, level: Option<u8>, fg: [u8; 3], bar: [u8; 3]) -> Option<Vec<u8>> {
    let f = fonts()?;
    let mut c = Canvas::new(size, size);
    let bar_h = (size / 8).max(2);
    let avail = (size - bar_h) as f32;
    let mut px = avail * 1.05;
    while measure(&f.text, px, text) > size as f32 + 1.0 && px > 6.0 {
        px -= 0.5;
    }
    let w = measure(&f.text, px, text);
    let base = (avail + f.text.as_scaled(PxScale::from(px)).ascent() * 0.72) / 2.0 - 0.5;
    c.text(&f.text, px, text, (size as f32 - w) / 2.0, base, fg, 1.0);
    let y0 = (size - bar_h) as i32;
    c.rect(0, y0, size as i32, size as i32, fg, 0.3);
    if let Some(l) = level {
        let fill = ((f32::from(l) / 100.0) * size as f32).round() as i32;
        c.rect(0, y0, fill, size as i32, bar, 1.0);
    }
    Some(c.to_rgba())
}

/// WhiteHub tray icon: a battery outline matching the app logo.
pub fn render_app_icon(size: u32, fg: [u8; 3]) -> Vec<u8> {
    let mut c = Canvas::new(size, size);
    let s = size as f32 / 16.0;
    let px = |v: f32| (v * s).round() as i32;
    let (x0, y0, x1, y1, t) = (px(1.0), px(4.0), px(13.0), px(12.0), px(1.0).max(1));
    c.rect(x0, y0, x1, y0 + t, fg, 1.0);
    c.rect(x0, y1 - t, x1, y1, fg, 1.0);
    c.rect(x0, y0, x0 + t, y1, fg, 1.0);
    c.rect(x1 - t, y0, x1, y1, fg, 1.0);
    c.rect(x1, px(6.0), x1 + t + px(0.5), px(10.0), fg, 1.0);
    c.rect(x0 + 2 * t, y0 + 2 * t, px(8.0), y1 - 2 * t, fg, 1.0);
    c.to_rgba()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_conversion() {
        let mut c = Canvas::new(2, 1);
        c.rect(0, 0, 1, 1, [255, 0, 0], 0.5);
        let px = c.to_bgra_premultiplied(1);
        assert_eq!(&px[..4], &[0, 0, 128, 128]);
        assert_eq!(&px[4..], &[0, 0, 0, 1], "empty pixels keep a hit-testable alpha");
        let rgba = c.to_rgba();
        assert_eq!(&rgba[..4], &[255, 0, 0, 128]);
    }

    #[test]
    fn strip_renders_with_system_fonts() {
        let items = [
            StripItem { glyph: Some(GLYPH_MOUSE), label: "M".into(), text: "82%".into(), color: None, dim: false, charging: false, tooltip: String::new() },
            StripItem { glyph: Some(GLYPH_HEADPHONE), label: "H".into(), text: "--".into(), color: None, dim: true, charging: true, tooltip: String::new() },
        ];
        let r = render_strip(&items, 72, 1.5, [255, 255, 255]).expect("Segoe UI is present on Windows 11");
        assert_eq!(r.h, 72);
        assert_eq!(r.bgra.len(), (r.w * r.h * 4) as usize);
        assert_eq!(r.regions.len(), 2);
        assert!(r.regions[0].1 <= r.regions[1].0 + 1);
        assert!(r.bgra.chunks(4).any(|p| p[3] > 200), "text pixels are drawn");
    }

    #[test]
    fn level_icon_fits_three_digits() {
        let px = render_level_icon(24, "100", Some(100), [255, 255, 255], [34, 197, 94]).unwrap();
        assert_eq!(px.len(), 24 * 24 * 4);
    }
}
