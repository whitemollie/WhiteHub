//! Software rendering of the taskbar strip and tray icons.
//!
//! System fonts (Segoe UI Black/Bold, Segoe Fluent Icons) are memory-mapped rather than read into the
//! heap, so they cost shared file-backed pages instead of private memory.
use super::icons::{draw_mark, draw_shape, mark_width, Shape};
use crate::settings::ChargeMark;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::OnceLock;
use windows_sys::Win32::{
    Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, GetFileSize, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, INVALID_FILE_SIZE, OPEN_EXISTING},
    System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_READ, PAGE_READONLY},
};

pub const GLYPH_MOUSE: char = '\u{E962}';
pub const GLYPH_HEADPHONE: char = '\u{E7F6}';
pub const GLYPH_HEADSET: char = '\u{E95B}';
pub const GLYPH_KEYBOARD: char = '\u{E765}';
pub const GLYPH_GAMEPAD: char = '\u{E7FC}';
pub const GLYPH_BLUETOOTH: char = '\u{E702}';
/// Filled charging bolt (iOS-style): outline in a unit box, width/height = BOLT_ASPECT.
const BOLT: [(f32, f32); 6] = [(0.72, 0.0), (0.02, 0.58), (0.45, 0.58), (0.28, 1.0), (0.98, 0.42), (0.55, 0.42)];
pub const BOLT_ASPECT: f32 = 0.62;
/// Corner radius relative to the bolt height.
const BOLT_ROUND: f32 = 0.012;

/// Replaces every corner of a closed polygon with a circular fillet of radius `r`.
fn rounded(points: &[(f32, f32)], r: f32) -> Vec<(f32, f32)> {
    let n = points.len();
    let mut out = Vec::with_capacity(n * 8);
    for i in 0..n {
        let (a, v, b) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (la, lb) = (((a.0 - v.0).powi(2) + (a.1 - v.1).powi(2)).sqrt(), ((b.0 - v.0).powi(2) + (b.1 - v.1).powi(2)).sqrt());
        let d1 = ((a.0 - v.0) / la, (a.1 - v.1) / la);
        let d2 = ((b.0 - v.0) / lb, (b.1 - v.1) / lb);
        let theta = (d1.0 * d2.0 + d1.1 * d2.1).clamp(-1.0, 1.0).acos();
        if theta < 1e-3 || (std::f32::consts::PI - theta) < 1e-3 {
            out.push(v);
            continue;
        }
        // Tangent length, limited so neighbouring fillets never overlap.
        let t = (r / (theta / 2.0).tan()).min(la.min(lb) * 0.45);
        let rr = t * (theta / 2.0).tan();
        let p1 = (v.0 + d1.0 * t, v.1 + d1.1 * t);
        let p2 = (v.0 + d2.0 * t, v.1 + d2.1 * t);
        let bis = (d1.0 + d2.0, d1.1 + d2.1);
        let bl = (bis.0 * bis.0 + bis.1 * bis.1).sqrt();
        let dist = rr / (theta / 2.0).sin();
        let centre = (v.0 + bis.0 / bl * dist, v.1 + bis.1 / bl * dist);
        let (s, mut e) = ((p1.1 - centre.1).atan2(p1.0 - centre.0), (p2.1 - centre.1).atan2(p2.0 - centre.0));
        // Take the short way round.
        while e - s > std::f32::consts::PI {
            e -= 2.0 * std::f32::consts::PI;
        }
        while s - e > std::f32::consts::PI {
            e += 2.0 * std::f32::consts::PI;
        }
        for k in 0..=6 {
            let ang = s + (e - s) * k as f32 / 6.0;
            out.push((centre.0 + rr * ang.cos(), centre.1 + rr * ang.sin()));
        }
    }
    out
}

/// Outline of the bolt in a box with the given top left corner and height.
pub fn bolt_points(x: f32, y: f32, h: f32) -> Vec<(f32, f32)> {
    let w = h * BOLT_ASPECT;
    let pts: Vec<(f32, f32)> = BOLT.iter().map(|&(px, py)| (x + px * w, y + py * h)).collect();
    rounded(&pts, h * BOLT_ROUND)
}

/// Draws the filled bolt with its left edge at `x`, vertically centred on `cy`; returns the right edge.
pub fn draw_bolt(c: &mut Canvas, x: f32, cy: f32, h: f32, rgb: [u8; 3], alpha: f32) -> f32 {
    c.polygon(&bolt_points(x, cy - h / 2.0, h), rgb, alpha);
    x + h * BOLT_ASPECT
}

pub struct Fonts {
    text: FontRef<'static>,
    /// Heavier face for the taskbar strip.
    strip: FontRef<'static>,
    icons: Option<FontRef<'static>>,
}

fn map_font(name: &str) -> Option<&'static [u8]> {
    let path = crate::system::windows_dir().join("Fonts").join(name);
    let path: Vec<u16> = path.to_string_lossy().encode_utf16().chain(Some(0)).collect();
    unsafe {
        let file = CreateFileW(path.as_ptr(), GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        if file == INVALID_HANDLE_VALUE {
            return None;
        }
        let size = GetFileSize(file, std::ptr::null_mut());
        if size == 0 || size == INVALID_FILE_SIZE {
            CloseHandle(file);
            return None;
        }
        let mapping = CreateFileMappingW(file, std::ptr::null(), PAGE_READONLY, 0, 0, std::ptr::null());
        CloseHandle(file);
        if mapping.is_null() {
            return None;
        }
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        CloseHandle(mapping); // the view keeps the mapping alive
        if view.Value.is_null() {
            return None;
        }
        // The view is intentionally never unmapped: fonts live for the whole process.
        Some(std::slice::from_raw_parts(view.Value as *const u8, size as usize))
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
            let strip = map_font("seguibl.ttf").and_then(|b| FontRef::try_from_slice(b).ok());
            text.map(|text| Fonts { strip: strip.unwrap_or_else(|| text.clone()), text, icons })
        })
        .as_ref()
}

/// Colour and opacity of a drawing operation.
#[derive(Clone, Copy)]
pub struct Paint {
    pub rgb: [u8; 3],
    pub alpha: f32,
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

    /// Draws a shape given as a signed distance, in units of `unit` pixels from `origin`.
    pub fn shape(&mut self, origin: (f32, f32), unit: f32, size: (f32, f32), paint: Paint, distance: impl Fn((f32, f32)) -> f32) {
        let end = (origin.0 + size.0 * unit, origin.1 + size.1 * unit);
        for y in origin.1.floor() as i32..end.1.ceil() as i32 {
            for x in origin.0.floor() as i32..end.0.ceil() as i32 {
                let p = ((x as f32 + 0.5 - origin.0) / unit, (y as f32 + 0.5 - origin.1) / unit);
                let cover = (0.5 - distance(p) * unit).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.blend(x, y, cover, paint.rgb, paint.alpha);
                }
            }
        }
    }

    /// Fills a polygon (even-odd) with 4×4 supersampled anti-aliasing.
    pub fn polygon(&mut self, pts: &[(f32, f32)], rgb: [u8; 3], alpha: f32) {
        if pts.len() < 3 {
            return;
        }
        let inside = |x: f32, y: f32| {
            let mut inside = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let ((xi, yi), (xj, yj)) = (pts[i], pts[j]);
                if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            inside
        };
        let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        for py in (y0.floor() as i32)..=(y1.ceil() as i32) {
            for px in (x0.floor() as i32)..=(x1.ceil() as i32) {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        if inside(px as f32 + (sx as f32 + 0.5) / 4.0, py as f32 + (sy as f32 + 0.5) / 4.0) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend(px, py, hits as f32 / 16.0, rgb, alpha);
                }
            }
        }
    }

    /// Draws `s` with its baseline at `base`; returns the pen position after the last glyph.
    pub fn text(&mut self, font: &FontRef, size: f32, s: &str, mut x: f32, base: f32, paint: Paint) -> f32 {
        let scale = PxScale::from(size);
        let sf = font.as_scaled(scale);
        let mut prev = None;
        for c in s.chars() {
            let id = font.glyph_id(c);
            x += prev.map_or(0.0, |p| sf.kern(p, id));
            prev = Some(id);
            // Sub-pixel x keeps the letter spacing even; a whole-pixel baseline keeps edges crisp.
            if let Some(o) = font.outline_glyph(id.with_scale_and_position(scale, ab_glyph::point(x, base.round()))) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| self.blend(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, cov, paint.rgb, paint.alpha));
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
    let ids: Vec<_> = s.chars().map(|c| font.glyph_id(c)).collect();
    ids.iter().map(|&id| sf.h_advance(id)).sum::<f32>() + ids.windows(2).map(|p| sf.kern(p[0], p[1])).sum::<f32>()
}

/// Top and bottom of the drawn outlines relative to the baseline (y grows downwards).
fn vertical_extent(font: &FontRef, size: f32, s: &str) -> Option<(f32, f32)> {
    s.chars()
        .filter_map(|c| font.outline_glyph(font.glyph_id(c).with_scale(PxScale::from(size))))
        .map(|o| (o.px_bounds().min.y, o.px_bounds().max.y))
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
}

/// Baseline that centres `s` on `mid`.
fn centred_base(font: &FontRef, size: f32, s: &str, mid: f32) -> f32 {
    let (top, bottom) = vertical_extent(font, size, s).unwrap_or((0.0, 0.0));
    (mid - (top + bottom) / 2.0).round()
}

/// Font size at which digits are `height` pixels tall.
fn size_for_digits(font: &FontRef, height: f32) -> f32 {
    const PROBE: f32 = 1000.0;
    let (top, bottom) = vertical_extent(font, PROBE, "0").unwrap_or((-700.0, 0.0));
    height / (bottom - top) * PROBE
}

/// How a device is marked in the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StripIcon {
    Glyph(char),
    Letter,
    Shape(Shape),
    Hidden,
}

/// One device in the taskbar strip.
#[derive(Clone, Debug, PartialEq)]
pub struct StripItem {
    pub icon: StripIcon,
    /// Letter for `StripIcon::Letter`, and for glyphs if the icon font is missing.
    pub label: &'static str,
    pub text: String,
    /// `None` = taskbar text colour.
    pub color: Option<[u8; 3]>,
    pub dim: bool,
    /// `None` = not charging.
    pub charging: Option<ChargeMark>,
    pub tooltip: String,
}

pub struct RenderedStrip {
    pub bgra: Vec<u8>,
    pub w: u32,
    pub h: u32,
    /// Horizontal extent of each item, for tooltips.
    pub regions: Vec<(i32, i32)>,
}

/// Height of the strip's digits in logical pixels.
const DIGIT_HEIGHT: f32 = 10.7;

pub fn render_strip(items: &[StripItem], height: u32, scale: f32, fg: [u8; 3]) -> Option<RenderedStrip> {
    let f = fonts()?;
    let mid = height as f32 / 2.0;
    let (size, icon_size) = (size_for_digits(&f.strip, DIGIT_HEIGHT * scale), 14.0 * scale);
    let (gap_icon, gap_item, pad) = (5.5 * scale, 16.0 * scale, 8.0 * scale);
    let (mark_h, mark_gap) = (13.0 * scale, 4.0 * scale);
    // Shapes are a little larger than glyphs, which fill their box less.
    let unit = 15.0 * scale / 16.0;
    let icon_w = |it: &StripItem| match (it.icon, &f.icons) {
        (StripIcon::Hidden, _) => 0.0,
        (StripIcon::Shape(s), _) => s.width() * unit + gap_icon,
        (StripIcon::Glyph(g), Some(icons)) => measure(icons, icon_size, &g.to_string()) + 0.7 * scale + gap_icon,
        _ => measure(&f.strip, size, it.label) + gap_icon,
    };
    let mark_w = |m: ChargeMark| if m == ChargeMark::Hidden { 0.0 } else { mark_gap + mark_width(m, mark_h) };
    let item_w = |it: &StripItem| icon_w(it) + measure(&f.strip, size, &it.text) + it.charging.map_or(0.0, mark_w);
    let width = pad * 2.0 + items.iter().map(item_w).sum::<f32>() + gap_item * items.len().saturating_sub(1) as f32;
    let w = width.ceil().max(1.0) as u32;
    let mut c = Canvas::new(w, height);
    let base = centred_base(&f.strip, size, "0", mid);
    let mut x = pad;
    let mut regions = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            x += gap_item;
        }
        let start = x;
        let alpha = if it.dim { 0.45 } else { 1.0 };
        let color = it.color.unwrap_or(fg);
        let paint = Paint { rgb: fg, alpha };
        match (it.icon, &f.icons) {
            (StripIcon::Hidden, _) => {}
            (StripIcon::Shape(s), _) => x = draw_shape(&mut c, s, (x, mid), unit, paint, Paint { rgb: color, alpha }) + gap_icon,
            (StripIcon::Glyph(g), Some(icons)) => {
                // Fluent icons have no bold weight: overdraw with a sub-pixel offset for a heavier
                // stroke that matches the digits.
                let g = g.to_string();
                let ib = centred_base(icons, icon_size, &g, mid);
                c.text(icons, icon_size, &g, x + 0.7 * scale, ib, paint);
                x = c.text(icons, icon_size, &g, x, ib, paint) + 0.7 * scale + gap_icon;
            }
            _ => x = c.text(&f.strip, size, it.label, x, base, Paint { rgb: fg, alpha: alpha * 0.7 }) + gap_icon,
        }
        x = c.text(&f.strip, size, &it.text, x, base, Paint { rgb: color, alpha });
        if let Some(mark) = it.charging.filter(|&m| m != ChargeMark::Hidden) {
            x = draw_mark(&mut c, mark, (x + mark_gap, mid), mark_h, paint);
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
    c.text(&f.text, px, text, (size as f32 - w) / 2.0, base, Paint { rgb: fg, alpha: 1.0 });
    let y0 = (size - bar_h) as i32;
    c.rect(0, y0, size as i32, size as i32, fg, 0.3);
    if let Some(l) = level {
        let fill = ((f32::from(l) / 100.0) * size as f32).round() as i32;
        c.rect(0, y0, fill, size as i32, bar, 1.0);
    }
    Some(c.to_rgba())
}

/// Signed distance from `p` to a rounded rectangle (x, y, width, height, radius).
pub fn rounded_rect(p: (f32, f32), [x, y, w, h, r]: [f32; 5]) -> f32 {
    let qx = (p.0 - x - w / 2.0).abs() - w / 2.0 + r;
    let qy = (p.1 - y - h / 2.0).abs() - h / 2.0 + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// WhiteHub tray icon: the logo. Shapes and colours are those of `assets/app-icon.svg`
/// (its 3-unit border is left out, it disappears at icon sizes). Straight-alpha RGBA.
pub fn render_app_icon(size: u32) -> Vec<u8> {
    let unit = 1024.0 / size as f32;
    let cover = |d: f32| (0.5 - d / unit).clamp(0.0, 1.0);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let p = ((x as f32 + 0.5) * unit, (y as f32 + 0.5) * unit);
            let tile = cover(rounded_rect(p, [32.0, 32.0, 960.0, 960.0, 220.0]));
            let outline = cover(rounded_rect(p, [196.0, 352.0, 568.0, 320.0, 72.0]).abs() - 24.0);
            let tip = cover(rounded_rect(p, [788.0, 452.0, 48.0, 120.0, 20.0]));
            let charge = cover(rounded_rect(p, [268.0, 424.0, 248.0, 176.0, 28.0]));
            let mark = outline.max(tip).max(charge);
            let grey = (10.0 + 240.0 * mark).round() as u8;
            out.extend_from_slice(&[grey, grey, grey, (tile * 255.0).round() as u8]);
        }
    }
    out
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
            StripItem { icon: StripIcon::Glyph(GLYPH_MOUSE), label: "M", text: "82%".into(), color: None, dim: false, charging: None, tooltip: String::new() },
            StripItem { icon: StripIcon::Glyph(GLYPH_HEADPHONE), label: "H", text: "--".into(), color: None, dim: true, charging: Some(ChargeMark::Bolt), tooltip: String::new() },
        ];
        let r = render_strip(&items, 72, 1.5, [255, 255, 255]).expect("Segoe UI is present on Windows 11");
        assert_eq!(r.h, 72);
        assert_eq!(r.bgra.len(), (r.w * r.h * 4) as usize);
        assert_eq!(r.regions.len(), 2);
        assert!(r.regions[0].1 <= r.regions[1].0 + 1);
        assert!(r.bgra.chunks(4).any(|p| p[3] > 200), "text pixels are drawn");
    }

    #[test]
    fn digits_and_icons_share_one_centre() {
        let f = fonts().unwrap();
        for scale in [1.0f32, 1.25, 1.5, 2.0] {
            let mid = 48.0 * scale / 2.0;
            let size = size_for_digits(&f.strip, DIGIT_HEIGHT * scale);
            let check = |font: &FontRef, size: f32, s: &str| {
                let (t, b) = vertical_extent(font, size, s).unwrap();
                let centre = centred_base(font, size, s, mid) + (t + b) / 2.0;
                assert!((centre - mid).abs() <= 0.5, "{s} is off centre by {} at {scale}x", centre - mid);
            };
            check(&f.strip, size, "0");
            if let Some(icons) = &f.icons {
                check(icons, 14.0 * scale, &GLYPH_MOUSE.to_string());
                check(icons, 14.0 * scale, &GLYPH_HEADPHONE.to_string());
            }
        }
    }

    #[test]
    fn bolt_is_filled_and_rounded() {
        let mut c = Canvas::new(20, 30);
        let right = draw_bolt(&mut c, 2.0, 15.0, 24.0, [255, 255, 255], 1.0);
        assert!((right - (2.0 + 24.0 * BOLT_ASPECT)).abs() < 0.01);
        let solid = c.px.iter().filter(|p| p[3] > 0.99).count();
        assert!(solid > 40, "bolt interior is filled, got {solid} solid pixels");
        assert_eq!(rounded(&BOLT, 0.05).len(), BOLT.len() * 7);
    }

    /// Writes a preview of the strip on a dark taskbar colour: `WHITEHUB_PREVIEW=out.bmp cargo test preview -- --ignored`.
    #[test]
    #[ignore]
    fn preview() {
        let Ok(path) = std::env::var("WHITEHUB_PREVIEW") else { return };
        let item = |icon, text: &str, mark| StripItem { icon, label: "M", text: text.into(), color: None, dim: false, charging: mark, tooltip: String::new() };
        use crate::battery::DeviceKind::*;
        let items = [
            item(StripIcon::Glyph(GLYPH_MOUSE), "61%", Some(ChargeMark::Bolt)),
            item(StripIcon::Glyph(GLYPH_HEADPHONE), "47%", Some(ChargeMark::BoltOutline)),
            item(StripIcon::Shape(Shape::Filled(Mouse)), "61%", Some(ChargeMark::Plug)),
            item(StripIcon::Shape(Shape::Filled(Headset)), "47%", Some(ChargeMark::Plus)),
            item(StripIcon::Shape(Shape::Filled(Keyboard)), "91%", Some(ChargeMark::Hidden)),
            item(StripIcon::Shape(Shape::Filled(Gamepad)), "Full", None),
            item(StripIcon::Shape(Shape::Filled(Other)), "52%", None),
            item(StripIcon::Shape(Shape::Battery(Some(61))), "61%", None),
            item(StripIcon::Shape(Shape::Ring(Some(47))), "47%", None),
            item(StripIcon::Letter, "61%", None),
            item(StripIcon::Hidden, "61%", None),
        ];
        let scale: f32 = std::env::var("WHITEHUB_PREVIEW_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5);
        let r = render_strip(&items, (48.0 * scale) as u32, scale, [255, 255, 255]).unwrap();
        let mut bmp = Vec::new();
        let size = 54 + r.w * r.h * 4;
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&size.to_le_bytes());
        bmp.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
        bmp.extend_from_slice(&(r.w as i32).to_le_bytes());
        bmp.extend_from_slice(&(-(r.h as i32)).to_le_bytes());
        bmp.extend_from_slice(&[1, 0, 32, 0, 0, 0, 0, 0]);
        bmp.extend_from_slice(&[0u8; 20]);
        for p in r.bgra.chunks(4) {
            // Composite over the Windows 11 dark taskbar colour (#1C1C1C).
            let a = f32::from(p[3]) / 255.0;
            let mix = |c: u8| (f32::from(c) + 28.0 * (1.0 - a)).round() as u8;
            bmp.extend_from_slice(&[mix(p[0]), mix(p[1]), mix(p[2]), 255]);
        }
        std::fs::write(path, bmp).unwrap();
    }

    #[test]
    fn app_icon_is_the_logo() {
        for size in [16u32, 20, 24, 32, 64] {
            let px = render_app_icon(size);
            assert_eq!(px.len(), (size * size * 4) as usize);
            // Pixel that contains a point of the 1024-unit logo.
            let at = |x: u32, y: u32| {
                let i = (((y * size / 1024) * size + x * size / 1024) * 4) as usize;
                [px[i], px[i + 1], px[i + 2], px[i + 3]]
            };
            assert_eq!(at(0, 0)[3], 0, "the corner is transparent at {size}");
            assert_eq!(at(512, 200), [10, 10, 10, 255], "black tile at {size}");
            assert_eq!(at(400, 512), [250, 250, 250, 255], "white charge at {size}");
        }
    }

    #[test]
    fn level_icon_fits_three_digits() {
        let px = render_level_icon(24, "100", Some(100), [255, 255, 255], [34, 197, 94]).unwrap();
        assert_eq!(px.len(), 24 * 24 * 4);
    }
}
