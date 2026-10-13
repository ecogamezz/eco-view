//! The component library every page is built from.
//!
//! Everything takes a [`Palette`] so components follow the theme, and anything
//! that moves (numbers, toggles, segmented controls) animates unless the user
//! asked for reduced motion.

use std::f32::consts::PI;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Id, Layout, Mesh, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{self, Palette, lerp_color};

// ----------------------------------------------------------------------
// Motion
// ----------------------------------------------------------------------

fn motion_id() -> Id {
    Id::new("eco-reduce-motion")
}

/// Records the user's reduced-motion preference for this frame.
pub fn set_reduce_motion(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(motion_id(), on));
}

pub fn reduce_motion(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(motion_id())).unwrap_or(false)
}

/// A value that glides toward `target`.
pub fn tween(ctx: &egui::Context, id: Id, target: f32, secs: f32) -> f32 {
    if reduce_motion(ctx) || !target.is_finite() {
        return target;
    }
    ctx.animate_value_with_time(id, target, secs)
}

// ----------------------------------------------------------------------
// Cards
// ----------------------------------------------------------------------

#[derive(Clone)]
pub struct Badge {
    pub text: String,
    pub color: Color32,
}

impl Badge {
    pub fn new(text: impl Into<String>, color: Color32) -> Self {
        Self { text: text.into(), color }
    }
}

#[derive(Clone)]
pub struct Header {
    pub title: String,
    pub icon: Option<Icon>,
    pub tint: Color32,
    pub badge: Option<Badge>,
}

impl Header {
    pub fn new(title: impl Into<String>, icon: Icon, tint: Color32) -> Self {
        Self { title: title.into(), icon: Some(icon), tint, badge: None }
    }

    pub fn badge(mut self, b: Option<Badge>) -> Self {
        self.badge = b;
        self
    }
}

pub const CARD_RADIUS: u8 = 14;

/// Paints a card's shadow, surface and hairline border.
pub fn card_surface(painter: &egui::Painter, pal: &Palette, rect: Rect, hovered: bool) {
    let r = CornerRadius::same(CARD_RADIUS);
    painter.add(pal.card_shadow().as_shape(rect, r));
    let fill = if hovered { lerp_color(pal.surface, pal.surface_hi, 0.55) } else { pal.surface };
    painter.rect_filled(rect, r, fill);
    // A faint top sheen gives the surface some depth.
    let sheen = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + rect.height().min(60.0)));
    let top = if pal.dark { Color32::from_white_alpha(7) } else { Color32::from_white_alpha(70) };
    gradient_rect(painter, sheen, top, Color32::TRANSPARENT, r, true);
    let border = if hovered { pal.border_hi } else { pal.border };
    painter.rect_stroke(rect, r, Stroke::new(1.0_f32, border), StrokeKind::Inside);
}

/// Vertical gradient in a rect; only the top corners are rounded when `top_only`.
pub fn gradient_rect(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32, r: CornerRadius, top_only: bool) {
    let _ = (r, top_only);
    let mut mesh = Mesh::default();
    let inset = CARD_RADIUS as f32 * 0.6;
    // Keep the gradient inside the rounded corners by insetting its sides near the top.
    let a = pos2(rect.left() + inset, rect.top());
    let b = pos2(rect.right() - inset, rect.top());
    let c = pos2(rect.right(), rect.top() + inset);
    let d = pos2(rect.right(), rect.bottom());
    let e = pos2(rect.left(), rect.bottom());
    let f = pos2(rect.left(), rect.top() + inset);
    let h = rect.height().max(1.0);
    let col = |p: Pos2| lerp_color(top, bottom, (p.y - rect.top()) / h);
    for p in [a, b, c, d, e, f] {
        mesh.colored_vertex(p, col(p));
    }
    for (i, j, k) in [(0, 1, 2), (0, 2, 3), (0, 3, 4), (0, 4, 5)] {
        mesh.add_triangle(i, j, k);
    }
    painter.add(Shape::mesh(mesh));
}

/// Draws a card header row: tinted icon tile, title and an optional badge.
pub fn card_header(ui: &mut Ui, pal: &Palette, h: &Header) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if let Some(icon) = h.icon {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, pal.tint(h.tint, if pal.dark { 0.16 } else { 0.18 }));
            icons::paint_in(ui.painter(), r.shrink(5.0), icon, h.tint);
        }
        ui.label(RichText::new(&h.title).font(theme::semibold(13.5)).color(pal.text));
        if let Some(b) = &h.badge {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(ui, pal, &b.text, b.color);
            });
        }
    });
    ui.add_space(4.0);
}

/// A card that fills `rect` exactly (used by the dashboard grid).
pub fn card_at(ui: &mut Ui, pal: &Palette, rect: Rect, header: &Header, add: impl FnOnce(&mut Ui)) -> Response {
    let resp = ui.interact(rect, ui.id().with(("card", &header.title)), Sense::hover());
    card_surface(ui.painter(), pal, rect, resp.hovered());
    let inner = rect.shrink2(vec2(14.0, 12.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
    child.set_clip_rect(rect.shrink(1.0).intersect(ui.clip_rect()));
    card_header(&mut child, pal, header);
    add(&mut child);
    resp
}

/// A card in normal flow layout, as wide as the available space.
pub fn card(ui: &mut Ui, pal: &Palette, header: Option<&Header>, add: impl FnOnce(&mut Ui)) -> Response {
    let where_ = ui.painter().add(Shape::Noop);
    let shadow = ui.painter().add(Shape::Noop);
    let resp = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(h) = header {
                card_header(ui, pal, h);
            }
            add(ui);
        })
        .response;
    let rect = resp.rect;
    let hovered = ui.rect_contains_pointer(rect);
    // Paint the surface underneath the content we already laid out.
    let r = CornerRadius::same(CARD_RADIUS);
    ui.painter().set(shadow, pal.card_shadow().as_shape(rect, r));
    let fill = if hovered { lerp_color(pal.surface, pal.surface_hi, 0.4) } else { pal.surface };
    ui.painter().set(
        where_,
        Shape::Rect(egui::epaint::RectShape::new(
            rect,
            r,
            fill,
            Stroke::new(1.0_f32, if hovered { pal.border_hi } else { pal.border }),
            StrokeKind::Inside,
        )),
    );
    resp
}

/// A settings-style group: a rounded panel whose rows are separated by hairlines.
pub fn group(ui: &mut Ui, pal: &Palette, title: Option<&str>, add: impl FnOnce(&mut Ui)) {
    if let Some(t) = title {
        ui.add_space(6.0);
        ui.label(RichText::new(t.to_uppercase()).font(theme::semibold(11.0)).color(pal.muted));
        ui.add_space(2.0);
    }
    let bg = ui.painter().add(Shape::Noop);
    let resp = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        })
        .response;
    ui.painter().set(
        bg,
        Shape::Rect(egui::epaint::RectShape::new(
            resp.rect,
            CornerRadius::same(12),
            pal.surface,
            Stroke::new(1.0_f32, pal.border),
            StrokeKind::Inside,
        )),
    );
    ui.add_space(10.0);
}

/// Hairline between rows of a [`group`].
pub fn separator(ui: &mut Ui, pal: &Palette) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0_f32, pal.border));
}

/// A label on the left, a control on the right.
pub fn row<R>(ui: &mut Ui, pal: &Palette, title: &str, desc: Option<&str>, control: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(4.0);
    let r = ui
        .horizontal(|ui| {
            ui.set_min_height(30.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new(title).font(theme::medium(13.0)).color(pal.text));
                if let Some(d) = desc {
                    ui.label(RichText::new(d).size(11.5).color(pal.muted));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), control).inner
        })
        .inner;
    ui.add_space(4.0);
    r
}

// ----------------------------------------------------------------------
// Text
// ----------------------------------------------------------------------

/// A large animated number with a unit.
pub fn big_number(ui: &mut Ui, pal: &Palette, id: Id, value: f32, decimals: usize, unit: &str, size: f32, color: Color32) {
    let v = tween(ui.ctx(), id, value, 0.45);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.label(RichText::new(format!("{v:.decimals$}")).font(theme::semibold(size)).color(color));
        if !unit.is_empty() {
            ui.label(RichText::new(unit).font(theme::medium(size * 0.48)).color(pal.text2));
        }
    });
}

/// Small caption over a value.
pub fn stat(ui: &mut Ui, pal: &Palette, label: &str, value: impl Into<String>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(RichText::new(label.to_uppercase()).font(theme::semibold(9.5)).color(pal.muted));
        ui.label(RichText::new(value.into()).font(theme::medium(13.5)).color(pal.text));
    });
}

pub fn caption(ui: &mut Ui, pal: &Palette, text: impl Into<String>) -> Response {
    ui.label(RichText::new(text.into()).size(11.5).color(pal.muted))
}

/// Key / value line with the value right-aligned.
pub fn kv(ui: &mut Ui, pal: &Palette, key: &str, value: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).color(pal.text2));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value.into()).font(theme::medium(13.0)).color(pal.text));
        });
    });
}

pub fn section_title(ui: &mut Ui, pal: &Palette, title: &str, subtitle: Option<&str>) {
    ui.label(RichText::new(title).font(theme::semibold(16.0)).color(pal.text));
    if let Some(s) = subtitle {
        ui.label(RichText::new(s).size(12.0).color(pal.muted));
    }
    ui.add_space(6.0);
}

// ----------------------------------------------------------------------
// Pills, buttons, fields
// ----------------------------------------------------------------------

/// A small rounded label on a tinted background.
pub fn pill(ui: &mut Ui, pal: &Palette, text: &str, color: Color32) -> Response {
    let font = theme::semibold(10.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let size = galley.size() + vec2(14.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    // A radius of exactly half the height makes the end caps overlap and
    // leaves a visible seam on translucent fills, so stay just under it.
    ui.painter().rect_filled(rect, rect.height() * 0.5 - 1.0, pal.tint(color, if pal.dark { 0.16 } else { 0.18 }));
    ui.painter().galley(rect.min + vec2(7.0, 3.0), galley, color);
    resp
}

/// A pill with an icon and a live value, used in the header.
pub fn status_pill(ui: &mut Ui, pal: &Palette, icon: Icon, text: &str, color: Color32, frac: Option<f32>) -> Response {
    let font = theme::medium(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, pal.text);
    let w = galley.size().x + 36.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 14.0, pal.surface);
    p.rect_stroke(rect, 14.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
    if let Some(f) = frac {
        // Fill proportional to the value, very faintly.
        let mut fill = rect.shrink(1.0);
        fill.set_width(fill.width() * f.clamp(0.0, 1.0));
        p.rect_filled(fill, 12.0, pal.tint(color, 0.12));
    }
    icons::paint_in(p, Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), Vec2::splat(15.0)), icon, color);
    p.galley(pos2(rect.left() + 28.0, rect.center().y - galley.size().y * 0.5), galley, pal.text);
    resp
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub fn button(ui: &mut Ui, pal: &Palette, kind: ButtonKind, icon: Option<Icon>, text: &str) -> Response {
    let font = theme::medium(13.0);
    let (fg, bg, border) = match kind {
        ButtonKind::Primary => (pal.on_accent, pal.accent, Color32::TRANSPARENT),
        ButtonKind::Secondary => (pal.text, pal.surface_hi, pal.border_hi),
        ButtonKind::Ghost => (pal.text2, Color32::TRANSPARENT, Color32::TRANSPARENT),
        ButtonKind::Danger => (Color32::WHITE, pal.crit, Color32::TRANSPARENT),
    };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let icon_w = if icon.is_some() { 20.0 } else { 0.0 };
    let pad = if text.is_empty() { 8.0 } else { 12.0 };
    let size = vec2(galley.size().x + icon_w + pad * 2.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let p = ui.painter();
    let mut fill = match kind {
        ButtonKind::Primary | ButtonKind::Danger => lerp_color(bg, Color32::WHITE, hover * 0.12),
        ButtonKind::Secondary => lerp_color(bg, pal.tint(pal.accent, 0.4), hover * 0.25),
        ButtonKind::Ghost => pal.tint(pal.text, 0.07 * hover),
    };
    if resp.is_pointer_button_down_on() {
        fill = lerp_color(fill, Color32::BLACK, 0.1);
    }
    p.rect_filled(rect, 9.0, fill);
    if border != Color32::TRANSPARENT {
        p.rect_stroke(rect, 9.0, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    }
    let mut x = rect.left() + pad;
    if let Some(i) = icon {
        icons::paint_in(p, Rect::from_center_size(pos2(x + 8.0, rect.center().y), Vec2::splat(15.0)), i, fg);
        x += icon_w;
    }
    p.galley(pos2(x, rect.center().y - galley.size().y * 0.5), galley, fg);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Square icon-only button with a hover background.
pub fn icon_button(ui: &mut Ui, pal: &Palette, icon: Icon, tip: &str, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || active, 0.12);
    let p = ui.painter();
    if hover > 0.0 {
        p.rect_filled(rect, 8.0, pal.tint(if active { pal.accent } else { pal.text }, if active { 0.16 } else { 0.08 } * hover.max(0.3)));
    }
    let c = if active { pal.accent } else { lerp_color(pal.text2, pal.text, hover) };
    icons::paint_in(p, rect.shrink(7.0), icon, c);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

/// A rounded search field with a magnifier.
pub fn search_field(ui: &mut Ui, pal: &Palette, text: &mut String, hint: &str, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 9.0, pal.inset);
    p.rect_stroke(rect, 9.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
    icons::paint_in(p, Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), Vec2::splat(14.0)), Icon::Search, pal.muted);
    let field = Rect::from_min_max(pos2(rect.left() + 28.0, rect.top() + 3.0), pos2(rect.right() - 8.0, rect.bottom() - 3.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field).layout(Layout::left_to_right(Align::Center)));
    child.add(
        egui::TextEdit::singleline(text)
            .hint_text(RichText::new(hint).color(pal.muted))
            .frame(false)
            .desired_width(field.width())
            .margin(egui::Margin::symmetric(0, 4)),
    )
}

// ----------------------------------------------------------------------
// Controls
// ----------------------------------------------------------------------

/// iOS-style switch.
pub fn toggle(ui: &mut Ui, pal: &Palette, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.16);
    let p = ui.painter();
    let track = lerp_color(pal.inset, pal.accent, t);
    p.rect_filled(rect, 11.0, track);
    p.rect_stroke(rect, 11.0, Stroke::new(1.0_f32, lerp_color(pal.border_hi, Color32::TRANSPARENT, t)), StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
    let c = pos2(x, rect.center().y);
    p.circle_filled(c + vec2(0.0, 1.0), 8.5, Color32::from_black_alpha(40));
    p.circle_filled(c, 8.5, Color32::WHITE);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A segmented control. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, pal: &Palette, current: &mut T, items: &[(T, &str)]) -> bool {
    let font = theme::medium(12.0);
    let widths: Vec<f32> = items
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap(l.to_string(), font.clone(), pal.text).size().x + 22.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, 9.0, pal.inset);
    p.rect_stroke(rect, 9.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
    let sel = items.iter().position(|(v, _)| v == current).unwrap_or(0);
    let mut lefts = Vec::with_capacity(items.len());
    let mut x = rect.left() + 3.0;
    for w in &widths {
        lefts.push(x);
        x += w;
    }
    let tx = tween(ui.ctx(), resp.id.with("x"), lefts[sel], 0.18);
    let tw = tween(ui.ctx(), resp.id.with("w"), widths[sel], 0.18);
    let thumb = Rect::from_min_size(pos2(tx, rect.top() + 3.0), vec2(tw, rect.height() - 6.0));
    p.add(egui::Shadow { offset: [0, 1], blur: 4, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(thumb, 7));
    p.rect_filled(thumb, 7.0, if pal.dark { pal.surface_hi } else { Color32::WHITE });
    let mut changed = false;
    for (i, (v, label)) in items.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], rect.top()), vec2(widths[i], rect.height()));
        let ir = ui.interact(r, resp.id.with(i), Sense::click());
        let selected = i == sel;
        let color = if selected { pal.text } else if ir.hovered() { pal.text2 } else { pal.muted };
        p.text(r.center(), Align2::CENTER_CENTER, *label, font.clone(), color);
        if ir.clicked() && !selected {
            *current = *v;
            changed = true;
        }
        if ir.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    changed
}

/// A labelled slider with the value shown on the right.
pub fn slider<N: egui::emath::Numeric>(ui: &mut Ui, value: &mut N, range: std::ops::RangeInclusive<N>, suffix: &str) -> Response {
    ui.add(egui::Slider::new(value, range).suffix(suffix).trailing_fill(true).max_decimals(2))
}

/// A row of colour swatches; returns the picked colour.
pub fn swatches(ui: &mut Ui, pal: &Palette, current: Color32, colors: &[Color32]) -> Option<Color32> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for &c in colors {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let p = ui.painter();
            p.circle_filled(rect.center(), 10.0, c);
            if c == current {
                p.circle_stroke(rect.center(), 10.5, Stroke::new(2.0_f32, pal.text));
            } else if resp.hovered() {
                p.circle_stroke(rect.center(), 10.5, Stroke::new(1.5_f32, pal.border_hi));
            }
            if resp.clicked() {
                picked = Some(c);
            }
        }
    });
    picked
}

// ----------------------------------------------------------------------
// Gauges and bars
// ----------------------------------------------------------------------

/// Adds an anti-aliased arc band to `mesh`. Angles in radians, clockwise from +x.
pub fn arc_band(mesh: &mut Mesh, c: Pos2, r: f32, thick: f32, a0: f32, a1: f32, feather: f32, color: impl Fn(f32) -> Color32) {
    if (a1 - a0).abs() < 1e-4 {
        return;
    }
    let n = (((a1 - a0).abs() * r / 3.0) as usize).clamp(8, 160);
    let base = mesh.vertices.len() as u32;
    let radii = [r - thick * 0.5 - feather * 0.5, r - thick * 0.5 + feather * 0.5, r + thick * 0.5 - feather * 0.5, r + thick * 0.5 + feather * 0.5];
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = a0 + (a1 - a0) * t;
        let (s, co) = a.sin_cos();
        let col = color(t);
        for (j, rr) in radii.iter().enumerate() {
            let cc = if j == 0 || j == 3 { Color32::TRANSPARENT } else { col };
            mesh.colored_vertex(c + vec2(co, s) * *rr, cc);
        }
    }
    for i in 0..n as u32 {
        let a = base + i * 4;
        let b = a + 4;
        for k in 0..3 {
            mesh.add_triangle(a + k, b + k, a + k + 1);
            mesh.add_triangle(a + k + 1, b + k, b + k + 1);
        }
    }
}

/// A 270° ring gauge.
pub fn ring(painter: &egui::Painter, pal: &Palette, c: Pos2, r: f32, thick: f32, frac: f32, color: Color32) {
    let feather = 1.0 / painter.ctx().pixels_per_point();
    let a0 = PI * 0.75;
    let sweep = PI * 1.5;
    let mut mesh = Mesh::default();
    arc_band(&mut mesh, c, r, thick, a0, a0 + sweep, feather, |_| pal.inset);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.001 {
        let start = lerp_color(color, pal.accent_alt, 0.35);
        arc_band(&mut mesh, c, r, thick, a0, a0 + sweep * f, feather, |t| lerp_color(start, color, t));
    }
    painter.add(Shape::mesh(mesh));
    if f > 0.001 {
        // Rounded caps.
        let cap = |a: f32, col: Color32| {
            let (s, co) = a.sin_cos();
            painter.circle_filled(c + vec2(co, s) * r, thick * 0.5, col);
        };
        cap(a0, lerp_color(color, pal.accent_alt, 0.35));
        cap(a0 + sweep * f, color);
        let (s, co) = (a0 + sweep * f).sin_cos();
        painter.circle_filled(c + vec2(co, s) * r, thick * 0.9, pal.tint(color, 0.18));
    }
}

/// A ring gauge widget with a value in the middle and a caption under it.
pub fn ring_gauge(ui: &mut Ui, pal: &Palette, id: Id, size: f32, frac: f32, color: Color32, value: &str, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size * 0.92), Sense::hover());
    let f = tween(ui.ctx(), id, frac.clamp(0.0, 1.0), 0.5);
    let c = pos2(rect.center().x, rect.top() + size * 0.5);
    let thick = (size * 0.085).max(5.0);
    ring(ui.painter(), pal, c, size * 0.5 - thick, thick, f, color);
    ui.painter().text(c - vec2(0.0, size * 0.03), Align2::CENTER_CENTER, value, theme::semibold(size * 0.2), pal.text);
    if !label.is_empty() {
        ui.painter().text(c + vec2(0.0, size * 0.17), Align2::CENTER_CENTER, label, theme::semibold(size * 0.085), pal.muted);
    }
    resp
}

/// A horizontal progress bar.
pub fn bar(ui: &mut Ui, pal: &Palette, id: Id, frac: f32, color: Color32, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let f = tween(ui.ctx(), id, frac.clamp(0.0, 1.0), 0.45);
    paint_bar(ui.painter(), pal, rect, f, color);
    resp
}

pub fn paint_bar(p: &egui::Painter, pal: &Palette, rect: Rect, frac: f32, color: Color32) {
    let r = rect.height() * 0.5;
    p.rect_filled(rect, r, pal.inset);
    if frac > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * frac).max(rect.height()));
        let mut mesh = Mesh::default();
        // Horizontal gradient body, then rounded ends on top.
        let a = lerp_color(color, pal.accent_alt, 0.3);
        let (x0, x1) = (fill.left() + r, fill.right() - r);
        if x1 > x0 {
            let w = (x1 - x0).max(1.0);
            let col = |x: f32| lerp_color(a, color, (x - fill.left()) / fill.width().max(1.0));
            let _ = w;
            for (x, y) in [(x0, fill.top()), (x1, fill.top()), (x1, fill.bottom()), (x0, fill.bottom())] {
                mesh.colored_vertex(pos2(x, y), col(x));
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(Shape::mesh(mesh));
        }
        p.circle_filled(pos2(fill.left() + r, fill.center().y), r, a);
        p.circle_filled(pos2(fill.right() - r, fill.center().y), r, color);
    }
}

/// A bar split into coloured segments (fractions should add up to <= 1).
pub fn stacked_bar(ui: &mut Ui, pal: &Palette, segments: &[(f32, Color32)], height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    let r = height * 0.5;
    p.rect_filled(rect, r, pal.inset);
    let mut x = rect.left();
    let n = segments.len();
    for (i, (f, c)) in segments.iter().enumerate() {
        let w = rect.width() * f.clamp(0.0, 1.0);
        if w <= 0.5 {
            continue;
        }
        let seg = Rect::from_min_size(pos2(x, rect.top()), vec2(w, height));
        let cr = CornerRadius {
            nw: if i == 0 { r as u8 } else { 0 },
            sw: if i == 0 { r as u8 } else { 0 },
            ne: if i + 1 == n { r as u8 } else { 0 },
            se: if i + 1 == n { r as u8 } else { 0 },
        };
        p.rect_filled(seg, cr, *c);
        x += w;
    }
    resp
}

/// Vertical bars, one per core, coloured by load.
pub fn core_bars(ui: &mut Ui, pal: &Palette, cores: &[f32], height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if cores.is_empty() {
        return resp;
    }
    let n = cores.len() as f32;
    let gap = (rect.width() / n * 0.22).clamp(2.0, 6.0);
    let w = ((rect.width() - gap * (n - 1.0)) / n).max(2.0);
    let p = ui.painter();
    let hover = resp.hover_pos();
    let mut tip = None;
    for (i, v) in cores.iter().enumerate() {
        let x = rect.left() + i as f32 * (w + gap);
        let col = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        let id = resp.id.with(i);
        let f = tween(ui.ctx(), id, (*v / 100.0).clamp(0.0, 1.0), 0.4);
        let rr = (w * 0.35).min(4.0);
        p.rect_filled(col, rr, pal.tint(pal.text, 0.05));
        let h = (col.height() * f).max(2.0);
        let fill = Rect::from_min_max(pos2(col.left(), col.bottom() - h), col.max);
        p.rect_filled(fill, rr, theme::heat(f));
        if hover.is_some_and(|h| col.expand2(vec2(gap * 0.5, 0.0)).contains(h)) {
            p.rect_stroke(col, rr, Stroke::new(1.0_f32, pal.text2), StrokeKind::Outside);
            tip = Some(format!("Core {}: {v:.0}%", i + 1));
        }
    }
    match tip {
        Some(t) => resp.on_hover_text(t),
        None => resp,
    }
}

/// A big friendly message for when there is nothing to show.
pub fn empty_state(ui: &mut Ui, pal: &Palette, icon: Icon, title: &str, body: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 22.0, pal.tint(pal.accent, 0.12));
        icons::paint_in(ui.painter(), r.shrink(11.0), icon, pal.accent);
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(theme::semibold(14.0)).color(pal.text));
        ui.label(RichText::new(body).size(12.0).color(pal.muted));
        ui.add_space(18.0);
    });
}

/// Inline percentage bar used inside table cells.
pub fn cell_bar(ui: &mut Ui, pal: &Palette, frac: f32, color: Color32, text: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
    let p = ui.painter();
    let track = Rect::from_min_size(pos2(rect.left(), rect.center().y + 4.0), vec2(rect.width(), 3.0));
    p.rect_filled(track, 1.5, pal.inset);
    let mut f = track;
    f.set_width(track.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 1.5, color);
    p.text(pos2(rect.right(), rect.center().y - 3.0), Align2::RIGHT_CENTER, text, theme::mono(11.5), pal.text);
}
