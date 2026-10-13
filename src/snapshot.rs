//! Headless rendering of the real UI to a PNG, for design work and docs.
//!
//! `eco-view --screenshot <page> <out.png> [--theme <name>] [--size WxH]
//!  [--pointer X,Y] [--palette] [--demo]`
//!
//! It drives the actual `EcoView::frame` code through a plain egui context,
//! tessellates the output and rasterises it in software, fonts included, so
//! nothing needs a window or the Screen Recording permission.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, Event, Pos2, RawInput, Rect, TextureId, ViewportId, ViewportInfo, epaint::ImageData, pos2, vec2};

use crate::app::{EcoView, Page};
use crate::metrics::{self, Metrics};
use crate::settings::{Settings, ThemeKind};

struct Tex {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Tex {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u * self.w as f32 - 0.5).clamp(0.0, self.w as f32 - 1.0);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, self.h as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let g = |x: usize, y: usize| self.px[y * self.w + x];
        let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy;
        }
        o
    }
}

fn f(c: Color32) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
}

pub fn run(args: &[String]) -> Result<String, String> {
    // "settings/appearance" opens a settings section.
    let (page_name, section) = args.first().map(|p| p.split_once('/').unwrap_or((p.as_str(), ""))).unwrap_or(("overview", ""));
    let page = Page::from_name(page_name).unwrap_or(Page::Overview);
    let out = args.get(1).cloned().unwrap_or_else(|| "screenshot.png".into());
    let mut theme = None;
    let mut size = (1440.0_f32, 900.0_f32);
    let mut pointer: Option<Pos2> = None;
    let mut palette = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--theme" => {
                theme = args.get(i + 1).and_then(|t| ThemeKind::ALL.into_iter().find(|k| k.label().eq_ignore_ascii_case(t)));
                i += 1;
            }
            "--size" => {
                if let Some((w, h)) = args.get(i + 1).and_then(|s| s.split_once('x')) {
                    size = (w.parse().unwrap_or(size.0), h.parse().unwrap_or(size.1));
                }
                i += 1;
            }
            "--pointer" => {
                if let Some((x, y)) = args.get(i + 1).and_then(|s| s.split_once(',')) {
                    pointer = Some(pos2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0)));
                }
                i += 1;
            }
            "--palette" => palette = true,
            _ => {}
        }
        i += 1;
    }

    let mut settings = Settings::load();
    settings.companion.enabled = false;
    settings.sound_enabled = false;
    if let Some(t) = theme {
        settings.theme = t;
    }
    let metrics = Arc::new(Mutex::new(sample_metrics()));
    let mut app = EcoView::build(settings, metrics, true);
    app.page = page;
    if let Some(s) = crate::app::SettingsSection::from_name(section) {
        app.section = s;
    }
    app.palette_open = palette;

    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let ppp = 2.0;
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(size.0, size.1));
    let mut textures: HashMap<TextureId, Tex> = HashMap::new();
    let mut last = None;
    // Enough frames for fonts to load and transitions to settle.
    for frame in 0..40 {
        let mut raw = RawInput { screen_rect: Some(screen), time: Some(frame as f64 * 0.05), focused: true, ..Default::default() };
        raw.viewports.insert(ViewportId::ROOT, ViewportInfo { native_pixels_per_point: Some(ppp), ..Default::default() });
        if let Some(p) = pointer {
            raw.events.push(Event::PointerMoved(p));
        }
        let full = ctx.run(raw, |ctx| app.frame(ctx));
        for (id, delta) in &full.textures_delta.set {
            let (w, h, px): (usize, usize, Vec<[f32; 4]>) = match &delta.image {
                ImageData::Color(img) => (img.size[0], img.size[1], img.pixels.iter().map(|c| f(*c)).collect()),
                ImageData::Font(img) => (img.size[0], img.size[1], img.srgba_pixels(None).map(f).collect()),
            };
            match delta.pos {
                None => {
                    textures.insert(*id, Tex { w, h, px });
                }
                Some([x0, y0]) => {
                    if let Some(t) = textures.get_mut(id) {
                        for y in 0..h {
                            for x in 0..w {
                                if x0 + x < t.w && y0 + y < t.h {
                                    t.px[(y0 + y) * t.w + x0 + x] = px[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        last = Some(full);
    }
    let full = last.ok_or("no frames")?;
    let prims = ctx.tessellate(full.shapes, full.pixels_per_point);
    let (w, h) = ((size.0 * ppp) as usize, (size.1 * ppp) as usize);
    let mut img = vec![[0.0f32; 4]; w * h];
    for cp in prims {
        let egui::epaint::Primitive::Mesh(mesh) = cp.primitive else { continue };
        let clip = cp.clip_rect;
        let (cx0, cy0) = ((clip.min.x * ppp).max(0.0) as usize, (clip.min.y * ppp).max(0.0) as usize);
        let (cx1, cy1) = (((clip.max.x * ppp).ceil() as usize).min(w), ((clip.max.y * ppp).ceil() as usize).min(h));
        let tex = textures.get(&mesh.texture_id);
        for tri in mesh.indices.chunks_exact(3) {
            let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let p = v.map(|v| [v.pos.x * ppp, v.pos.y * ppp]);
            let det = (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
            if det.abs() < 1e-9 {
                continue;
            }
            let minx = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize).max(cx0);
            let maxx = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(cx1);
            let miny = (p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize).max(cy0);
            let maxy = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(cy1);
            let cols = v.map(|v| f(v.color));
            for y in miny..maxy {
                for x in minx..maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let a = ((p[1][1] - p[2][1]) * (px - p[2][0]) + (p[2][0] - p[1][0]) * (py - p[2][1])) / det;
                    let b = ((p[2][1] - p[0][1]) * (px - p[2][0]) + (p[0][0] - p[2][0]) * (py - p[2][1])) / det;
                    let g = 1.0 - a - b;
                    if a < -1e-5 || b < -1e-5 || g < -1e-5 {
                        continue;
                    }
                    let mut c = [0.0; 4];
                    for k in 0..4 {
                        c[k] = cols[0][k] * a + cols[1][k] * b + cols[2][k] * g;
                    }
                    if let Some(t) = tex {
                        let u = v[0].uv.x * a + v[1].uv.x * b + v[2].uv.x * g;
                        let vv = v[0].uv.y * a + v[1].uv.y * b + v[2].uv.y * g;
                        let s = t.sample(u, vv);
                        for k in 0..4 {
                            c[k] *= s[k];
                        }
                    }
                    let d = &mut img[y * w + x];
                    for k in 0..4 {
                        d[k] = c[k] + d[k] * (1.0 - c[3]);
                    }
                }
            }
        }
    }
    let mut png = image::RgbaImage::new(w as u32, h as u32);
    for (i, p) in img.iter().enumerate() {
        let a = p[3].clamp(0.0, 1.0);
        let un = |v: f32| if a > 0.0 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
        png.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([(un(p[0]) * 255.0) as u8, (un(p[1]) * 255.0) as u8, (un(p[2]) * 255.0) as u8, (a * 255.0) as u8]));
    }
    png.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// One real sample of this Mac, plus a plausible recent history so charts
/// have something to show.
fn sample_metrics() -> Metrics {
    let shared = Arc::new(Mutex::new(Metrics::default()));
    metrics::spawn(shared.clone(), Arc::new(Mutex::new(400)));
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if shared.lock().map(|m| m.history.samples >= 3).unwrap_or(false) {
            break;
        }
    }
    let mut m = shared.lock().map(|m| m.clone()).unwrap_or_default();
    let n = metrics::HISTORY;
    let wave = |i: usize, base: f32, amp: f32, period: f32, seed: f32| {
        let t = i as f32;
        let noise = ((t * 12.9898 + seed).sin() * 43758.545).fract().abs();
        (base + (t / period * std::f32::consts::TAU + seed).sin() * amp + noise * amp * 0.6).max(0.0)
    };
    let s = &m.snap;
    let cpu = s.cpu_global.max(8.0);
    let mem = metrics::pct(s.mem_used, s.mem_total).max(20.0);
    m.history.cpu = (0..n).map(|i| wave(i, cpu, 9.0, 37.0, 1.0).min(100.0)).collect();
    m.history.ram = (0..n).map(|i| wave(i, mem, 1.5, 120.0, 2.0).min(100.0)).collect();
    m.history.gpu = (0..n).map(|i| wave(i, s.gpu.load.unwrap_or(12.0).max(6.0), 6.0, 23.0, 3.0).min(100.0)).collect();
    m.history.net_down = (0..n).map(|i| wave(i, 180_000.0, 140_000.0, 17.0, 4.0)).collect();
    m.history.net_up = (0..n).map(|i| wave(i, 40_000.0, 30_000.0, 13.0, 5.0)).collect();
    m.history.temp = (0..n).map(|i| wave(i, s.temperature.as_ref().map(|t| t.celsius).unwrap_or(54.0), 3.0, 80.0, 6.0)).collect();
    m.history.battery = (0..n).map(|i| s.battery.as_ref().map(|b| b.percent).unwrap_or(80.0) + (n - i) as f32 * 0.01).collect();
    m.history.audio = (0..n).map(|_| 50.0).collect();
    if let Some(l) = m.history.cpu.back_mut() {
        *l = cpu;
    }
    m.history.samples = n as u64;
    m
}
