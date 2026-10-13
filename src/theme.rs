use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, Style, Visuals};

use crate::settings::{Settings, ThemeKind};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_alt: Color32,
    pub warn: Color32,
    pub ok: Color32,
    pub track: Color32,
}

impl Palette {
    pub fn from_settings(s: &Settings) -> Self {
        let accent = Color32::from_rgb(s.accent[0], s.accent[1], s.accent[2]);
        let accent_alt = Color32::from_rgb(s.accent_alt[0], s.accent_alt[1], s.accent_alt[2]);
        match s.theme {
            ThemeKind::Dark => Self {
                bg: Color32::from_rgb(6, 8, 14),
                panel: Color32::from_rgb(s.surface[0], s.surface[1], s.surface[2]),
                card: Color32::from_rgba_unmultiplied(18, 24, 38, 230),
                text: Color32::from_rgb(230, 240, 255),
                muted: Color32::from_rgb(140, 156, 184),
                accent,
                accent_alt,
                warn: Color32::from_rgb(255, 92, 120),
                ok: Color32::from_rgb(80, 255, 170),
                track: Color32::from_rgb(28, 36, 54),
            },
            ThemeKind::Light => Self {
                bg: Color32::from_rgb(236, 240, 248),
                panel: Color32::from_rgb(248.max(s.surface[0]), 250, 255),
                card: Color32::from_rgba_unmultiplied(255, 255, 255, 235),
                text: Color32::from_rgb(18, 24, 40),
                muted: Color32::from_rgb(90, 104, 128),
                accent,
                accent_alt,
                warn: Color32::from_rgb(200, 40, 70),
                ok: Color32::from_rgb(16, 150, 100),
                track: Color32::from_rgb(220, 226, 238),
            },
        }
    }

    pub fn apply(self, ctx: &egui::Context, text_scale: f32) {
        let mut style = Style {
            visuals: match self.bg.r() < 80 {
                true => Visuals::dark(),
                false => Visuals::light(),
            },
            ..Style::default()
        };
        style.visuals.override_text_color = Some(self.text);
        style.visuals.panel_fill = self.bg;
        style.visuals.window_fill = self.panel;
        style.visuals.extreme_bg_color = self.track;
        style.visuals.widgets.inactive.bg_fill = self.card;
        style.visuals.widgets.hovered.bg_fill = self.card;
        style.visuals.widgets.active.bg_fill = self.card;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text);
        style.visuals.selection.bg_fill = self.accent.linear_multiply(0.35);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(10);
        style.visuals.widgets.hovered.corner_radius = CornerRadius::same(10);
        style.visuals.widgets.active.corner_radius = CornerRadius::same(10);
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::proportional(14.0 * text_scale),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::proportional(22.0 * text_scale),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            FontId::proportional(11.0 * text_scale),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::proportional(13.0 * text_scale),
        );
        ctx.set_style(style);
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
}

pub fn heat(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        lerp_color(
            Color32::from_rgb(40, 220, 180),
            Color32::from_rgb(255, 200, 60),
            t * 2.0,
        )
    } else {
        lerp_color(
            Color32::from_rgb(255, 200, 60),
            Color32::from_rgb(255, 70, 110),
            (t - 0.5) * 2.0,
        )
    }
}
