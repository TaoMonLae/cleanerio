//! Small drawing helpers shared by every screen.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

pub const TEXT: Color32 = Color32::from_rgb(240, 238, 250);
/// Secondary text: white at reduced strength, so it takes the tint of whichever screen colour
/// is behind it and stays readable on all of them.
pub const MUTED: Color32 = Color32::from_rgba_premultiplied(200, 200, 200, 200);
/// Least important text (counts, hints). Still readable: never use it for instructions.
pub const DIM: Color32 = Color32::from_rgba_premultiplied(158, 158, 158, 158);
pub const GOOD: Color32 = Color32::from_rgb(90, 220, 150);
pub const WARN: Color32 = Color32::from_rgb(255, 190, 80);
pub const BAD: Color32 = Color32::from_rgb(255, 95, 110);

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Fills a rectangle with a smooth four-corner gradient (a fine grid, so no seams show).
pub fn gradient(painter: &egui::Painter, rect: Rect, tl: Color32, tr: Color32, bl: Color32, br: Color32) {
    let n = 12u32;
    let lerp = |a: Color32, b: Color32, t: f32| -> Color32 {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
    };
    let mut mesh = egui::Mesh::default();
    for j in 0..=n {
        let v = j as f32 / n as f32;
        let left = lerp(tl, bl, v);
        let right = lerp(tr, br, v);
        for i in 0..=n {
            let u = i as f32 / n as f32;
            let pos = Pos2::new(rect.left() + rect.width() * u, rect.top() + rect.height() * v);
            mesh.colored_vertex(pos, lerp(left, right, u));
        }
    }
    let w = n + 1;
    for j in 0..n {
        for i in 0..n {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w, a + w + 1);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Soft glowing disc: many faint rings that add up to a smooth fall-off.
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let steps = 40;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let r = radius * (1.7 - 0.9 * t);
        painter.circle_filled(center, r, alpha(color, (1.0 + 3.0 * t) as u8));
    }
}

/// A four-pointed sparkle, used as the app's mark.
pub fn sparkle(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let k = r * 0.28;
    let pts = vec![
        c + Vec2::new(0.0, -r),
        c + Vec2::new(k, -k),
        c + Vec2::new(r, 0.0),
        c + Vec2::new(k, k),
        c + Vec2::new(0.0, r),
        c + Vec2::new(-k, k),
        c + Vec2::new(-r, 0.0),
        c + Vec2::new(-k, -k),
    ];
    painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// The big round action button in the middle of a screen.
pub fn big_button(ui: &mut Ui, label: &str, accent: Color32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    let hover = resp.hovered();
    let t = ui.ctx().animate_bool(resp.id, hover);
    glow(p, c, size * 0.55, accent);
    p.circle_filled(c, size * 0.5, alpha(accent, 50));
    let inner = size * (0.42 + 0.02 * t);
    let top = mix(accent, Color32::WHITE, 0.35);
    // vertical gradient disc using stacked circles
    for i in 0..24 {
        let k = i as f32 / 23.0;
        let col = mix(top, accent, k);
        p.circle_filled(c + Vec2::new(0.0, k * 2.0), inner * (1.0 - k * 0.04), col);
    }
    p.circle_stroke(c, inner, Stroke::new(1.5, alpha(Color32::WHITE, 120)));
    p.text(c, Align2::CENTER_CENTER, label, FontId::proportional(size * 0.15), Color32::WHITE);
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A rounded pill-shaped button.
pub fn pill(ui: &mut Ui, label: &str, fill: Color32) -> Response {
    let b = egui::Button::new(RichText::new(label).color(Color32::WHITE).size(14.0))
        .fill(fill)
        .stroke(Stroke::new(1.0, alpha(Color32::WHITE, 40)))
        .corner_radius(16.0)
        .min_size(Vec2::new(110.0, 32.0));
    ui.add(b)
}

pub fn ghost(ui: &mut Ui, label: &str) -> Response {
    let b = egui::Button::new(RichText::new(label).color(TEXT).size(13.0))
        .fill(alpha(Color32::WHITE, 18))
        .stroke(Stroke::new(1.0, alpha(Color32::WHITE, 30)))
        .corner_radius(14.0)
        .min_size(Vec2::new(0.0, 28.0));
    ui.add(b)
}

/// A translucent card whose contents always stack top to bottom.
pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    card_frame().show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), add);
    });
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(alpha(Color32::WHITE, 14))
        .stroke(Stroke::new(1.0, alpha(Color32::WHITE, 26)))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::same(16))
}

/// A ring gauge, used for CPU, memory and disk.
pub fn ring(ui: &mut Ui, frac: f32, label: &str, value: &str, color: Color32) {
    let size = 120.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size + 26.0), Sense::hover());
    let c = Pos2::new(rect.center().x, rect.top() + size / 2.0);
    let p = ui.painter();
    let r = size * 0.4;
    p.circle_stroke(c, r, Stroke::new(10.0, alpha(Color32::WHITE, 25)));
    let n = 64;
    let frac = frac.clamp(0.0, 1.0);
    let pts: Vec<Pos2> = (0..=((n as f32 * frac) as usize))
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * (i as f32 / n as f32);
            c + Vec2::angled(a) * r
        })
        .collect();
    if pts.len() > 1 {
        p.add(egui::Shape::line(pts, Stroke::new(10.0, color)));
    }
    p.text(c, Align2::CENTER_CENTER, value, FontId::proportional(20.0), TEXT);
    p.text(Pos2::new(c.x, rect.bottom() - 10.0), Align2::CENTER_CENTER, label, FontId::proportional(13.0), MUTED);
}

/// Bar showing how full something is.
pub fn bar(ui: &mut Ui, frac: f32, width: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, alpha(Color32::WHITE, 25));
    let mut f = rect;
    f.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 4.0, color);
}

pub fn icon_badge(ui: &mut Ui, icon: &str, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    let r = (size * 0.28) as u8;
    p.rect_filled(rect, r, mix(color, Color32::BLACK, 0.15));
    let mut top = rect.shrink(1.0);
    top.set_height(rect.height() * 0.62);
    p.rect_filled(top, egui::CornerRadius { nw: r, ne: r, sw: r / 2, se: r / 2 }, mix(color, Color32::WHITE, 0.18));
    p.text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size * 0.55), Color32::WHITE);
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(30.0).strong().color(TEXT));
}

pub fn subtitle(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(15.0).color(MUTED));
}

/// Colour from hue, saturation and brightness (all 0..1), in screen colours.
pub fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let f = |u: f32| ((u + m) * 255.0).round() as u8;
    Color32::from_rgb(f(r), f(g), f(b))
}

/// The large round button that sits half below the bottom edge of the window.
pub fn big_button_at(ui: &mut Ui, center: Pos2, size: f32, label: &str, accent: Color32) -> bool {
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    let resp = ui.interact(rect, ui.scope_id().with(("round_button", label)), Sense::click());
    let p = ui.painter().with_clip_rect(Rect::EVERYTHING);
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    let r = size * 0.5;
    glow(&p, center, size * 0.75, mix(accent, Color32::WHITE, 0.25));
    let top = mix(accent, Color32::WHITE, 0.30 + 0.1 * t);
    let bottom = mix(accent, Color32::BLACK, 0.15);
    for i in 0..28 {
        let k = i as f32 / 27.0;
        p.circle_filled(center + Vec2::new(0.0, k * r * 0.12), r * (1.0 - k * 0.12), mix(top, bottom, k));
    }
    p.circle_stroke(center, r, Stroke::new(3.0, alpha(Color32::WHITE, 150 + (60.0 * t) as u8)));
    p.text(center - Vec2::new(0.0, r * 0.5), Align2::CENTER_CENTER, label, FontId::proportional(24.0), Color32::WHITE);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// The round button at the bottom middle of a screen.
pub fn round_button(ui: &mut Ui, label: &str, accent: Color32) -> bool {
    let r = ui.max_rect();
    big_button_at(ui, Pos2::new(r.center().x, r.bottom() + 22.0), 116.0, label, accent)
}

/// A row of pill-shaped tabs. Returns true when the choice changed.
pub fn tabs(ui: &mut Ui, labels: &[String], sel: &mut usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, l) in labels.iter().enumerate() {
            let on = *sel == i;
            let b = egui::Button::new(RichText::new(l).size(14.0).color(if on { Color32::WHITE } else { TEXT }))
                .fill(if on { alpha(Color32::WHITE, 46) } else { alpha(Color32::WHITE, 12) })
                .stroke(Stroke::new(1.0, alpha(Color32::WHITE, if on { 70 } else { 24 })))
                .corner_radius(15.0)
                .min_size(Vec2::new(0.0, 30.0));
            if ui.add(b).clicked() && !on {
                *sel = i;
                changed = true;
            }
        }
    });
    changed
}
