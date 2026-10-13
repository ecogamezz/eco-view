//! The raccoon's window: a small, transparent, always-on-top viewport that
//! tracks the NPC across the desktop.
//!
//! It is deliberately *small* and follows the raccoon rather than covering the
//! screen. That is what keeps the rest of the desktop usable: clicks that miss
//! the window fall through to whatever app is underneath, so there is no need
//! for click-through regions or hit-test tricks. Only the handful of pixels
//! around the sprite belong to this window.

use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, ColorImage, Context, Image, Pos2, Rect, TextureOptions, Vec2, ViewportBuilder, ViewportId,
};

use crate::companion::Companion;
use crate::sprite::FRAME_W;

/// Size of the overlay window in points. Comfortably larger than the sprite so
/// it can spin and bounce without clipping.
pub const OVERLAY_SIZE: Vec2 = Vec2::new(160.0, 180.0);

/// Shared between the main window and the overlay viewport.
pub type Shared = Arc<Mutex<Companion>>;

/// Stable id for the overlay viewport.
pub fn viewport_id() -> ViewportId {
    ViewportId::from_hash_of("eco-companion-overlay")
}

/// Builds the overlay window. Called once, from the main window.
pub fn builder() -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title("eco view companion")
        .with_app_id("com.ecoview.companion")
        .with_inner_size(OVERLAY_SIZE)
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_visible(true)
}

/// One-shot things the overlay noticed, consumed by the main window.
#[derive(Default)]
pub struct OverlayEvents {
    pub grabbed: bool,
    pub released: bool,
    pub throw: Vec2,
}

/// The overlay's own UI: paints the sprite and handles grabbing it.
pub fn ui(ctx: &Context, shared: &Shared, out: &mut OverlayEvents) {
    let Ok(mut npc) = shared.lock() else { return };
    if !npc.enabled {
        return;
    }

    let handle = ctx.load_texture(
        npc.tex_name.clone(),
        ColorImage::clone(&npc.atlas.image),
        TextureOptions::NEAREST,
    );
    let tex = egui::load::SizedTexture::from_handle(&handle);

    // This viewport's own position on the desktop, so global positions can be
    // converted into window-local ones.
    let origin = ctx.input(|i| i.screen_rect()).min;
    let size = FRAME_W as f32 * npc.scale;
    // The sprite's ground point is `npc.pos`; lift it so the paws land there.
    let local = npc.pos - origin.to_vec2();
    let rect = Rect::from_center_size(local + Vec2::new(0.0, -size * 0.44), Vec2::splat(size));

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            if npc.show_shadow && npc.on_ground && !npc.is_grabbed() {
                ui.painter().add(egui::Shape::ellipse_filled(
                    local + Vec2::new(0.0, 2.0),
                    Vec2::new(size * 0.24, size * 0.08),
                    egui::Color32::from_black_alpha(70),
                ));
            }

            // Mirror by swapping the UV rect's x extents; `Image` has no
            // dedicated flip in this version.
            let mut uv = npc.anim.uv(&npc.atlas);
            if npc.flip_sprite && npc.facing < 0.0 {
                uv = Rect::from_min_max(
                    egui::epaint::Pos2::new(uv.max.x, uv.min.y),
                    egui::epaint::Pos2::new(uv.min.x, uv.max.y),
                );
            }

            let mut img = Image::new(tex).uv(uv).fit_to_exact_size(Vec2::splat(size));
            if npc.render_spin.abs() > 0.001 {
                // `rotate` takes its origin in normalised UV space over the whole
                // texture, so pivot about the centre of this cell, not the sheet.
                let pivot = uv.center().to_vec2();
                img = img.rotate(npc.render_spin * std::f32::consts::TAU, pivot);
            }
            ui.add(img);

            let sense = if npc.click_through {
                egui::Sense::hover()
            } else {
                egui::Sense::click_and_drag()
            };
            // Grab target is a little larger than the sprite, which makes it
            // easy to pick up without covering the whole window.
            let grab = rect.expand(size * 0.12);
            let resp = ui.interact(grab, ui.id().with("raccoon-grab"), sense);

            if !npc.click_through {
                if resp.drag_started() {
                    let p = ctx
                        .input(|i| i.pointer.interact_pos())
                        .map(|p| p + origin.to_vec2())
                        .unwrap_or(npc.pos);
                    npc.press(p);
                    out.grabbed = true;
                }
                if resp.dragged() {
                    let dt = ctx.input(|i| i.pointer.delta()).length();
                    let p = ctx
                        .input(|i| i.pointer.interact_pos())
                        .map(|p| p + origin.to_vec2())
                        .unwrap_or(npc.pos);
                    // Keep the window under the cursor: the sprite follows the
                    // pointer and the window follows the sprite, so a drag
                    // never escapes the viewport.
                    npc.drag_to(p, dt.clamp(0.0, 0.05).max(1.0 / 240.0));
                }
                if resp.drag_stopped() {
                    npc.release();
                    out.released = true;
                    out.throw = npc.last_throw;
                }
            }

            if npc.show_name_tag {
                ui.painter().text(
                    rect.center_bottom() + Vec2::new(0.0, 14.0),
                    egui::Align2::CENTER_CENTER,
                    "raccoon",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_white_alpha(190),
                );
            }
        });
}

/// Keeps the overlay window glued to the raccoon.
///
/// The window is recentred every frame, which means a fast throw still tracks,
/// and dragging keeps the pointer inside the window because the sprite follows
/// the pointer and the window follows the sprite.
pub fn sync(ctx: &Context, shared: &Shared) {
    let Ok(npc) = shared.lock() else { return };
    let scale = npc.scale;
    let pos = npc.pos;
    // Centre the window horizontally on the sprite and put its lower third just
    // below the ground point, so there is room to spin above and to fall below.
    let origin = pos - Vec2::new(OVERLAY_SIZE.x * 0.5, OVERLAY_SIZE.y * 0.62);
    let _ = scale;
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(origin));
}

/// Shows or hides the overlay window without destroying it.
#[allow(dead_code)]
pub fn set_visible(ctx: &Context, visible: bool) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
}

/// Destroys the overlay window.
#[allow(dead_code)]
pub fn close(ctx: &Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}

/// Global screen point -> window-local point.
#[allow(dead_code)]
pub fn to_local(global: Pos2, origin: Pos2) -> Pos2 {
    global - origin.to_vec2()
}