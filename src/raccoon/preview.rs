//! Offline rendering of the raccoon to a PNG contact sheet, one cell per
//! activity. Used by `eco-view --raccoon-sheet out.png` and by the tests, so
//! the art can be checked without opening a window.

use eframe::egui::{Color32, Mesh, Pos2, Rect, pos2, vec2};

use super::acts::{self, Act};
use super::render::{Canvas, Look, draw_pose};
use super::{Inputs, RaccoonSettings, Vitals, World};

/// A premultiplied RGBA float canvas.
pub struct Raster {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Raster {
    pub fn new(w: usize, h: usize, bg: Color32) -> Self {
        let c = to_f(bg);
        Self { w, h, px: vec![c; w * h] }
    }

    /// Rasterises a mesh whose positions are in pixels.
    pub fn draw(&mut self, mesh: &Mesh, scale: f32, offset: [f32; 2]) {
        for tri in mesh.indices.chunks_exact(3) {
            let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let p = v.map(|v| [v.pos.x * scale + offset[0], v.pos.y * scale + offset[1]]);
            let c = v.map(|v| to_f(v.color));
            let minx = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let maxx = p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil().min(self.w as f32 - 1.0);
            let miny = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let maxy = p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil().min(self.h as f32 - 1.0);
            if maxx < 0.0 || maxy < 0.0 {
                continue;
            }
            let det = (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
            if det.abs() < 1e-9 {
                continue;
            }
            for y in miny..=maxy as usize {
                for x in minx..=maxx as usize {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let a = ((p[1][1] - p[2][1]) * (px - p[2][0]) + (p[2][0] - p[1][0]) * (py - p[2][1])) / det;
                    let b = ((p[2][1] - p[0][1]) * (px - p[2][0]) + (p[0][0] - p[2][0]) * (py - p[2][1])) / det;
                    let g = 1.0 - a - b;
                    if a < -1e-4 || b < -1e-4 || g < -1e-4 {
                        continue;
                    }
                    let mut s = [0.0; 4];
                    for k in 0..4 {
                        s[k] = c[0][k] * a + c[1][k] * b + c[2][k] * g;
                    }
                    let d = &mut self.px[y * self.w + x];
                    for k in 0..4 {
                        d[k] = s[k] + d[k] * (1.0 - s[3]);
                    }
                }
            }
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let mut img = image::RgbaImage::new(self.w as u32, self.h as u32);
        for (i, p) in self.px.iter().enumerate() {
            let a = p[3].clamp(0.0, 1.0);
            let un = |v: f32| if a > 0.0 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
            img.put_pixel(
                (i % self.w) as u32,
                (i / self.w) as u32,
                image::Rgba([(un(p[0]) * 255.0) as u8, (un(p[1]) * 255.0) as u8, (un(p[2]) * 255.0) as u8, (a * 255.0) as u8]),
            );
        }
        img.save(path).map_err(|e| e.to_string())
    }
}

fn to_f(c: Color32) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
}

/// Puts a world into `act` and lets it play for `secs`.
pub fn staged(act: Act, secs: f32, cfg: &RaccoonSettings) -> World {
    let mut c = cfg.clone();
    c.enabled = true;
    c.speech_bubbles = false;
    let mut w = World::new(&c, 7);
    let stage = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
    w.set_stage(stage, stage);
    w.placed = true;
    w.pos = pos2(700.0, 900.0);
    w.support = Some(super::FLOOR);
    w.force(act);
    if act == Act::Dragged {
        w.force(Act::Idle);
        w.act = Act::Dragged;
    }
    let dt = 1.0 / 60.0;
    let mut t = 0.0;
    while t < secs {
        let keep = w.act;
        w.tick(&Inputs { dt, vitals: Vitals { cpu: 30.0, ..Default::default() }, hour: 12, ..Default::default() });
        // Hold the activity even if it would naturally end.
        if w.act != keep && !act.airborne() {
            w.act = keep;
        }
        if act.airborne() && w.act != act {
            w.act = act;
        }
        t += dt;
    }
    w
}

/// Renders a contact sheet of every activity.
pub fn sheet(path: &std::path::Path, cfg: &RaccoonSettings) -> Result<(), String> {
    // RACCOON_SHEET_ONLY=0,4,17 renders just those activities (by index).
    let only: Option<Vec<usize>> = std::env::var("RACCOON_SHEET_ONLY")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let list: Vec<Act> = acts::all()
        .iter()
        .enumerate()
        .filter(|(i, _)| only.as_ref().is_none_or(|o| o.contains(i)))
        .map(|(_, d)| d.act)
        .collect();
    let cols = list.len().clamp(1, 10);
    let rows = list.len().div_ceil(cols);
    let (cw, ch) = (150.0, 120.0);
    let ppp = 2.0;
    let mut r = Raster::new((cols as f32 * cw * ppp) as usize, (rows as f32 * ch * ppp) as usize, Color32::from_rgb(70, 82, 104));
    let pal = cfg.palette();
    for (i, act) in list.iter().enumerate() {
        let w = staged(*act, 0.7, cfg);
        let mut xf = w.xform();
        let cell = pos2((i % cols) as f32 * cw, (i / cols) as f32 * ch);
        xf.origin = cell + vec2(cw * 0.5, ch * 0.82);
        let mut cv = Canvas::new(ppp);
        let look = Look { pal: &pal, outline_width: cfg.outline_width, alpha: 1.0 };
        draw_pose(&mut cv, &w.pose, xf, &look);
        r.draw(&cv.mesh, ppp, [0.0, 0.0]);
        println!("{:>2}: {}", i, act.label());
    }
    r.save(path)
}
