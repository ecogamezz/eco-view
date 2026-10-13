//! Design tokens: colour palettes, typography and the egui style built from
//! them.
//!
//! Every surface in the app takes its colours from a [`Palette`], which is
//! derived from the selected [`ThemeKind`] preset plus an optional custom
//! accent. Text uses macOS's own Avenir Next and SF Mono when they are present
//! and falls back to egui's bundled fonts otherwise.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Visuals, vec2};

use crate::settings::{Settings, ThemeKind};

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    /// Window background, top and bottom of its vertical gradient.
    pub bg: Color32,
    pub bg_top: Color32,
    pub sidebar: Color32,
    /// Cards.
    pub surface: Color32,
    /// Hovered / raised surfaces.
    pub surface_hi: Color32,
    /// Wells, tracks, inputs.
    pub inset: Color32,
    pub border: Color32,
    pub border_hi: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_alt: Color32,
    /// Text drawn on top of the accent colour.
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub crit: Color32,
    pub info: Color32,
    pub shadow: Color32,
}

fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 6] = [
        ThemeKind::Midnight,
        ThemeKind::Graphite,
        ThemeKind::Aurora,
        ThemeKind::Ember,
        ThemeKind::Daylight,
        ThemeKind::Paper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Midnight => "Midnight",
            ThemeKind::Graphite => "Graphite",
            ThemeKind::Aurora => "Aurora",
            ThemeKind::Ember => "Ember",
            ThemeKind::Daylight => "Daylight",
            ThemeKind::Paper => "Paper",
        }
    }

    pub fn is_dark(self) -> bool {
        !matches!(self, ThemeKind::Daylight | ThemeKind::Paper)
    }

    /// The preset's own palette, before any custom accent.
    pub fn palette(self) -> Palette {
        let dark_sem = (hex(0x34D399), hex(0xFBBF24), hex(0xF87171), hex(0x60A5FA));
        let light_sem = (hex(0x16A34A), hex(0xD97706), hex(0xDC2626), hex(0x2563EB));
        let dark_lines = (Color32::from_white_alpha(14), Color32::from_white_alpha(30));
        let light_lines = (Color32::from_black_alpha(16), Color32::from_black_alpha(34));
        let (bg, bg_top, sidebar, surface, surface_hi, inset, text, text2, muted, accent, accent_alt) = match self {
            ThemeKind::Midnight => (
                0x0A0D13, 0x101724, 0x0D1119, 0x131A25, 0x1A2231, 0x0E131B, 0xE8EDF5, 0xA7B2C4, 0x6B778B, 0x3DFFC8, 0xFF4DC8,
            ),
            ThemeKind::Graphite => (
                0x141416, 0x1C1C20, 0x18181B, 0x1F1F23, 0x29292E, 0x111113, 0xF2F2F5, 0xB0B0B8, 0x76767F, 0x0A84FF, 0xBF5AF2,
            ),
            ThemeKind::Aurora => (
                0x0C0A18, 0x151032, 0x0F0C20, 0x17132B, 0x201B3A, 0x0E0B1E, 0xEDEAFE, 0xB3ACD6, 0x7A72A0, 0x8B7CFF, 0x2DE2C3,
            ),
            ThemeKind::Ember => (
                0x110C0A, 0x1E1310, 0x150F0D, 0x1D1513, 0x271C19, 0x140E0C, 0xF7EDE8, 0xC9B5AB, 0x8E7A70, 0xFF8A3D, 0xFF4D6D,
            ),
            ThemeKind::Daylight => (
                0xEEF1F6, 0xF7F9FC, 0xE6EAF1, 0xFFFFFF, 0xF6F8FB, 0xE8ECF2, 0x101521, 0x4A5568, 0x8A94A6, 0x0A84FF, 0xFF2D92,
            ),
            ThemeKind::Paper => (
                0xF1ECE3, 0xF8F4EC, 0xE9E3D7, 0xFFFDF8, 0xFAF6EE, 0xEAE3D6, 0x2A2520, 0x5F574D, 0x978C7E, 0xD9622B, 0x3A7D6B,
            ),
        };
        let dark = self.is_dark();
        let (ok, warn, crit, info) = if dark { dark_sem } else { light_sem };
        let (border, border_hi) = if dark { dark_lines } else { light_lines };
        let accent = hex(accent);
        Palette {
            dark,
            bg: hex(bg),
            bg_top: hex(bg_top),
            sidebar: hex(sidebar),
            surface: hex(surface),
            surface_hi: hex(surface_hi),
            inset: hex(inset),
            border,
            border_hi,
            text: hex(text),
            text2: hex(text2),
            muted: hex(muted),
            accent,
            accent_alt: hex(accent_alt),
            on_accent: on_color(accent),
            ok,
            warn,
            crit,
            info,
            shadow: if dark { Color32::from_black_alpha(110) } else { Color32::from_black_alpha(28) },
        }
    }
}

/// Black or white, whichever reads better on `c`.
pub fn on_color(c: Color32) -> Color32 {
    let l = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if l > 150.0 { Color32::from_rgb(10, 14, 20) } else { Color32::WHITE }
}

impl Palette {
    pub fn from_settings(s: &Settings) -> Self {
        let mut p = s.theme.palette();
        if s.custom_accent {
            p.accent = Color32::from_rgb(s.accent[0], s.accent[1], s.accent[2]);
            p.accent_alt = Color32::from_rgb(s.accent_alt[0], s.accent_alt[1], s.accent_alt[2]);
            p.on_accent = on_color(p.accent);
        }
        p
    }

    /// Colour for a utilisation level: calm, then warning, then critical.
    pub fn level(&self, frac: f32) -> Color32 {
        let t = frac.clamp(0.0, 1.0);
        if t < 0.6 {
            self.accent
        } else if t < 0.85 {
            lerp_color(self.accent, self.warn, (t - 0.6) / 0.25)
        } else {
            lerp_color(self.warn, self.crit, (t - 0.85) / 0.15)
        }
    }

    /// The accent at low opacity, for tinted backgrounds.
    pub fn tint(&self, c: Color32, a: f32) -> Color32 {
        let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
    }

    pub fn card_shadow(&self) -> Shadow {
        Shadow { offset: [0, 6], blur: 22, spread: 0, color: self.shadow }
    }

    /// Builds and installs the egui style.
    pub fn apply(&self, ctx: &egui::Context, text_scale: f32, compact: bool) {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.override_text_color = Some(self.text);
        v.panel_fill = self.bg;
        v.window_fill = self.surface;
        v.window_stroke = Stroke::new(1.0_f32, self.border_hi);
        v.window_corner_radius = CornerRadius::same(14);
        v.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: self.shadow };
        v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: self.shadow };
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = self.inset;
        v.faint_bg_color = self.surface_hi;
        v.code_bg_color = self.inset;
        v.hyperlink_color = self.accent;
        v.selection.bg_fill = self.tint(self.accent, 0.32);
        v.selection.stroke = Stroke::new(1.0_f32, self.accent);
        v.warn_fg_color = self.warn;
        v.error_fg_color = self.crit;
        v.striped = true;
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        v.indent_has_left_vline = false;

        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.surface;
        w.noninteractive.weak_bg_fill = self.surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text2);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = self.inset;
        w.inactive.weak_bg_fill = self.surface_hi;
        w.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        w.inactive.fg_stroke = Stroke::new(1.0_f32, self.text);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = self.surface_hi;
        w.hovered.weak_bg_fill = lerp_color(self.surface_hi, self.accent, 0.08);
        w.hovered.bg_stroke = Stroke::new(1.0_f32, self.tint(self.accent, 0.5));
        w.hovered.fg_stroke = Stroke::new(1.5_f32, self.text);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = self.tint(self.accent, 0.25);
        w.active.weak_bg_fill = self.tint(self.accent, 0.25);
        w.active.bg_stroke = Stroke::new(1.0_f32, self.accent);
        w.active.fg_stroke = Stroke::new(1.5_f32, self.text);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;
        w.open = w.hovered;

        let mut style = Style { visuals: v, ..Style::default() };
        let k = text_scale;
        let fam = |name: &str| FontFamily::Name(name.into());
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0 * k, fam("semibold"))),
            (TextStyle::Body, FontId::new(13.5 * k, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0 * k, fam("medium"))),
            (TextStyle::Small, FontId::new(11.0 * k, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0 * k, FontFamily::Monospace)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = if compact { vec2(6.0, 4.0) } else { vec2(8.0, 6.0) };
        s.button_padding = vec2(10.0, 5.0);
        s.interact_size = vec2(36.0, if compact { 22.0 } else { 26.0 });
        s.menu_margin = Margin::same(6);
        s.window_margin = Margin::same(14);
        s.slider_width = 180.0;
        s.combo_width = 160.0;
        s.icon_width = 16.0;
        s.icon_spacing = 6.0;
        s.scroll = egui::style::ScrollStyle::floating();
        s.scroll.bar_width = 8.0;
        style.interaction.selectable_labels = false;
        style.animation_time = 0.14;
        ctx.set_style(style);
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// A fixed calm → hot ramp, independent of the theme (used for heatmaps).
pub fn heat(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        lerp_color(hex(0x2DD4BF), hex(0xFBBF24), t * 2.0)
    } else {
        lerp_color(hex(0xFBBF24), hex(0xF43F5E), (t - 0.5) * 2.0)
    }
}

/// Font family helpers for the named weights installed by [`install_fonts`].
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Loads Avenir Next (four weights) and SF Mono from the system, if present.
/// The named families always exist, falling back to egui's bundled fonts.
pub fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions};
    use std::sync::Arc;

    let mut defs = FontDefinitions::default();
    let base_prop = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let base_mono = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();

    fn load(path: &str) -> Option<&'static [u8]> {
        std::fs::read(path).ok().map(|b| &*Box::leak(b.into_boxed_slice()))
    }
    let avenir = load("/System/Library/Fonts/Avenir Next.ttc");
    let sf_mono = load("/System/Library/Fonts/SFNSMono.ttf");

    let mut add = |name: &str, bytes: &'static [u8], index: u32, y: f32| {
        let mut fd = FontData::from_static(bytes);
        fd.index = index;
        fd.tweak.y_offset_factor = y;
        defs.font_data.insert(name.to_owned(), Arc::new(fd));
    };
    let mut weights: Vec<(&str, Option<&str>)> =
        vec![("regular", None), ("medium", None), ("semibold", None), ("bold", None)];
    if let Some(b) = avenir {
        // Face indices inside Avenir Next.ttc.
        add("avenir-regular", b, 7, 0.04);
        add("avenir-medium", b, 5, 0.04);
        add("avenir-demibold", b, 2, 0.04);
        add("avenir-bold", b, 0, 0.04);
        weights = vec![
            ("regular", Some("avenir-regular")),
            ("medium", Some("avenir-medium")),
            ("semibold", Some("avenir-demibold")),
            ("bold", Some("avenir-bold")),
        ];
    }
    if let Some(b) = sf_mono {
        add("sf-mono", b, 0, 0.0);
        let mut mono = vec!["sf-mono".to_owned()];
        mono.extend(base_mono);
        defs.families.insert(FontFamily::Monospace, mono);
    }
    for (family, face) in weights {
        let mut list: Vec<String> = face.map(|f| vec![f.to_owned()]).unwrap_or_default();
        list.extend(base_prop.iter().cloned());
        if family == "regular" {
            defs.families.insert(FontFamily::Proportional, list);
        } else {
            defs.families.insert(FontFamily::Name(family.into()), list);
        }
    }
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_has_readable_text() {
        for t in ThemeKind::ALL {
            let p = t.palette();
            let lum = |c: Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
            let gap = (lum(p.text) - lum(p.surface)).abs();
            assert!(gap > 120.0, "{} text contrast too low ({gap})", t.label());
            assert_eq!(p.dark, lum(p.bg) < 128.0, "{} dark flag disagrees with its background", t.label());
        }
    }

    #[test]
    fn on_color_picks_the_readable_side() {
        assert_eq!(on_color(Color32::WHITE), Color32::from_rgb(10, 14, 20));
        assert_eq!(on_color(Color32::from_rgb(10, 30, 80)), Color32::WHITE);
    }

    #[test]
    fn level_goes_calm_to_critical() {
        let p = ThemeKind::Midnight.palette();
        assert_eq!(p.level(0.2), p.accent);
        assert_eq!(p.level(1.0), p.crit);
    }
}
