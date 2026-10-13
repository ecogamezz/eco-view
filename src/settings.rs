use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::raccoon::RaccoonSettings;
use crate::grid::{CardId, GridLayout};
use crate::particles::ParticleStyle;

/// Theme presets. Older settings files said "Dark" / "Light".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ThemeKind {
    #[serde(alias = "Dark")]
    Midnight,
    Graphite,
    Aurora,
    Ember,
    #[serde(alias = "Light")]
    Daylight,
    Paper,
}

/// Version of the dashboard grid's default arrangement.
pub const LAYOUT_VERSION: u32 = 2;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeKind,
    /// Use `accent` / `accent_alt` instead of the theme's own accent.
    pub custom_accent: bool,
    pub accent: [u8; 3],
    pub accent_alt: [u8; 3],
    pub surface: [u8; 3],
    pub text_scale: f32,
    pub refresh_ms: u64,
    pub warn_cpu: f32,
    pub warn_ram: f32,
    pub warn_gpu: f32,
    pub warn_temp: f32,
    pub particles: ParticleStyle,
    pub particle_intensity: f32,
    pub show_toasts: bool,
    pub show_banner: bool,
    pub sound_volume: f32,
    pub sound_enabled: bool,
    pub autostart: bool,
    pub discord_enabled: bool,
    pub discord_app_id: String,
    pub discord_details: String,
    pub discord_state: String,
    pub discord_large_image: String,
    pub discord_large_text: String,
    pub custom_icon: Option<PathBuf>,
    pub layout: GridLayout,
    pub visible: Vec<CardId>,
    pub cpu_heatmap: bool,
    pub net_heatmap: bool,
    pub companion_enabled: bool,
    /// Tighter spacing everywhere.
    pub compact: bool,
    /// Bumped when the default dashboard changes shape; older layouts are reset once.
    #[serde(default)]
    pub layout_version: u32,
    pub sidebar_collapsed: bool,
    /// Skip page transitions and animated numbers.
    pub reduce_motion: bool,
    /// Time window of the Performance charts, in seconds.
    pub chart_window_secs: u32,
    /// Post macOS notifications for alerts (the in-app alert centre always logs them).
    pub native_alerts: bool,
    /// Minimum seconds between two alerts.
    pub alert_cooldown_secs: u32,
    /// Where the user last dragged the floating companion chip, in window coords.
    /// Kept out of the grid so it can never push cards around.
    pub companion_chip_pos: Option<[f32; 2]>,
    /// Everything tunable about the raccoon.
    pub companion: RaccoonSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeKind::Midnight,
            custom_accent: false,
            accent: [0x3D, 0xFF, 0xC8],
            accent_alt: [0xFF, 0x4D, 0xC8],
            surface: [0x12, 0x16, 0x22],
            text_scale: 1.0,
            refresh_ms: 1000,
            warn_cpu: 90.0,
            warn_ram: 90.0,
            warn_gpu: 90.0,
            warn_temp: 95.0,
            particles: ParticleStyle::AuroraWaves,
            particle_intensity: 0.35,
            show_toasts: true,
            show_banner: true,
            sound_volume: 0.4,
            sound_enabled: true,
            autostart: false,
            discord_enabled: false,
            discord_app_id: String::new(),
            discord_details: "watching {host}".into(),
            discord_state: "CPU {cpu} · RAM {ram}".into(),
            discord_large_image: String::new(),
            discord_large_text: "eco view".into(),
            custom_icon: None,
            layout: GridLayout::default_dashboard(),
            visible: CardId::all().to_vec(),
            cpu_heatmap: true,
            net_heatmap: false,
            companion_enabled: false,
            compact: false,
            layout_version: LAYOUT_VERSION,
            sidebar_collapsed: false,
            reduce_motion: false,
            chart_window_secs: 300,
            native_alerts: true,
            alert_cooldown_secs: 60,
            companion_chip_pos: None,
            companion: RaccoonSettings::default(),
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home)
            .join("Library/Application Support/eco-view/settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut settings: Settings = serde_json::from_str(&text).unwrap_or_else(|err| {
            // Never silently discard a hand-built layout: park the unreadable
            // file and start clean so the user can still recover it by hand.
            let backup = path.with_extension("json.corrupt");
            let _ = fs::copy(&path, &backup);
            eprintln!("eco view: could not read settings ({err}); copied to {backup:?}");
            Self::default()
        });
        settings.sanitize();
        settings
    }

    /// Clamp anything that JSON can carry but the UI assumes is in range.
    /// `#[serde(default)]` only fills in *missing* fields, so a hand-edited or
    /// corrupted file could otherwise ask for, say, a particle count in the
    /// tens of millions and stall the render loop on a huge allocation.
    pub fn sanitize(&mut self) {
        self.text_scale = self.text_scale.clamp(0.7, 2.0);
        self.refresh_ms = self.refresh_ms.clamp(250, 5000);
        self.sound_volume = self.sound_volume.clamp(0.0, 1.0);
        self.particle_intensity = self.particle_intensity.clamp(0.0, 1.0);
        self.warn_cpu = self.warn_cpu.clamp(1.0, 100.0);
        self.warn_ram = self.warn_ram.clamp(1.0, 100.0);
        self.warn_gpu = self.warn_gpu.clamp(1.0, 100.0);
        self.warn_temp = self.warn_temp.clamp(1.0, 150.0);
        self.chart_window_secs = self.chart_window_secs.clamp(30, 900);
        self.alert_cooldown_secs = self.alert_cooldown_secs.clamp(5, 3600);
        if let Some(pos) = self.companion_chip_pos {
            self.companion_chip_pos = Some([
                if pos[0].is_finite() { pos[0] } else { 24.0 },
                if pos[1].is_finite() { pos[1] } else { 72.0 },
            ]);
        }
        self.companion.sanitize();
        if self.layout_version < LAYOUT_VERSION {
            self.layout = GridLayout::default_dashboard();
            self.layout_version = LAYOUT_VERSION;
        }
        self.layout.sanitize();
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(text) = serde_json::to_string_pretty(self) else {
            return;
        };
        // Write-then-rename so a crash mid-write cannot leave a truncated file.
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }

    /// Validates the image and keeps a private copy next to the settings, so
    /// later launches never have to reach into Downloads or Desktop.
    pub fn set_icon_from_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let dest = Self::path().with_file_name("icon.png");
        if let Some(dir) = dest.parent() {
            let _ = fs::create_dir_all(dir);
        }
        match img.save(&dest) {
            Ok(()) => self.custom_icon = Some(dest),
            Err(_) => self.custom_icon = Some(path.to_path_buf()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_theme_names_still_load() {
        let s: Settings = serde_json::from_str(r#"{"theme":"Light"}"#).unwrap();
        assert_eq!(s.theme, ThemeKind::Daylight);
        let s: Settings = serde_json::from_str(r#"{"theme":"Dark"}"#).unwrap();
        assert_eq!(s.theme, ThemeKind::Midnight);
    }

    #[test]
    fn layouts_from_older_builds_are_reset_once() {
        let mut s: Settings = serde_json::from_str(r#"{"layout":{"items":[{"id":"Cpu","col":3,"row":3,"w":8,"h":4}]}}"#).unwrap();
        assert_eq!(s.layout_version, 0);
        s.sanitize();
        assert_eq!(s.layout_version, LAYOUT_VERSION);
        assert_eq!(s.layout.items.len(), GridLayout::default_dashboard().items.len());
        // Once migrated, the user's own edits survive.
        s.layout.items.retain(|i| i.id != CardId::Devices);
        s.sanitize();
        assert!(!s.layout.items.iter().any(|i| i.id == CardId::Devices));
    }

    #[test]
    fn default_layout_has_no_overlaps() {
        let l = GridLayout::default_dashboard();
        for a in &l.items {
            for b in &l.items {
                assert!(!a.overlaps(b), "{:?} overlaps {:?}", a.id, b.id);
            }
            assert!(a.col + a.w <= crate::grid::COLS);
        }
    }
}
