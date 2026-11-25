//! Strip icons and charging marks that are drawn as shapes instead of font glyphs.
//!
//! Shapes are signed distances in a box 16 units high (negative inside), so they are sharp at every
//! scale. The previews in the window (`src/components/StripIcons.tsx`) use the same coordinates.
use super::render::{bolt_points, draw_bolt, rounded_rect, Canvas, Paint, BOLT_ASPECT};
use crate::{battery::DeviceKind, settings::ChargeMark};

type Point = (f32, f32);

const BOX: f32 = 16.0;
/// Height of the charging marks in box units.
const MARK: f32 = 13.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Filled(DeviceKind),
    /// Battery filled to the level in percent.
    Battery(Option<u8>),
    /// Ring filled to the level in percent.
    Ring(Option<u8>),
}

impl Shape {
    /// Width in box units.
    pub fn width(self) -> f32 {
        match self {
            Shape::Battery(_) => 21.0,
            _ => BOX,
        }
    }
}

fn cut(shape: f32, hole: f32) -> f32 {
    shape.max(-hole)
}

/// Distance to a line from `a` to `b` with the given half width.
fn line(p: Point, a: Point, b: Point, half: f32) -> f32 {
    let (d, v) = ((b.0 - a.0, b.1 - a.1), (p.0 - a.0, p.1 - a.1));
    let t = ((v.0 * d.0 + v.1 * d.1) / (d.0 * d.0 + d.1 * d.1)).clamp(0.0, 1.0);
    (v.0 - d.0 * t).hypot(v.1 - d.1 * t) - half
}

fn circle(p: Point, centre: Point, r: f32) -> f32 {
    (p.0 - centre.0).hypot(p.1 - centre.1) - r
}

fn filled(kind: DeviceKind, p: Point) -> f32 {
    let rect = |x, y, w, h, r| rounded_rect(p, [x, y, w, h, r]);
    match kind {
        DeviceKind::Mouse => {
            let buttons = rect(7.4, 0.0, 1.2, 6.8, 0.0).min(rect(2.0, 6.2, 12.0, 1.2, 0.0));
            cut(rect(3.0, 1.0, 10.0, 14.0, 4.4), buttons)
        }
        DeviceKind::Headset => {
            let band = (circle(p, (8.0, 8.0), 6.0).abs() - 1.0).max(p.1 - 8.0);
            let arms = rect(1.0, 8.0, 2.0, 4.0, 0.0).min(rect(13.0, 8.0, 2.0, 4.0, 0.0));
            let cups = rect(1.0, 8.6, 4.6, 6.2, 1.6).min(rect(10.4, 8.6, 4.6, 6.2, 1.6));
            band.min(arms).min(cups)
        }
        DeviceKind::Keyboard => {
            let keys = (0..4).map(|i| rect(2.6 + 3.0 * i as f32, 5.6, 1.6, 1.6, 0.2)).fold(f32::MAX, f32::min);
            cut(rect(0.4, 3.2, 15.2, 9.6, 2.0), keys.min(rect(4.6, 9.2, 6.8, 1.5, 0.2)))
        }
        DeviceKind::Gamepad => {
            let cross = rect(3.4, 7.3, 3.8, 1.2, 0.0).min(rect(4.7, 6.0, 1.2, 3.8, 0.0));
            let dots = circle(p, (10.4, 9.0), 0.8).min(circle(p, (12.2, 6.9), 0.8));
            cut(rect(0.6, 3.4, 14.8, 9.2, 4.4), cross.min(dots))
        }
        DeviceKind::Other => {
            let rune = [(5.4, 5.5), (10.4, 10.4), (8.0, 12.7), (8.0, 3.3), (10.4, 5.6), (5.4, 10.5)];
            let rune = rune.windows(2).map(|s| line(p, s[0], s[1], 0.6)).fold(f32::MAX, f32::min);
            cut(rect(2.4, 0.8, 11.2, 14.4, 5.6), rune)
        }
    }
}

fn ring(p: Point) -> f32 {
    circle(p, (8.0, 8.0), 5.8).abs() - 1.2
}

/// Part of the ring that is filled, clockwise from the top.
fn ring_part(p: Point, percent: u8) -> f32 {
    if percent >= 100 {
        return ring(p);
    }
    let angle = (p.0 - 8.0).atan2(8.0 - p.1).rem_euclid(std::f32::consts::TAU);
    ring(p).max((angle - std::f32::consts::TAU * f32::from(percent) / 100.0) * 5.8)
}

/// Draws `shape` with its left edge at `x`, centred on `mid`; returns the right edge.
pub fn draw_shape(c: &mut Canvas, shape: Shape, (x, mid): Point, unit: f32, fg: Paint, level: Paint) -> f32 {
    let origin = (x, mid - BOX / 2.0 * unit);
    let size = (shape.width(), BOX);
    match shape {
        Shape::Filled(kind) => c.shape(origin, unit, size, fg, |p| filled(kind, p)),
        Shape::Battery(percent) => {
            c.shape(origin, unit, size, fg, |p| (rounded_rect(p, [0.8, 3.6, 17.0, 8.8, 2.4]).abs() - 0.75).min(rounded_rect(p, [18.8, 6.2, 1.8, 3.6, 0.8])));
            if let Some(percent) = percent.filter(|&l| l > 0) {
                let w = (13.4 * f32::from(percent.min(100)) / 100.0).max(1.2);
                c.shape(origin, unit, size, level, |p| rounded_rect(p, [2.6, 5.4, w, 5.2, 0.6]));
            }
        }
        Shape::Ring(percent) => {
            c.shape(origin, unit, size, Paint { alpha: fg.alpha * 0.28, ..fg }, ring);
            if let Some(percent) = percent.filter(|&l| l > 0) {
                c.shape(origin, unit, size, level, |p| ring_part(p, percent));
            }
        }
    }
    x + shape.width() * unit
}

/// Width of a charging mark that is `h` pixels high.
pub fn mark_width(mark: ChargeMark, h: f32) -> f32 {
    match mark {
        ChargeMark::Bolt | ChargeMark::BoltOutline => h * BOLT_ASPECT,
        ChargeMark::Plug => h / MARK * 12.0,
        ChargeMark::Plus => h / MARK * 10.0,
        ChargeMark::Hidden => 0.0,
    }
}

/// Draws a charging mark with its left edge at `x`, centred on `mid`; returns the right edge.
pub fn draw_mark(c: &mut Canvas, mark: ChargeMark, (x, mid): Point, h: f32, paint: Paint) -> f32 {
    let unit = h / MARK;
    let origin = (x, mid - BOX / 2.0 * unit);
    match mark {
        ChargeMark::Bolt => {
            draw_bolt(c, x, mid, h, paint.rgb, paint.alpha);
        }
        ChargeMark::BoltOutline => {
            let pts = bolt_points(0.0, (BOX - MARK) / 2.0, MARK);
            let edges: Vec<(Point, Point)> = (0..pts.len()).map(|i| (pts[i], pts[(i + 1) % pts.len()])).collect();
            c.shape(origin, unit, (BOX, BOX), paint, |p| edges.iter().map(|&(a, b)| line(p, a, b, 0.6)).fold(f32::MAX, f32::min));
        }
        ChargeMark::Plug => c.shape(origin, unit, (12.0, BOX), paint, |p| {
            let rect = |x, y, w, h, r| rounded_rect(p, [x, y, w, h, r]);
            let body = rect(1.4, 1.9, 9.2, 9.4, 3.6).max(5.5 - p.1);
            body.min(rect(2.8, 1.5, 1.6, 4.5, 0.0)).min(rect(7.6, 1.5, 1.6, 4.5, 0.0)).min(rect(5.2, 11.0, 1.6, 3.6, 0.0))
        }),
        ChargeMark::Plus => c.shape(origin, unit, (10.0, BOX), paint, |p| {
            rounded_rect(p, [3.9, 4.0, 2.2, 8.0, 0.4]).min(rounded_rect(p, [1.0, 6.9, 8.0, 2.2, 0.4]))
        }),
        ChargeMark::Hidden => {}
    }
    x + mark_width(mark, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [DeviceKind; 5] = [DeviceKind::Mouse, DeviceKind::Keyboard, DeviceKind::Headset, DeviceKind::Gamepad, DeviceKind::Other];
    const WHITE: Paint = Paint { rgb: [255, 255, 255], alpha: 1.0 };

    /// Sum of the opacity that `draw` leaves on an empty canvas.
    fn ink(draw: impl FnOnce(&mut Canvas)) -> f32 {
        let mut c = Canvas::new(64, 48);
        draw(&mut c);
        c.to_rgba().chunks(4).map(|p| f32::from(p[3]) / 255.0).sum()
    }

    fn shape_ink(shape: Shape) -> f32 {
        ink(|c| {
            draw_shape(c, shape, (4.0, 24.0), 2.0, WHITE, WHITE);
        })
    }

    #[test]
    fn every_kind_has_a_filled_icon() {
        for kind in KINDS {
            assert!(shape_ink(Shape::Filled(kind)) > 200.0, "{kind:?}");
        }
    }

    #[test]
    fn level_shapes_grow_with_the_level() {
        for shape in [Shape::Battery as fn(Option<u8>) -> Shape, Shape::Ring] {
            let inks: Vec<f32> = [None, Some(0), Some(25), Some(60), Some(100)].map(|l| shape_ink(shape(l))).to_vec();
            assert_eq!(inks[0], inks[1], "an empty battery looks like an unknown one");
            assert!(inks[1..].windows(2).all(|w| w[0] < w[1]), "{inks:?}");
        }
        assert_eq!(shape_ink(Shape::Battery(Some(100))), shape_ink(Shape::Battery(Some(255))));
    }

    #[test]
    fn half_a_ring_fills_the_right_side() {
        assert!(ring_part((13.8, 8.0), 50) < 0.0);
        assert!(ring_part((2.2, 8.0), 50) > 0.0);
        assert!(ring_part((2.2, 8.0), 100) < 0.0);
    }

    #[test]
    fn charging_marks_stay_within_their_width() {
        for mark in [ChargeMark::Bolt, ChargeMark::BoltOutline, ChargeMark::Plug, ChargeMark::Plus] {
            let mut c = Canvas::new(64, 48);
            let right = draw_mark(&mut c, mark, (10.0, 24.0), 26.0, WHITE);
            assert!((right - 10.0 - mark_width(mark, 26.0)).abs() < 0.01);
            let px = c.to_rgba();
            let inked: Vec<usize> = (0..64).filter(|x| (0..48).any(|y| px[(y * 64 + x) * 4 + 3] > 32)).collect();
            let (first, last) = (*inked.first().unwrap() as f32, *inked.last().unwrap() as f32);
            assert!(first >= 8.0 && last <= right + 2.0, "{mark:?}: {first}..{last} of {right}");
        }
        assert_eq!(ink(|c| { draw_mark(c, ChargeMark::Hidden, (10.0, 24.0), 26.0, WHITE); }), 0.0);
    }
}
