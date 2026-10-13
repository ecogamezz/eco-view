//! Everything about the companion that a user might reasonably want to tune.
//!
//! These live in their own struct so the settings page can present them as one
//! collapsible group, and so a reset only has to restore defaults for that
//! group.

use serde::{Deserialize, Serialize};

use crate::companion::State;
use crate::sprite::Palette;

/// Per-state tuning. `duration_mul` scales the moveset's own timing, `weight_mul`
/// scales how often an idle behaviour is picked, and `cooldown_mul` scales how
/// often it is allowed to repeat.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StateTuning {
    pub duration_mul: f32,
    pub weight_mul: f32,
    pub cooldown_mul: f32,
}

impl Default for StateTuning {
    fn default() -> Self {
        Self { duration_mul: 1.0, weight_mul: 1.0, cooldown_mul: 1.0 }
    }
}

/// A single tunable state row, stored as a sparse map so newly added states
/// appear with sensible defaults without needing a migration.
pub type StateTunings = std::collections::BTreeMap<String, StateTuning>;

fn tuning_of(map: &StateTunings, state: State) -> StateTuning {
    map.get(state_name(state)).copied().unwrap_or_default()
}

pub fn state_name(state: State) -> &'static str {
    // Reuse the moveset table so names never drift apart.
    crate::companion::def_for(state).label
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CompanionSettings {
    pub enabled: bool,

    // --- presentation ---
    pub sprite_scale: f32,
    pub animation_speed: f32,
    pub show_shadow: bool,
    pub show_name_tag: bool,
    pub flip_sprite_with_facing: bool,
    pub fur: [u8; 3],
    pub fur_dark: [u8; 3],
    pub fur_light: [u8; 3],
    pub cream: [u8; 3],
    pub mask: [u8; 3],
    pub iris: [u8; 3],
    pub outline: [u8; 3],

    // --- window ---
    /// Float above every other application instead of living in the dashboard.
    pub always_on_top: bool,
    /// Ignore mouse clicks entirely, so the raccoon is purely decorative.
    pub click_through: bool,
    pub follow_cursor: bool,

    // --- locomotion ---
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    pub turn_speed: f32,

    // --- physics ---
    pub gravity: f32,
    pub air_drag: f32,
    pub restitution: f32,
    pub ground_friction: f32,
    pub wall_restitution: f32,
    pub throw_power: f32,
    pub toss_strength: f32,
    pub spin_per_speed: f32,
    pub hard_impact_speed: f32,
    pub throw_dizzy: bool,

    // --- reactions ---
    pub sleep_below_cpu: f32,
    pub sleep_below_gpu: f32,
    pub panic_above_cpu: f32,
    pub panic_above_gpu: f32,
    pub react_to_cpu: bool,
    pub react_to_gpu: bool,
    pub react_to_memory: bool,
    pub react_to_battery: bool,
    pub react_to_network: bool,

    // --- timing ---
    pub state_time_scale: f32,
    pub idle_boredom: f32,
    pub states: StateTunings,
}

impl Default for CompanionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            sprite_scale: 1.6,
            animation_speed: 1.0,
            show_shadow: true,
            show_name_tag: false,
            flip_sprite_with_facing: true,
            fur: [126, 104, 88],
            fur_dark: [88, 71, 60],
            fur_light: [163, 140, 120],
            cream: [238, 224, 206],
            mask: [34, 29, 26],
            iris: [214, 150, 44],
            outline: [20, 16, 14],
            always_on_top: true,
            click_through: false,
            follow_cursor: false,
            walk_speed: 70.0,
            run_speed: 190.0,
            sprint_speed: 330.0,
            turn_speed: 1.0,
            gravity: 900.0,
            air_drag: 0.4,
            restitution: 0.45,
            ground_friction: 0.72,
            wall_restitution: 0.5,
            throw_power: 1.0,
            toss_strength: 260.0,
            spin_per_speed: 0.0022,
            hard_impact_speed: 620.0,
            throw_dizzy: true,
            sleep_below_cpu: 12.0,
            sleep_below_gpu: 12.0,
            panic_above_cpu: 88.0,
            panic_above_gpu: 90.0,
            react_to_cpu: true,
            react_to_gpu: true,
            react_to_memory: false,
            react_to_battery: false,
            react_to_network: false,
            state_time_scale: 1.0,
            idle_boredom: 1.0,
            states: StateTunings::new(),
        }
    }
}

impl CompanionSettings {
    pub fn palette(&self) -> Palette {
        Palette {
            fur: rgb(self.fur),
            fur_dark: rgb(self.fur_dark),
            fur_light: rgb(self.fur_light),
            cream: rgb(self.cream),
            eye_white: eframe::egui::Color32::from_rgb(246, 243, 235),
            mask: rgb(self.mask),
            iris: rgb(self.iris),
            nose: eframe::egui::Color32::from_rgb(
                (self.mask[0] as f32 * 0.75) as u8,
                (self.mask[1] as f32 * 0.75) as u8,
                (self.mask[2] as f32 * 0.75) as u8,
            ),
            outline: rgb(self.outline),
        }
    }

    pub fn weight_of(&self, state: State) -> f32 {
        tuning_of(&self.states, state).weight_mul
    }

    pub fn duration_of(&self, state: State) -> f32 {
        tuning_of(&self.states, state).duration_mul * self.state_time_scale
    }

    pub fn cooldown_of(&self, state: State) -> f32 {
        tuning_of(&self.states, state).cooldown_mul
    }

    /// Clamps everything that could destabilise the render loop or the physics.
    ///
    /// NaN and infinity are handled explicitly: `f32::clamp` propagates NaN, so
    /// a corrupt or hand-edited file with `"throw_power": null` would otherwise
    /// put NaN straight into the integrator.
    pub fn sanitize(&mut self) {
        fn n(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
            if v.is_finite() { v.clamp(lo, hi) } else { fallback }
        }
        let d = Self::default();
        self.sprite_scale = n(self.sprite_scale, 0.4, 5.0, d.sprite_scale);
        self.animation_speed = n(self.animation_speed, 0.1, 6.0, d.animation_speed);
        self.walk_speed = n(self.walk_speed, 0.0, 2000.0, d.walk_speed);
        self.run_speed = n(self.run_speed, 0.0, 3000.0, d.run_speed);
        self.sprint_speed = n(self.sprint_speed, 0.0, 4000.0, d.sprint_speed);
        self.gravity = n(self.gravity, 0.0, 6000.0, d.gravity);
        self.air_drag = n(self.air_drag, 0.0, 12.0, d.air_drag);
        self.restitution = n(self.restitution, 0.0, 1.0, d.restitution);
        self.ground_friction = n(self.ground_friction, 0.0, 1.0, d.ground_friction);
        self.wall_restitution = n(self.wall_restitution, 0.0, 1.0, d.wall_restitution);
        self.throw_power = n(self.throw_power, 0.0, 6.0, d.throw_power);
        self.toss_strength = n(self.toss_strength, 0.0, 2000.0, d.toss_strength);
        self.spin_per_speed = n(self.spin_per_speed, 0.0, 0.05, d.spin_per_speed);
        self.hard_impact_speed = n(self.hard_impact_speed, 10.0, 4000.0, d.hard_impact_speed);
        self.state_time_scale = n(self.state_time_scale, 0.1, 5.0, d.state_time_scale);
        self.idle_boredom = n(self.idle_boredom, 0.0, 4.0, d.idle_boredom);
        self.sleep_below_cpu = n(self.sleep_below_cpu, 0.0, 100.0, d.sleep_below_cpu);
        self.sleep_below_gpu = n(self.sleep_below_gpu, 0.0, 100.0, d.sleep_below_gpu);
        self.panic_above_cpu = n(self.panic_above_cpu, 0.0, 100.0, d.panic_above_cpu);
        self.panic_above_gpu = n(self.panic_above_gpu, 0.0, 100.0, d.panic_above_gpu);
        for v in self.states.values_mut() {
            v.duration_mul = n(v.duration_mul, 0.05, 20.0, 1.0);
            v.weight_mul = n(v.weight_mul, 0.0, 50.0, 1.0);
            v.cooldown_mul = n(v.cooldown_mul, 0.0, 10.0, 1.0);
        }
    }
}

fn rgb(v: [u8; 3]) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgb(v[0], v[1], v[2])
}