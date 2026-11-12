//! Hand-drawn symbols and the large glassy pictures for each section (all drawn in code).

use crate::ui_kit::{alpha, mix};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Sparkles,
    Bubble,
    Shield,
    Bolt,
    Grid,
    Folder,
    Lens,
    Cloud,
    Sliders,
    Chart,
    Trash,
    Mail,
    Wrench,
    Power,
    Rocket,
    Copy,
    Download,
    Bug,
    Disk,
    Leftovers,
    Hammer,
}

fn poly(painter: &Painter, pts: Vec<Pos2>, fill: Color32) {
    painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
}

fn sparkle(painter: &Painter, c: Pos2, r: f32, color: Color32) {
    let k = r * 0.26;
    poly(
        painter,
        vec![
            c + Vec2::new(0.0, -r),
            c + Vec2::new(k, -k),
            c + Vec2::new(r, 0.0),
            c + Vec2::new(k, k),
            c + Vec2::new(0.0, r),
            c + Vec2::new(-k, k),
            c + Vec2::new(-r, 0.0),
            c + Vec2::new(-k, -k),
        ],
        color,
    );
}

/// A simple white symbol filling `rect`.
pub fn glyph(painter: &Painter, g: Glyph, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let u = s / 24.0; // design grid of 24 units
    let at = |x: f32, y: f32| Pos2::new(c.x + (x - 12.0) * u, c.y + (y - 12.0) * u);
    let stroke = Stroke::new((2.0 * u).max(1.2), color);
    match g {
        Glyph::Sparkles => {
            sparkle(painter, at(10.0, 13.0), 8.0 * u, color);
            sparkle(painter, at(18.5, 5.5), 3.5 * u, color);
            sparkle(painter, at(19.0, 18.5), 2.5 * u, color);
        }
        Glyph::Bubble => {
            painter.circle_stroke(at(12.0, 12.0), 9.0 * u, stroke);
            sparkle(painter, at(12.0, 12.0), 5.0 * u, color);
            painter.circle_filled(at(17.0, 6.5), 1.6 * u, color);
        }
        Glyph::Shield => {
            let pts = vec![at(12.0, 2.5), at(20.0, 5.5), at(19.0, 14.0), at(12.0, 21.5), at(5.0, 14.0), at(4.0, 5.5)];
            painter.add(Shape::closed_line(pts, stroke));
            painter.add(Shape::line(vec![at(8.5, 12.0), at(11.0, 14.5), at(15.5, 9.0)], stroke));
        }
        Glyph::Bolt => {
            poly(painter, vec![at(14.0, 1.5), at(5.0, 13.5), at(11.0, 13.5)], color);
            poly(painter, vec![at(11.0, 13.5), at(13.0, 10.5), at(19.0, 10.5), at(10.0, 22.5)], color);
            poly(painter, vec![at(14.0, 1.5), at(11.0, 13.5), at(13.0, 10.5)], color);
            poly(painter, vec![at(5.0, 13.5), at(13.0, 10.5), at(11.0, 13.5)], color);
        }
        Glyph::Grid => {
            for (x, y) in [(4.0, 4.0), (13.0, 4.0), (4.0, 13.0), (13.0, 13.0)] {
                let r = Rect::from_min_size(at(x, y), Vec2::splat(7.0 * u));
                painter.rect_filled(r, 2.0 * u, color);
            }
        }
        Glyph::Folder => {
            let back = Rect::from_min_max(at(3.0, 5.0), at(21.0, 19.5));
            painter.rect_filled(back, 2.0 * u, alpha(color, 150));
            painter.rect_filled(Rect::from_min_max(at(3.0, 4.0), at(10.0, 8.0)), 1.5 * u, alpha(color, 150));
            painter.rect_filled(Rect::from_min_max(at(3.0, 8.0), at(21.0, 19.5)), 2.0 * u, color);
        }
        Glyph::Lens => {
            painter.circle_stroke(at(10.5, 10.5), 6.5 * u, Stroke::new(2.4 * u, color));
            painter.line_segment([at(15.5, 15.5), at(20.5, 20.5)], Stroke::new(3.0 * u, color));
        }
        Glyph::Cloud => {
            painter.circle_filled(at(9.0, 13.0), 5.0 * u, color);
            painter.circle_filled(at(14.5, 10.5), 6.0 * u, color);
            painter.circle_filled(at(18.5, 14.5), 3.8 * u, color);
            painter.rect_filled(Rect::from_min_max(at(5.0, 13.0), at(19.0, 18.3)), 2.5 * u, color);
        }
        Glyph::Sliders => {
            for (i, k) in [(0, 8.0), (1, 15.0), (2, 11.0)] {
                let y = 6.0 + i as f32 * 6.0;
                painter.line_segment([at(4.0, y), at(20.0, y)], Stroke::new(1.8 * u, alpha(color, 170)));
                painter.circle_filled(at(k, y), 2.6 * u, color);
            }
        }
        Glyph::Chart => {
            painter.rect_stroke(Rect::from_min_max(at(3.0, 3.0), at(21.0, 21.0)), 4.0 * u, stroke, egui::StrokeKind::Middle);
            painter.add(Shape::line(vec![at(6.5, 16.0), at(10.0, 11.5), at(13.0, 14.0), at(17.5, 7.5)], stroke));
        }
        Glyph::Trash => {
            painter.rect_filled(Rect::from_min_max(at(5.0, 5.5), at(19.0, 7.5)), u, color);
            painter.rect_filled(Rect::from_min_max(at(10.0, 3.0), at(14.0, 5.5)), u, color);
            poly(painter, vec![at(6.5, 8.5), at(17.5, 8.5), at(16.5, 21.0), at(7.5, 21.0)], color);
        }
        Glyph::Mail => {
            let r = Rect::from_min_max(at(3.0, 6.0), at(21.0, 18.0));
            painter.rect_filled(r, 2.0 * u, color);
            painter.add(Shape::line(vec![at(4.0, 7.0), at(12.0, 13.0), at(20.0, 7.0)], Stroke::new(1.6 * u, Color32::WHITE)));
        }
        Glyph::Wrench => {
            painter.line_segment([at(6.0, 18.0), at(14.0, 10.0)], Stroke::new(3.2 * u, color));
            painter.circle_stroke(at(16.0, 8.0), 3.8 * u, Stroke::new(2.2 * u, color));
        }
        Glyph::Power => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = -PI / 2.0 + 0.6 + (TAU - 1.2) * i as f32 / 24.0;
                    at(12.0, 13.0) + Vec2::angled(a) * 7.5 * u
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
            painter.line_segment([at(12.0, 3.0), at(12.0, 12.0)], stroke);
        }
        Glyph::Rocket => {
            poly(painter, vec![at(12.0, 2.0), at(16.0, 8.0), at(16.0, 16.0), at(8.0, 16.0), at(8.0, 8.0)], color);
            poly(painter, vec![at(8.0, 12.0), at(4.5, 18.0), at(8.0, 16.0)], color);
            poly(painter, vec![at(16.0, 12.0), at(16.0, 16.0), at(19.5, 18.0)], color);
            painter.circle_filled(at(12.0, 19.5), 2.0 * u, alpha(color, 170));
        }
        Glyph::Copy => {
            painter.rect_stroke(Rect::from_min_max(at(4.0, 4.0), at(15.0, 15.0)), 2.0 * u, stroke, egui::StrokeKind::Middle);
            painter.rect_filled(Rect::from_min_max(at(9.0, 9.0), at(20.0, 20.0)), 2.0 * u, color);
        }
        Glyph::Download => {
            painter.line_segment([at(12.0, 3.0), at(12.0, 15.0)], Stroke::new(2.6 * u, color));
            painter.add(Shape::line(vec![at(7.0, 10.5), at(12.0, 15.5), at(17.0, 10.5)], Stroke::new(2.6 * u, color)));
            painter.line_segment([at(4.0, 20.0), at(20.0, 20.0)], Stroke::new(2.6 * u, color));
        }
        Glyph::Bug => {
            painter.circle_filled(at(12.0, 13.5), 6.0 * u, color);
            painter.circle_filled(at(12.0, 6.5), 3.0 * u, color);
            for (a, b) in [((6.0, 11.0), (3.0, 9.0)), ((18.0, 11.0), (21.0, 9.0)), ((6.0, 16.0), (3.0, 18.0)), ((18.0, 16.0), (21.0, 18.0))] {
                painter.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
            }
        }
        Glyph::Disk => {
            painter.rect_filled(Rect::from_min_max(at(3.0, 6.0), at(21.0, 18.0)), 3.0 * u, color);
            painter.circle_filled(at(17.0, 14.5), 1.3 * u, alpha(Color32::BLACK, 110));
            painter.line_segment([at(6.0, 14.5), at(12.0, 14.5)], Stroke::new(1.4 * u, alpha(Color32::BLACK, 110)));
        }
        Glyph::Leftovers => {
            painter.rect_filled(Rect::from_min_max(at(4.0, 4.0), at(14.0, 14.0)), 2.0 * u, alpha(color, 160));
            painter.circle_filled(at(17.0, 17.0), 4.0 * u, color);
            painter.circle_filled(at(8.0, 19.0), 2.0 * u, color);
            painter.circle_filled(at(19.0, 7.0), 1.6 * u, color);
        }
        Glyph::Hammer => {
            painter.line_segment([at(13.0, 9.0), at(5.0, 20.0)], Stroke::new(2.6 * u, color));
            poly(painter, vec![at(9.0, 4.0), at(17.0, 4.0), at(20.0, 8.0), at(20.0, 11.0), at(9.0, 8.0)], color);
        }
    }
}

// ---------------------------------------------------------------- big pictures

#[derive(Clone, Copy)]
pub enum Outline {
    Squircle,
    Circle,
    Octagon,
    Chevron,
    Hexagon,
    Blob,
    Cloud,
}

fn outline_points(o: Outline, c: Pos2, r: f32) -> Vec<Pos2> {
    let n = 96;
    let squash = 0.82;
    let pt = |a: f32, rr: f32| Pos2::new(c.x + a.cos() * rr, c.y + a.sin() * rr * squash);
    match o {
        Outline::Circle => (0..n).map(|i| pt(TAU * i as f32 / n as f32, r)).collect(),
        Outline::Squircle => (0..n)
            .map(|i| {
                let a = TAU * i as f32 / n as f32;
                let (ca, sa) = (a.cos(), a.sin());
                let p = 4.0;
                let x = ca.signum() * ca.abs().powf(2.0 / p);
                let y = sa.signum() * sa.abs().powf(2.0 / p);
                Pos2::new(c.x + x * r, c.y + y * r * squash)
            })
            .collect(),
        Outline::Blob => (0..n)
            .map(|i| {
                let a = TAU * i as f32 / n as f32;
                let rr = r * (1.0 - 0.10 * (4.0 * a).cos().powi(2) * 0.9 + 0.04 * (4.0 * a).cos());
                let (ca, sa) = (a.cos(), a.sin());
                let x = ca.signum() * ca.abs().powf(0.7);
                let y = sa.signum() * sa.abs().powf(0.7);
                Pos2::new(c.x + x * rr, c.y + y * rr * squash)
            })
            .collect(),
        Outline::Octagon | Outline::Hexagon => {
            let sides = if matches!(o, Outline::Octagon) { 8 } else { 6 };
            let rot = if sides == 8 { PI / 8.0 } else { 0.0 };
            let corner = r * 0.16;
            let mut pts = vec![];
            for k in 0..sides {
                let a0 = rot + TAU * k as f32 / sides as f32;
                let v = Vec2::angled(a0) * (r - corner);
                for j in 0..8 {
                    let a = a0 - PI / sides as f32 + (TAU / sides as f32) * j as f32 / 7.0;
                    let p = c + v + Vec2::angled(a) * corner;
                    pts.push(Pos2::new(p.x, c.y + (p.y - c.y) * squash));
                }
            }
            pts
        }
        Outline::Chevron => {
            // a banner whose bottom edge points up into a notch
            let w = r;
            let h = r * 0.95;
            let mut pts = vec![];
            let top = c.y - h * 0.75;
            let bottom = c.y + h * 0.75;
            for i in 0..=16 {
                let t = i as f32 / 16.0;
                let x = c.x - w + 2.0 * w * t;
                let y = top + (1.0 - (2.0 * t - 1.0).powi(2)).sqrt() * -h * 0.25 + h * 0.25;
                pts.push(Pos2::new(x, y));
            }
            pts.push(Pos2::new(c.x + w, bottom));
            pts.push(Pos2::new(c.x, bottom - h * 0.35));
            pts.push(Pos2::new(c.x - w, bottom));
            pts
        }
        Outline::Cloud => (0..n)
            .map(|i| {
                let a = TAU * i as f32 / n as f32;
                let rr = r * (0.88 + 0.12 * (3.0 * a + 0.6).cos().abs());
                pt(a, rr)
            })
            .collect(),
    }
}

/// Fills any simple outline with a top-to-bottom gradient (a fan from the middle).
fn fill_gradient(painter: &Painter, pts: &[Pos2], top: Color32, bottom: Color32) {
    if pts.len() < 3 {
        return;
    }
    let (mut y0, mut y1) = (f32::MAX, f32::MIN);
    for p in pts {
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    let lerp = |t: f32| -> Color32 {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgba_premultiplied(l(top.r(), bottom.r()), l(top.g(), bottom.g()), l(top.b(), bottom.b()), l(top.a(), bottom.a()))
    };
    let col = |y: f32| lerp(((y - y0) / (y1 - y0).max(1.0)).clamp(0.0, 1.0));
    let cx = pts.iter().map(|p| p.x).sum::<f32>() / pts.len() as f32;
    let cy = pts.iter().map(|p| p.y).sum::<f32>() / pts.len() as f32;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(Pos2::new(cx, cy), col(cy));
    for p in pts {
        mesh.colored_vertex(*p, col(p.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

/// A glassy 3D tile with a symbol on it.
pub fn hero(painter: &Painter, outline: Outline, g: Glyph, c: Pos2, r: f32, base: Color32) {
    // ground shadow and thickness
    let shadow = outline_points(outline, c + Vec2::new(0.0, r * 0.22), r * 0.96);
    fill_gradient(painter, &shadow, alpha(Color32::from_rgb(40, 30, 90), 26), alpha(Color32::from_rgb(40, 30, 90), 46));
    let side = outline_points(outline, c + Vec2::new(0.0, r * 0.10), r);
    fill_gradient(painter, &side, mix(base, Color32::BLACK, 0.30), mix(base, Color32::BLACK, 0.45));
    // top face
    let face = outline_points(outline, c, r);
    fill_gradient(painter, &face, mix(base, Color32::WHITE, 0.45), mix(base, Color32::BLACK, 0.25));
    painter.add(Shape::closed_line(face.clone(), Stroke::new(2.0, alpha(Color32::WHITE, 110))));
    // inner well
    let inner = outline_points(outline, c + Vec2::new(0.0, r * 0.03), r * 0.82);
    fill_gradient(painter, &inner, mix(base, Color32::BLACK, 0.15), mix(base, Color32::WHITE, 0.2));
    painter.add(Shape::closed_line(inner, Stroke::new(1.0, alpha(Color32::WHITE, 50))));
    // glossy highlight
    let hl: Vec<Pos2> = outline_points(outline, c + Vec2::new(-r * 0.12, -r * 0.30), r * 0.48);
    fill_gradient(painter, &hl, alpha(Color32::WHITE, 60), alpha(Color32::WHITE, 0));
    // the symbol, with a soft drop shadow
    let gs = r * 0.95;
    let gr = Rect::from_center_size(c + Vec2::new(0.0, r * 0.02), Vec2::splat(gs));
    glyph(painter, g, gr.translate(Vec2::new(0.0, r * 0.06)), alpha(mix(base, Color32::BLACK, 0.5), 120));
    glyph(painter, g, gr, Color32::from_rgb(245, 240, 255));
}

/// Points around a rounded rectangle (clockwise), for gradient fills.
pub fn rounded_points(rect: Rect, radius: f32) -> Vec<Pos2> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right() - r, rect.bottom() - r, 0.0),
        (rect.left() + r, rect.bottom() - r, PI / 2.0),
        (rect.left() + r, rect.top() + r, PI),
        (rect.right() - r, rect.top() + r, PI * 1.5),
    ];
    let mut pts = vec![];
    for (cx, cy, a0) in corners {
        for j in 0..=8 {
            let a = a0 + PI / 2.0 * j as f32 / 8.0;
            pts.push(Pos2::new(cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    pts
}

/// A rounded glassy panel with a top-to-bottom gradient; colours may be see-through.
pub fn panel(painter: &Painter, rect: Rect, radius: f32, top: Color32, bottom: Color32, stroke: Stroke) {
    let pts = rounded_points(rect, radius);
    let lerp = |t: f32| -> Color32 {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgba_premultiplied(l(top.r(), bottom.r()), l(top.g(), bottom.g()), l(top.b(), bottom.b()), l(top.a(), bottom.a()))
    };
    let col = |y: f32| lerp(((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0));
    let mut mesh = egui::Mesh::default();
    let c = rect.center();
    mesh.colored_vertex(c, col(c.y));
    for p in &pts {
        mesh.colored_vertex(*p, col(p.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
}

/// The app's mark: a ring with a gap and the piece that was cleared away, on a raised tile.
pub fn brand_mark(painter: &Painter, rect: Rect) {
    let rad = rect.height() * 0.28;
    painter.rect_filled(rect.translate(Vec2::new(0.0, rect.height() * 0.09)), rad, Color32::from_rgb(61, 34, 166));
    painter.rect_filled(rect, rad, Color32::from_rgb(91, 56, 214));
    let c = rect.center();
    let r = rect.width() * 0.25;
    let pts: Vec<Pos2> = (0..=48).map(|i| c + Vec2::angled((-5.0 + 280.0 * i as f32 / 48.0).to_radians()) * r).collect();
    painter.add(Shape::line(pts, Stroke::new(rect.width() * 0.11, Color32::WHITE)));
    painter.circle_filled(c + Vec2::angled((-45.0f32).to_radians()) * r, rect.width() * 0.06, Color32::from_rgb(111, 242, 196));
}
