//! Small drawing helpers shared by every screen.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

// The look: a flat, light interface. Colour and depth are saved for the one 3D object on
// each screen and for the primary action.
pub const GROUND: Color32 = Color32::from_rgb(243, 244, 247);
pub const LINE: Color32 = Color32::from_rgb(225, 228, 236);
pub const EDGE: Color32 = Color32::from_rgb(201, 206, 219);
pub const TEXT: Color32 = Color32::from_rgb(21, 23, 30);
pub const BODY: Color32 = Color32::from_rgb(58, 63, 78);
/// Secondary text.
pub const MUTED: Color32 = Color32::from_rgb(86, 91, 107);
/// Least important text (counts, hints). Still readable: never use it for instructions.
pub const DIM: Color32 = Color32::from_rgb(106, 112, 128);
pub const GOOD: Color32 = Color32::from_rgb(31, 133, 86);
pub const WARN: Color32 = Color32::from_rgb(150, 92, 0);
pub const BAD: Color32 = Color32::from_rgb(196, 43, 62);
pub const ACCENT: Color32 = Color32::from_rgb(91, 56, 214);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(61, 34, 166);
pub const TINT: Color32 = Color32::from_rgb(236, 232, 255);

/// The text colour at reduced strength, for hover washes and tracks.
pub fn ink(a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(21, 23, 30, a)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Fills a rectangle with a smooth four-corner gradient (a fine grid, so no seams show).
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let steps = 40;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let r = radius * (1.7 - 0.9 * t);
        painter.circle_filled(center, r, alpha(color, (1.0 + 3.0 * t) as u8));
    }
}

/// A four-pointed sparkle, used as the app's mark.
#[allow(dead_code)]
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
#[allow(dead_code)]
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

/// The filled button for a screen's main action (always the same violet).
pub fn pill(ui: &mut Ui, label: &str, _accent: Color32) -> Response {
    let b = egui::Button::new(RichText::new(label).color(Color32::WHITE).size(14.5))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(10.0)
        .min_size(Vec2::new(110.0, 34.0));
    ui.add(b)
}

/// The outlined button for everything else.
pub fn ghost(ui: &mut Ui, label: &str) -> Response {
    let b = egui::Button::new(RichText::new(label).color(TEXT).size(13.5))
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(10.0)
        .min_size(Vec2::new(0.0, 30.0));
    ui.add(b)
}

/// A flat white panel whose contents always stack top to bottom.
pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    card_frame().show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), add);
    });
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, LINE))
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
    p.circle_stroke(c, r, Stroke::new(10.0, Color32::from_rgb(230, 232, 239)));
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
    p.rect_filled(rect, 4.0, Color32::from_rgb(230, 232, 239));
    let mut f = rect;
    f.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 4.0, color);
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(30.0).strong().color(TEXT));
}

#[allow(dead_code)]
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

/// The primary button, drawn at a fixed place. The one raised control on a screen.
pub fn primary_at(ui: &mut Ui, rect: Rect, label: &str) -> bool {
    let resp = ui.interact(rect, ui.scope_id().with(("primary", label)), Sense::click());
    let p = ui.painter().with_clip_rect(Rect::EVERYTHING);
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    p.add(egui::epaint::Shadow { offset: [0, 5], blur: 14, spread: 0, color: alpha(ACCENT, 70) }.as_shape(rect, 12.0));
    p.rect_filled(rect, 12.0, mix(ACCENT, ACCENT_DARK, 0.4 * t));
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(16.0), Color32::WHITE);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// How wide a primary button needs to be for its label.
pub fn primary_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 9.5 + 68.0).max(170.0)
}

/// A screen's main action, centred at the bottom of the screen.
pub fn round_button(ui: &mut Ui, label: &str, _accent: Color32) -> bool {
    let r = ui.max_rect();
    let rect = Rect::from_center_size(Pos2::new(r.center().x, r.bottom() - 16.0), Vec2::new(primary_width(label), 44.0));
    primary_at(ui, rect, label)
}

/// A row of pill-shaped tabs. Returns true when the choice changed.
pub fn tabs(ui: &mut Ui, labels: &[String], sel: &mut usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, l) in labels.iter().enumerate() {
            let on = *sel == i;
            let b = egui::Button::new(RichText::new(l).size(14.0).color(if on { Color32::WHITE } else { TEXT }))
                .fill(if on { TEXT } else { Color32::WHITE })
                .stroke(Stroke::new(1.0, if on { TEXT } else { EDGE }))
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
