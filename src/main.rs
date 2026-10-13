mod app;
mod audio;
mod autostart;
mod companion;
mod companion_settings;
mod devices;
mod discord;
mod format;
mod gpu;
mod grid;
mod icons;
mod metrics;
mod notifications;
mod overlay;
mod particles;
mod settings;
mod sound;
mod sprite;
mod theme;
mod ui;

use eframe::egui::{IconData, ViewportBuilder};

fn main() -> eframe::Result<()> {
    let icon = default_icon();
    let mut viewport = ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([960.0, 640.0])
        .with_title("eco view")
        .with_app_id("com.ecoview.monitor");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "eco view",
        options,
        Box::new(|cc| Ok(Box::new(app::EcoView::new(cc)))),
    )
}

fn default_icon() -> Option<std::sync::Arc<IconData>> {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = 32.0f32;
    let cy = 34.0f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let i = ((y * size + x) * 4) as usize;
            let in_head = dx * dx + (dy + 4.0) * (dy + 4.0) < 22.0 * 22.0;
            let mask = (dx.abs() - 8.0).abs() < 7.0 && (dy + 2.0).abs() < 6.0;
            let ear = {
                let lx = dx + 14.0;
                let rx = dx - 14.0;
                let ey = dy + 18.0;
                lx * lx + ey * ey < 36.0 || rx * rx + ey * ey < 36.0
            };
            if in_head || ear {
                if mask {
                    rgba[i] = 28;
                    rgba[i + 1] = 24;
                    rgba[i + 2] = 20;
                } else {
                    rgba[i] = 118;
                    rgba[i + 1] = 96;
                    rgba[i + 2] = 82;
                }
                rgba[i + 3] = 255;
            }
        }
    }
    Some(std::sync::Arc::new(IconData {
        rgba,
        width: size,
        height: size,
    }))
}
