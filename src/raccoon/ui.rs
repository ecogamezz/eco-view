//! The raccoon's section of the Settings page.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText, Ui};

use super::acts::{self, Act, Kind};
use super::settings::{Coat, RaccoonSettings};
use super::World;
use crate::theme::Palette;

/// Draws the panel. Returns true when a setting changed.
pub fn panel(ui: &mut Ui, pal: Palette, c: &mut RaccoonSettings, world: &Arc<Mutex<World>>) -> bool {
    let mut dirty = false;

    ui.label(RichText::new("Raccoon companion").heading());
    ui.label(
        RichText::new(
            "He lives on top of your desktop: walks along window title bars, climbs, naps when \
             the Mac is quiet and panics when it isn't. Pick him up and throw him, rub the cursor \
             over him to pet him, click to boop. Clicks anywhere else go straight through.",
        )
        .small()
        .color(pal.muted),
    );
    ui.horizontal(|ui| {
        dirty |= ui.checkbox(&mut c.enabled, "Let him out").changed();
        ui.label("Name");
        dirty |= ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(120.0)).changed();
    });

    status(ui, pal, c, world);

    if section(ui, pal, "Appearance", true) {
        dirty |= slider(ui, &mut c.size, 0.4..=3.5, "Size", "x");
        dirty |= slider(ui, &mut c.animation_speed, 0.2..=4.0, "Animation speed", "x");
        dirty |= slider(ui, &mut c.outline_width, 0.0..=5.0, "Outline", " pt");
        ui.horizontal_wrapped(|ui| {
            ui.label("Coat");
            for coat in Coat::ALL {
                let sel = c.coat == coat;
                if ui.selectable_label(sel, coat.label()).clicked() && !sel {
                    if coat == Coat::Custom {
                        let from = c.coat;
                        c.load_coat_into_custom(from);
                    }
                    c.coat = coat;
                    dirty = true;
                }
            }
        });
        if c.coat == Coat::Custom {
            egui::Grid::new("raccoon_colours").num_columns(5).show(ui, |ui| {
                for label in ["Fur", "Belly", "Mask", "Eyes", "Accent"] {
                    ui.label(RichText::new(label).small());
                }
                ui.end_row();
                for v in [&mut c.fur, &mut c.belly, &mut c.mask, &mut c.eyes, &mut c.accent] {
                    dirty |= ui.color_edit_button_srgb(v).changed();
                }
                ui.end_row();
            });
        }
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.show_shadow, "Shadow").changed();
            dirty |= ui.checkbox(&mut c.show_name_tag, "Name tag").changed();
            dirty |= ui.checkbox(&mut c.particles, "Effects").changed();
            dirty |= ui.checkbox(&mut c.speech_bubbles, "Speech bubbles").changed();
        });
        if c.speech_bubbles {
            dirty |= slider(ui, &mut c.chattiness, 0.0..=1.0, "Chattiness", "");
        }
    }

    if section(ui, pal, "Behaviour", false) {
        dirty |= slider(ui, &mut c.liveliness, 0.1..=3.0, "Liveliness", "x");
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.walk_on_windows, "Walk on windows").changed();
            dirty |= ui.checkbox(&mut c.walk_on_dashboard, "Walk on dashboard cards").changed();
            dirty |= ui.checkbox(&mut c.can_climb, "Climb windows").changed();
        });
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.follow_cursor, "Follow the cursor").changed();
            dirty |= ui.checkbox(&mut c.eyes_follow_cursor, "Eyes follow the cursor").changed();
            dirty |= ui.checkbox(&mut c.play_with_cursor, "Play with the cursor").changed();
        });
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.allow_petting, "Petting").changed();
            dirty |= ui.checkbox(&mut c.click_through, "Ignore the mouse (decorative)").changed();
        });
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.sleep_when_idle, "Nap when the Mac is idle").changed();
            dirty |= ui.checkbox(&mut c.sleepy_at_night, "Sleepy at night").changed();
        });
        if c.sleep_when_idle {
            dirty |= slider(ui, &mut c.sleep_after_secs, 5.0..=600.0, "Nap after", " s");
            dirty |= slider(ui, &mut c.sleep_below_cpu, 0.0..=60.0, "Idle means CPU below", "%");
        }
    }

    if section(ui, pal, "Reactions", false) {
        ui.label(RichText::new("What he notices about the Mac.").small().color(pal.muted));
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.react_cpu, "CPU").changed();
            dirty |= ui.checkbox(&mut c.react_gpu, "GPU").changed();
            dirty |= ui.checkbox(&mut c.react_temperature, "Heat").changed();
            dirty |= ui.checkbox(&mut c.react_battery, "Battery").changed();
            dirty |= ui.checkbox(&mut c.react_memory, "Memory").changed();
            dirty |= ui.checkbox(&mut c.react_network, "Downloads").changed();
        });
        dirty |= slider(ui, &mut c.panic_above_cpu, 10.0..=100.0, "Panic above CPU", "%");
        dirty |= slider(ui, &mut c.panic_above_gpu, 10.0..=100.0, "Panic above GPU", "%");
        dirty |= slider(ui, &mut c.hot_above_celsius, 40.0..=110.0, "Overheat above", " °C");
        dirty |= slider(ui, &mut c.low_battery_percent, 0.0..=50.0, "Low battery below", "%");
        dirty |= slider(ui, &mut c.memory_full_percent, 50.0..=100.0, "Memory full above", "%");
        dirty |= slider(ui, &mut c.download_mbps, 1.0..=1000.0, "Big download above", " Mbps");
    }

    if section(ui, pal, "Movement", false) {
        dirty |= slider(ui, &mut c.walk_speed, 5.0..=400.0, "Walk speed", " pt/s");
        dirty |= slider(ui, &mut c.run_speed, 10.0..=800.0, "Run speed", " pt/s");
        dirty |= slider(ui, &mut c.sprint_speed, 10.0..=1400.0, "Sprint speed", " pt/s");
        dirty |= slider(ui, &mut c.jump_height, 20.0..=600.0, "Jump height", " pt");
        dirty |= slider(ui, &mut c.climb_speed, 10.0..=400.0, "Climb speed", " pt/s");
    }

    if section(ui, pal, "Physics", false) {
        dirty |= slider(ui, &mut c.gravity, 100.0..=6000.0, "Gravity", "");
        dirty |= slider(ui, &mut c.air_drag, 0.0..=4.0, "Air drag", "");
        dirty |= slider(ui, &mut c.restitution, 0.0..=0.95, "Bounciness", "");
        dirty |= slider(ui, &mut c.wall_restitution, 0.0..=0.95, "Wall bounce", "");
        dirty |= slider(ui, &mut c.ground_friction, 0.0..=1.0, "Ground friction", "");
        dirty |= slider(ui, &mut c.throw_power, 0.0..=4.0, "Throw power", "x");
        dirty |= slider(ui, &mut c.max_throw_speed, 200.0..=8000.0, "Max throw speed", "");
        dirty |= slider(ui, &mut c.spin_per_speed, 0.0..=0.03, "Spin", "");
        dirty |= slider(ui, &mut c.hard_impact_speed, 100.0..=4000.0, "Hard landing", "");
        dirty |= slider(ui, &mut c.dangle_stiffness, 0.2..=4.0, "Dangle stiffness", "x");
        ui.horizontal_wrapped(|ui| {
            dirty |= ui.checkbox(&mut c.dizzy_after_hard_landing, "Dizzy after hard landings").changed();
            dirty |= ui.checkbox(&mut c.splat_on_walls, "Splat on walls").changed();
        });
    }

    if section(ui, pal, "Sound", false) {
        dirty |= ui.checkbox(&mut c.sounds, "Squeaks, thuds and purrs").changed();
        ui.label(RichText::new("Uses the app's sound volume.").small().color(pal.muted));
    }

    if section(ui, pal, "Per-activity tuning", false) {
        dirty |= slider(ui, &mut c.state_time_scale, 0.1..=5.0, "Global time scale", "x");
        ui.label(
            RichText::new("How long each activity lasts, how often he picks it, and how long before he repeats it.")
                .small()
                .color(pal.muted),
        );
        egui::ScrollArea::vertical().max_height(320.0).id_salt("raccoon_tuning").show(ui, |ui| {
            egui::Grid::new("raccoon_tuning_grid").num_columns(4).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Activity").small().strong());
                ui.label(RichText::new("Length").small().strong());
                ui.label(RichText::new("Chance").small().strong());
                ui.label(RichText::new("Cooldown").small().strong());
                ui.end_row();
                for d in acts::all() {
                    let mut t = c.tuning(d.act);
                    ui.label(d.label);
                    let a = ui.add(egui::DragValue::new(&mut t.duration_mul).range(0.05..=20.0).speed(0.02).suffix("x"));
                    let b = if matches!(d.kind, Kind::Ambient | Kind::Social) {
                        ui.add(egui::DragValue::new(&mut t.weight_mul).range(0.0..=50.0).speed(0.05).suffix("x"))
                    } else {
                        ui.label(RichText::new("—").color(pal.muted))
                    };
                    let cd = ui.add(egui::DragValue::new(&mut t.cooldown_mul).range(0.0..=10.0).speed(0.02).suffix("x"));
                    ui.end_row();
                    if a.changed() || b.changed() || cd.changed() {
                        c.states.insert(d.label.to_string(), t);
                        dirty = true;
                    }
                }
            });
        });
    }

    ui.add_space(4.0);
    if ui.button("Reset raccoon settings").clicked() {
        let enabled = c.enabled;
        *c = RaccoonSettings::default();
        c.enabled = enabled;
        dirty = true;
    }
    if dirty {
        c.sanitize();
    }
    dirty
}

fn status(ui: &mut Ui, pal: Palette, c: &RaccoonSettings, world: &Arc<Mutex<World>>) {
    let Ok(mut w) = world.lock() else { return };
    egui::Frame::new()
        .fill(pal.accent.gamma_multiply(0.08))
        .corner_radius(8.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{} is: {}", c.name, w.act.label())).color(pal.accent).strong());
                let place = if w.is_grabbed() {
                    "being carried"
                } else if w.support == Some(super::FLOOR) {
                    "on the floor"
                } else if w.support.is_some() {
                    "on a window"
                } else {
                    "in the air"
                };
                ui.label(RichText::new(place).small().color(pal.muted));
            });
            ui.horizontal(|ui| {
                meter(ui, "Energy", w.energy, Color32::from_rgb(120, 210, 120));
                meter(ui, "Boredom", w.boredom, Color32::from_rgb(230, 180, 80));
                meter(ui, "Affection", w.affection, Color32::from_rgb(255, 110, 150));
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Call him over").on_hover_text("Drops him in above the middle of the screen").clicked() {
                    let at = w.area.center();
                    w.summon(at);
                }
                if ui.button("Toss him").clicked() {
                    w.toss();
                }
                if ui.button("Nap").clicked() {
                    w.force(Act::Sleep);
                }
                if ui.button("Wake").clicked() && w.act == Act::Sleep {
                    w.force(Act::Wake);
                }
                let mut chosen: Option<Act> = None;
                egui::ComboBox::from_id_salt("raccoon_force")
                    .selected_text("Do something…")
                    .height(320.0)
                    .show_ui(ui, |ui| {
                        for d in acts::all() {
                            if matches!(d.act, Act::Dragged | Act::Climb | Act::Mantle | Act::Explore | Act::FollowCursor) {
                                continue;
                            }
                            if ui.selectable_label(false, d.label).clicked() {
                                chosen = Some(d.act);
                            }
                        }
                    });
                if let Some(a) = chosen {
                    w.force(a);
                }
            });
        });
}

fn meter(ui: &mut Ui, label: &str, v: f32, color: Color32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).small());
        ui.add(egui::ProgressBar::new(v.clamp(0.0, 1.0)).desired_width(90.0).fill(color));
    });
}

fn section(ui: &mut Ui, pal: Palette, title: &str, open: bool) -> bool {
    egui::CollapsingHeader::new(RichText::new(title).strong().color(pal.accent))
        .id_salt(("raccoon_section", title))
        .default_open(open)
        .show(ui, |ui| ui.add_space(2.0))
        .body_response
        .is_some()
}

fn slider(ui: &mut Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str, suffix: &str) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 18.0], egui::Label::new(label));
        ui.add(egui::Slider::new(v, range).suffix(suffix))
    })
    .inner
    .changed()
}
