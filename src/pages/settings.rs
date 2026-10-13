//! Settings, grouped into sections with a list on the left.

use eframe::egui::{self, Align, Align2, Color32, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{EcoView, Level, SettingsSection};
use crate::grid::CardId;
use crate::icons::{self, Icon};
use crate::metrics;
use crate::particles::ParticleStyle;
use crate::settings::ThemeKind;
use crate::sound::Cue;
use crate::theme::{self, Palette, lerp_color};
use crate::widgets::{self, ButtonKind};

const SECTIONS: [(SettingsSection, &str, Icon); 8] = [
    (SettingsSection::General, "General", Icon::Settings),
    (SettingsSection::Appearance, "Appearance", Icon::Palette),
    (SettingsSection::Dashboard, "Dashboard", Icon::Overview),
    (SettingsSection::Alerts, "Alerts", Icon::Bell),
    (SettingsSection::Sound, "Sound", Icon::Audio),
    (SettingsSection::Discord, "Discord", Icon::Plug),
    (SettingsSection::Raccoon, "Raccoon", Icon::Paw),
    (SettingsSection::About, "About", Icon::Info),
];

impl EcoView {
    pub(crate) fn settings_page(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let h = ui.available_height();
        ui.horizontal_top(|ui| {
            // Section list.
            ui.allocate_ui_with_layout(vec2(196.0, h), Layout::top_down(Align::Min), |ui| {
                for (sec, label, icon) in SECTIONS {
                    let (r, resp) = ui.allocate_exact_size(vec2(188.0, 34.0), Sense::click());
                    let sel = self.section == sec;
                    let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                    let p = ui.painter();
                    if sel {
                        p.rect_filled(r, 9.0, pal.surface);
                        p.rect_stroke(r, 9.0, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
                    } else if hov > 0.0 {
                        p.rect_filled(r, 9.0, pal.tint(pal.text, 0.05 * hov));
                    }
                    let ir = Rect::from_center_size(pos2(r.left() + 20.0, r.center().y), Vec2::splat(24.0));
                    p.rect_filled(ir, 7.0, if sel { pal.accent } else { pal.tint(pal.text, 0.07) });
                    icons::paint_in(p, ir.shrink(5.0), icon, if sel { pal.on_accent } else { pal.text2 });
                    p.text(pos2(r.left() + 42.0, r.center().y), Align2::LEFT_CENTER, label, theme::medium(13.5), if sel { pal.text } else { pal.text2 });
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        self.section = sec;
                    }
                    ui.add_space(2.0);
                }
            });
            ui.add_space(18.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), h), Layout::top_down(Align::Min), |ui| {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(760.0);
                    let dirty = match self.section {
                        SettingsSection::General => self.s_general(ui),
                        SettingsSection::Appearance => self.s_appearance(ui),
                        SettingsSection::Dashboard => self.s_dashboard(ui),
                        SettingsSection::Alerts => self.s_alerts(ui),
                        SettingsSection::Sound => self.s_sound(ui),
                        SettingsSection::Discord => self.s_discord(ui),
                        SettingsSection::Raccoon => {
                            let mut d = false;
                            widgets::card(ui, &pal, None, |ui| {
                                d = crate::raccoon::ui::panel(ui, pal, &mut self.settings.companion, &self.raccoon.world);
                            });
                            d
                        }
                        SettingsSection::About => self.s_about(ui),
                    };
                    if dirty {
                        self.persist();
                    }
                    ui.add_space(20.0);
                });
            });
        });
    }

    fn s_general(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        widgets::section_title(ui, &pal, "General", Some("How often eco view samples, and how it starts."));
        widgets::group(ui, &pal, Some("Sampling"), |ui| {
            widgets::row(ui, &pal, "Refresh interval", Some("Lower is livelier but uses a little more CPU."), |ui| {
                let mut ms = self.settings.refresh_ms;
                if widgets::segmented(ui, &pal, &mut ms, &[(500, "0.5 s"), (1000, "1 s"), (2000, "2 s"), (5000, "5 s")]) {
                    self.settings.refresh_ms = ms;
                    dirty = true;
                }
            });
        });
        widgets::group(ui, &pal, Some("Startup"), |ui| {
            widgets::row(ui, &pal, "Open at login", Some("Adds a per-user LaunchAgent that starts this copy of eco view."), |ui| {
                let mut on = self.settings.autostart;
                if widgets::toggle(ui, &pal, &mut on).changed() {
                    self.settings.autostart = on;
                    match crate::autostart::set_enabled(on) {
                        Ok(()) => self.toast(Level::Ok, if on { "Opens at login" } else { "Won't open at login" }, ""),
                        Err(e) => self.toast(Level::Crit, "Couldn't update the login item", e),
                    }
                    dirty = true;
                }
            });
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "App icon", Some("PNG, JPEG, BMP or ICO. Shown in the Dock while eco view runs."), |ui| {
                if self.settings.custom_icon.is_some() && widgets::button(ui, &pal, ButtonKind::Ghost, None, "Restore default").clicked() {
                    self.settings.custom_icon = None;
                    dirty = true;
                    self.toast(Level::Info, "Default icon restored", "Takes effect on next launch.");
                }
                if widgets::button(ui, &pal, ButtonKind::Secondary, None, "Choose…").clicked()
                    && let Some(path) = crate::app::native_open()
                {
                    match self.settings.set_icon_from_file(&path) {
                        Ok(()) => {
                            crate::app::apply_icon(ui.ctx(), &self.settings);
                            self.toast(Level::Ok, "Icon updated", "");
                            dirty = true;
                        }
                        Err(e) => self.toast(Level::Crit, "That image couldn't be read", e),
                    }
                }
            });
        });
        dirty
    }

    fn s_appearance(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        widgets::section_title(ui, &pal, "Appearance", Some("Pick a theme, an accent and how dense things feel."));
        widgets::group(ui, &pal, Some("Theme"), |ui| {
            ui.add_space(6.0);
            let w = ((ui.available_width() - 2.0 * 12.0) / 3.0).max(120.0);
            for chunk in ThemeKind::ALL.chunks(3) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for &t in chunk {
                        if theme_tile(ui, &pal, t, self.settings.theme == t, w) {
                            self.settings.theme = t;
                            dirty = true;
                        }
                    }
                });
                ui.add_space(10.0);
            }
        });
        widgets::group(ui, &pal, Some("Accent"), |ui| {
            widgets::row(ui, &pal, "Use the theme's accent", None, |ui| {
                let mut on = !self.settings.custom_accent;
                if widgets::toggle(ui, &pal, &mut on).changed() {
                    self.settings.custom_accent = !on;
                    dirty = true;
                }
            });
            if self.settings.custom_accent {
                widgets::separator(ui, &pal);
                let presets = [0x3DFFC8, 0x0A84FF, 0x8B7CFF, 0xFF8A3D, 0xFF2D92, 0x34D399, 0xFBBF24, 0xF43F5E];
                let colors: Vec<Color32> = presets.iter().map(|v| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, *v as u8)).collect();
                widgets::row(ui, &pal, "Accent", None, |ui| {
                    let mut c = Color32::from_rgb(self.settings.accent[0], self.settings.accent[1], self.settings.accent[2]);
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        self.settings.accent = [c.r(), c.g(), c.b()];
                        dirty = true;
                    }
                    if let Some(p) = widgets::swatches(ui, &pal, c, &colors) {
                        self.settings.accent = [p.r(), p.g(), p.b()];
                        dirty = true;
                    }
                });
                widgets::row(ui, &pal, "Secondary", None, |ui| {
                    let mut c = Color32::from_rgb(self.settings.accent_alt[0], self.settings.accent_alt[1], self.settings.accent_alt[2]);
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        self.settings.accent_alt = [c.r(), c.g(), c.b()];
                        dirty = true;
                    }
                    if let Some(p) = widgets::swatches(ui, &pal, c, &colors) {
                        self.settings.accent_alt = [p.r(), p.g(), p.b()];
                        dirty = true;
                    }
                });
            }
        });
        widgets::group(ui, &pal, Some("Layout & motion"), |ui| {
            widgets::row(ui, &pal, "Text size", None, |ui| {
                if widgets::slider(ui, &mut self.settings.text_scale, 0.8..=1.3, "×").changed() {
                    dirty = true;
                }
            });
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Compact spacing", Some("Fit more on screen."), |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.compact).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Reduce motion", Some("No page transitions or animated numbers."), |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.reduce_motion).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Collapsed sidebar", Some("Icons only. Toggle any time with ⌘B."), |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.sidebar_collapsed).changed());
        });
        widgets::group(ui, &pal, Some("Background"), |ui| {
            widgets::row(ui, &pal, "Ambient effect", None, |ui| {
                egui::ComboBox::from_id_salt("particles").selected_text(self.settings.particles.label()).show_ui(ui, |ui| {
                    for s in ParticleStyle::all() {
                        if ui.selectable_label(self.settings.particles == *s, s.label()).clicked() {
                            self.settings.particles = *s;
                            dirty = true;
                        }
                    }
                });
            });
            if self.settings.particles != ParticleStyle::Off {
                widgets::separator(ui, &pal);
                widgets::row(ui, &pal, "Intensity", None, |ui| dirty |= widgets::slider(ui, &mut self.settings.particle_intensity, 0.0..=1.0, "").changed());
            }
        });
        dirty
    }

    fn s_dashboard(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        widgets::section_title(ui, &pal, "Dashboard", Some("Choose which cards appear on the Overview."));
        widgets::group(ui, &pal, Some("Cards"), |ui| {
            let ids = CardId::all();
            for (i, id) in ids.iter().enumerate() {
                let mut on = self.settings.visible.contains(id);
                let changed = ui
                    .horizontal(|ui| {
                        ui.set_min_height(34.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                        ui.painter().rect_filled(r, 7.0, pal.tint(pal.accent, 0.12));
                        icons::paint_in(ui.painter(), r.shrink(5.0), Icon::for_card(*id), pal.accent);
                        ui.label(RichText::new(id.title()).font(theme::medium(13.0)).color(pal.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| widgets::toggle(ui, &pal, &mut on).changed()).inner
                    })
                    .inner;
                if changed {
                    if on {
                        self.settings.visible.push(*id);
                    } else {
                        self.settings.visible.retain(|v| v != id);
                    }
                    dirty = true;
                }
                if i + 1 < ids.len() {
                    widgets::separator(ui, &pal);
                }
            }
        });
        widgets::group(ui, &pal, Some("Details"), |ui| {
            widgets::row(ui, &pal, "Per-core bars on the CPU card", None, |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.cpu_heatmap).changed());
        });
        widgets::group(ui, &pal, Some("Arrangement"), |ui| {
            widgets::row(ui, &pal, "Layout", Some("Drag cards around and resize them on the Overview."), |ui| {
                if widgets::button(ui, &pal, ButtonKind::Ghost, Some(Icon::Refresh), "Reset").clicked() {
                    self.settings.layout = crate::grid::GridLayout::default_dashboard();
                    dirty = true;
                    self.toast(Level::Ok, "Layout reset", "");
                }
                if widgets::button(ui, &pal, ButtonKind::Secondary, Some(Icon::Layout), "Edit layout").clicked() {
                    self.edit_layout = true;
                    self.go(crate::app::Page::Overview);
                }
            });
        });
        dirty
    }

    fn s_alerts(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        let s = self.m.snap.clone();
        widgets::section_title(ui, &pal, "Alerts", Some("Get told when something crosses a limit."));
        widgets::group(ui, &pal, Some("Thresholds"), |ui| {
            let mem = metrics::pct(s.mem_used, s.mem_total);
            let rows: [(&str, f32, &mut f32, std::ops::RangeInclusive<f32>, &str); 4] = [
                ("CPU", s.cpu_global, &mut self.settings.warn_cpu, 30.0..=100.0, "%"),
                ("Memory", mem, &mut self.settings.warn_ram, 30.0..=100.0, "%"),
                ("GPU", s.gpu.load.unwrap_or(0.0), &mut self.settings.warn_gpu, 30.0..=100.0, "%"),
                ("Temperature", s.temperature.as_ref().map(|t| t.celsius).unwrap_or(0.0), &mut self.settings.warn_temp, 50.0..=110.0, "°C"),
            ];
            let n = rows.len();
            for (i, (name, now, v, range, unit)) in rows.into_iter().enumerate() {
                let desc = format!("Now {now:.0}{unit}");
                widgets::row(ui, &pal, name, Some(&desc), |ui| {
                    dirty |= widgets::slider(ui, v, range, unit).changed();
                });
                if i + 1 < n {
                    widgets::separator(ui, &pal);
                }
            }
        });
        widgets::group(ui, &pal, Some("How you're told"), |ui| {
            widgets::row(ui, &pal, "In-app toasts", None, |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.show_toasts).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Banner above the page", None, |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.show_banner).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "macOS notifications", Some("Shows up in Notification Centre."), |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.native_alerts).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Quiet period", Some("Minimum time between two alerts."), |ui| dirty |= widgets::slider(ui, &mut self.settings.alert_cooldown_secs, 5..=600, " s").changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Try it", None, |ui| {
                if widgets::button(ui, &pal, ButtonKind::Secondary, Some(Icon::Bell), "Send a test alert").clicked() {
                    self.toast(Level::Warn, "This is a test alert", "Real ones look just like this.");
                }
            });
        });
        dirty
    }

    fn s_sound(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        widgets::section_title(ui, &pal, "Sound", Some("Subtle cues for clicks and alerts, using macOS system sounds."));
        widgets::group(ui, &pal, None, |ui| {
            widgets::row(ui, &pal, "Play sounds", None, |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.sound_enabled).changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Volume", None, |ui| dirty |= widgets::slider(ui, &mut self.settings.sound_volume, 0.0..=1.0, "").changed());
            widgets::separator(ui, &pal);
            widgets::row(ui, &pal, "Preview", None, |ui| {
                for (label, cue) in [("Alert", Cue::Warn), ("Accent", Cue::Accent), ("Click", Cue::Click)] {
                    if widgets::button(ui, &pal, ButtonKind::Secondary, None, label).clicked() {
                        self.sound.play(cue);
                    }
                }
            });
        });
        dirty
    }

    fn s_discord(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        let mut dirty = false;
        widgets::section_title(ui, &pal, "Discord Rich Presence", Some("Show live stats on your Discord profile through the local Discord app."));
        widgets::group(ui, &pal, None, |ui| {
            widgets::row(ui, &pal, "Enabled", Some("Needs the Discord desktop app running."), |ui| dirty |= widgets::toggle(ui, &pal, &mut self.settings.discord_enabled).changed());
            for (label, value, hint) in [
                ("Application ID", &mut self.settings.discord_app_id, "From the Discord Developer Portal"),
                ("Details", &mut self.settings.discord_details, "watching {host}"),
                ("State", &mut self.settings.discord_state, "CPU {cpu} · RAM {ram}"),
                ("Large image key", &mut self.settings.discord_large_image, "an asset key"),
                ("Large image text", &mut self.settings.discord_large_text, "eco view"),
            ] {
                widgets::separator(ui, &pal);
                widgets::row(ui, &pal, label, None, |ui| {
                    dirty |= ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(280.0)).changed();
                });
            }
        });
        ui.label(RichText::new("PLACEHOLDERS").font(theme::semibold(11.0)).color(pal.muted));
        ui.horizontal_wrapped(|ui| {
            for t in ["{cpu}", "{ram}", "{gpu}", "{temp}", "{host}", "{uptime}", "{network_down}", "{network_up}"] {
                if widgets::pill(ui, &pal, t, pal.accent).interact(Sense::click()).on_hover_text("Click to copy").clicked() {
                    ui.ctx().copy_text(t.into());
                }
            }
        });
        dirty
    }

    fn s_about(&mut self, ui: &mut Ui) -> bool {
        let pal = self.pal;
        widgets::section_title(ui, &pal, "About eco view", None);
        widgets::card(ui, &pal, None, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                ui.painter().rect_filled(r, 16.0, pal.accent);
                icons::paint_in(ui.painter(), r.shrink(14.0), Icon::Leaf, pal.on_accent);
                ui.vertical(|ui| {
                    ui.label(RichText::new("eco view").font(theme::bold(22.0)).color(pal.text));
                    widgets::caption(ui, &pal, format!("Version {} · native macOS · Intel x86_64", env!("CARGO_PKG_VERSION")));
                    widgets::caption(ui, &pal, "Everything stays on this Mac. No telemetry.");
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, &pal, ButtonKind::Secondary, Some(Icon::Storage), "Show settings file").clicked() {
                    let _ = std::process::Command::new("open").arg("-R").arg(crate::settings::Settings::path()).spawn();
                }
            });
        });
        ui.add_space(12.0);
        widgets::group(ui, &pal, Some("Keyboard shortcuts"), |ui| {
            let keys = [
                ("Command palette", "⌘K"),
                ("Overview … System", "⌘1 – ⌘6"),
                ("Settings", "⌘,"),
                ("Edit dashboard layout", "⌘E"),
                ("Toggle sidebar", "⌘B"),
                ("Close popovers / finish editing", "esc"),
            ];
            for (i, (what, key)) in keys.iter().enumerate() {
                widgets::row(ui, &pal, what, None, |ui| {
                    widgets::pill(ui, &pal, key, pal.text2);
                });
                if i + 1 < keys.len() {
                    widgets::separator(ui, &pal);
                }
            }
        });
        false
    }
}

/// A clickable miniature of a theme. Returns true when picked.
fn theme_tile(ui: &mut Ui, pal: &Palette, t: ThemeKind, selected: bool, w: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 112.0), Sense::click());
    let tp = t.palette();
    let p = ui.painter();
    let preview = Rect::from_min_size(rect.min, vec2(w, 84.0));
    p.rect_filled(preview, 10.0, tp.bg);
    // Sidebar, header line and two cards.
    p.rect_filled(Rect::from_min_size(preview.min, vec2(w * 0.24, preview.height())), egui::CornerRadius { nw: 10, sw: 10, ne: 0, se: 0 }, tp.sidebar);
    for i in 0..3 {
        p.rect_filled(Rect::from_min_size(preview.min + vec2(8.0, 12.0 + i as f32 * 12.0), vec2(w * 0.24 - 16.0, 5.0)), 2.5, if i == 0 { tp.accent } else { tp.tint(tp.text, 0.2) });
    }
    let cx = preview.left() + w * 0.24 + 8.0;
    let cw = (preview.right() - cx - 16.0) * 0.5;
    for i in 0..2 {
        let cr = Rect::from_min_size(pos2(cx + i as f32 * (cw + 8.0), preview.top() + 14.0), vec2(cw, preview.height() - 26.0));
        p.rect_filled(cr, 6.0, tp.surface);
        p.rect_stroke(cr, 6.0, Stroke::new(1.0_f32, tp.border), StrokeKind::Inside);
        let c = if i == 0 { tp.accent } else { tp.accent_alt };
        widgets::ring(p, &tp, pos2(cr.left() + 16.0, cr.top() + 18.0), 9.0, 3.0, 0.65 - i as f32 * 0.3, c);
        p.rect_filled(Rect::from_min_size(pos2(cr.left() + 8.0, cr.bottom() - 14.0), vec2(cr.width() - 16.0, 4.0)), 2.0, tp.inset);
        p.rect_filled(Rect::from_min_size(pos2(cr.left() + 8.0, cr.bottom() - 14.0), vec2((cr.width() - 16.0) * (0.7 - i as f32 * 0.3), 4.0)), 2.0, c);
    }
    let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let ring = if selected { pal.accent } else { lerp_color(pal.border, pal.border_hi, hov) };
    p.rect_stroke(preview.expand(if selected { 2.0 } else { 0.0 }), 12.0, Stroke::new(if selected { 2.0_f32 } else { 1.0 }, ring), StrokeKind::Outside);
    p.text(pos2(rect.left() + 2.0, rect.bottom() - 12.0), Align2::LEFT_CENTER, t.label(), theme::medium(13.0), if selected { pal.text } else { pal.text2 });
    if selected {
        let c = pos2(rect.right() - 10.0, rect.bottom() - 12.0);
        p.circle_filled(c, 8.0, pal.accent);
        icons::paint_in(p, Rect::from_center_size(c, Vec2::splat(11.0)), Icon::Check, pal.on_accent);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}
