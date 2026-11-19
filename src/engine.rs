//! Player, movement/collision, and the per-frame software renderer:
//! floor/ceiling casting -> textured walls (+ z-buffer) -> depth-sorted
//! sprites -> minimap, with dynamic point lights and distance fog.

use crate::map::{Light, Map, SpriteKind};
use crate::ray::cast;
use crate::tex::{rgba, Textures, T};

pub const FWD: u32 = 1;
pub const BACK: u32 = 2;
pub const STRAFE_L: u32 = 4;
pub const STRAFE_R: u32 = 8;
pub const TURN_L: u32 = 16;
pub const TURN_R: u32 = 32;
pub const RUN: u32 = 64;

pub const PLAYER_RADIUS: f32 = 0.22;
const FOV_PLANE: f32 = 0.66; // tan(fov/2): ~66 degree horizontal FOV
const AMBIENT: [f32; 3] = [0.07, 0.07, 0.09];
const FOG: [f32; 3] = [8.0, 9.0, 12.0];

#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub rays: u32,
    pub dda_steps: u32,
    pub sprites_drawn: u32,
    pub lights_active: u32,
}

pub struct Options {
    pub minimap: bool,
    pub lighting: bool,
    pub fog: bool,
    pub torch: bool,
    pub textures: bool,
}

pub struct Game {
    pub map: Map,
    pub tex: Textures,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub w: usize,
    pub h: usize,
    pub fb: Vec<u32>,
    pub zbuf: Vec<f32>,
    pub time: f32,
    pub won: bool,
    pub opt: Options,
    pub stats: Stats,
    active: Vec<Light>,
}

impl Game {
    pub fn new(map: Map, w: usize, h: usize) -> Game {
        let (x, y, angle) = map.start;
        Game {
            map, tex: Textures::generate(), x, y, angle, w, h,
            fb: vec![0; w * h], zbuf: vec![0.0; w], time: 0.0, won: false,
            opt: Options { minimap: true, lighting: true, fog: true, torch: true, textures: true },
            stats: Stats::default(), active: Vec::with_capacity(16),
        }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        self.w = w.max(8);
        self.h = h.max(8);
        self.fb = vec![0; self.w * self.h];
        self.zbuf = vec![0.0; self.w];
    }

    // ---------------------------------------------------------------- update
    /// Axis-separated collision: try the x move, then the y move, each
    /// checked with the player's radius. Blocked axes are dropped, which
    /// produces wall sliding instead of sticking.
    pub fn try_move(&mut self, dx: f32, dy: f32) {
        let r = PLAYER_RADIUS;
        let nx = self.x + dx;
        if !self.blocked(nx, self.y, r) {
            self.x = nx;
        }
        let ny = self.y + dy;
        if !self.blocked(self.x, ny, r) {
            self.y = ny;
        }
    }

    pub fn blocked(&self, x: f32, y: f32, r: f32) -> bool {
        let m = &self.map;
        if m.solid(x - r, y - r) || m.solid(x + r, y - r) || m.solid(x - r, y + r) || m.solid(x + r, y + r) {
            return true;
        }
        // solid props (pillars, barrels) block too
        m.sprites.iter().any(|s| {
            matches!(s.kind, SpriteKind::Pillar | SpriteKind::Barrel)
                && (s.x - x).powi(2) + (s.y - y).powi(2) < (r + 0.28).powi(2)
        })
    }

    pub fn update(&mut self, dt: f32, input: u32, mouse_dx: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let turn = 2.6 * dt;
        if input & TURN_L != 0 { self.angle -= turn; }
        if input & TURN_R != 0 { self.angle += turn; }
        self.angle += mouse_dx * 0.0025;
        let speed = if input & RUN != 0 { 4.2 } else { 2.6 } * dt;
        let (s, c) = self.angle.sin_cos();
        let (mut mx, mut my) = (0.0, 0.0);
        if input & FWD != 0 { mx += c; my += s; }
        if input & BACK != 0 { mx -= c; my -= s; }
        if input & STRAFE_L != 0 { mx += s; my -= c; }
        if input & STRAFE_R != 0 { mx -= s; my += c; }
        let len = (mx * mx + my * my).sqrt();
        if len > 0.0 {
            // sub-step so a fast frame can't tunnel through a thin corner
            let steps = ((speed / 0.1).ceil() as usize).max(1);
            for _ in 0..steps {
                self.try_move(mx / len * speed / steps as f32, my / len * speed / steps as f32);
            }
        }
        for sp in &self.map.sprites {
            if sp.kind == SpriteKind::Exit && (sp.x - self.x).powi(2) + (sp.y - self.y).powi(2) < 0.36 {
                self.won = true;
            }
        }
    }

    // ---------------------------------------------------------------- lighting
    fn gather_lights(&mut self) {
        self.active.clear();
        if !self.opt.lighting {
            return;
        }
        if self.opt.torch {
            let f = 1.0 + 0.06 * (self.time * 13.0).sin() + 0.04 * (self.time * 7.3).sin();
            self.active.push(Light { x: self.x, y: self.y, color: [1.0 * f, 0.86 * f, 0.66 * f], radius: 4.2, flicker: 0.0 });
        }
        for l in &self.map.lights {
            let d2 = (l.x - self.x).powi(2) + (l.y - self.y).powi(2);
            if d2 > 18.0 * 18.0 {
                continue; // too far to matter on screen
            }
            let mut l2 = *l;
            if l.flicker > 0.0 {
                let p = l.x * 12.9898 + l.y * 78.233;
                let k = 1.0 + l.flicker * ((self.time * 9.0 + p).sin() * 0.6 + (self.time * 23.0 + p * 1.7).sin() * 0.4);
                l2.color = [l.color[0] * k, l.color[1] * k, l.color[2] * k];
            }
            self.active.push(l2);
        }
        // keep the nearest 12
        let (px, py) = (self.x, self.y);
        self.active.sort_by(|a, b| {
            let da = (a.x - px).powi(2) + (a.y - py).powi(2);
            let db = (b.x - px).powi(2) + (b.y - py).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.active.truncate(12);
        self.stats.lights_active = self.active.len() as u32;
    }

    /// Light arriving at world point (x, y): ambient + sum of point lights with
    /// a smooth, finite falloff (1 - d^2/r^2)^2, so culling by radius is exact.
    /// Lights are not occluded by walls (no shadows).
    #[inline]
    pub fn light_at(&self, x: f32, y: f32) -> [f32; 3] {
        if !self.opt.lighting {
            return [1.0, 1.0, 1.0];
        }
        let mut c = AMBIENT;
        for l in &self.active {
            let d2 = (l.x - x).powi(2) + (l.y - y).powi(2);
            let r2 = l.radius * l.radius;
            if d2 < r2 {
                let f = 1.0 - d2 / r2;
                let k = f * f * 1.1;
                c[0] += l.color[0] * k;
                c[1] += l.color[1] * k;
                c[2] += l.color[2] * k;
            }
        }
        c
    }

    #[inline]
    fn fog_k(&self, dist: f32) -> f32 {
        if self.opt.fog { (-dist * 0.085).exp() } else { 1.0 }
    }

    #[inline]
    fn lit(texel: u32, l: [f32; 3], fog: f32) -> u32 {
        let r = (texel & 0xff) as f32;
        let g = ((texel >> 8) & 0xff) as f32;
        let b = ((texel >> 16) & 0xff) as f32;
        let r = (r * l[0] * fog + FOG[0] * (1.0 - fog)).min(255.0);
        let g = (g * l[1] * fog + FOG[1] * (1.0 - fog)).min(255.0);
        let b = (b * l[2] * fog + FOG[2] * (1.0 - fog)).min(255.0);
        rgba(r as u8, g as u8, b as u8, 255)
    }

    // ---------------------------------------------------------------- render
    pub fn render(&mut self) {
        self.stats = Stats::default();
        self.gather_lights();
        let (w, h) = (self.w, self.h);
        let (dir_x, dir_y) = (self.angle.cos(), self.angle.sin());
        // camera plane is perpendicular to dir, scaled for the FOV
        let (plane_x, plane_y) = (-dir_y * FOV_PLANE, dir_x * FOV_PLANE);
        self.floor_ceiling(dir_x, dir_y, plane_x, plane_y);
        self.walls(dir_x, dir_y, plane_x, plane_y);
        self.sprites(dir_x, dir_y, plane_x, plane_y);
        if self.opt.minimap {
            self.minimap(dir_x, dir_y, plane_x, plane_y);
        }
        let _ = (w, h);
    }

    /// Horizontal-scanline floor casting. For screen row y below the horizon,
    /// the floor point seen there is at distance rowDist = (h/2)/(y - h/2)
    /// (camera height = half a wall). Along the row, the world position moves
    /// linearly from the leftmost ray to the rightmost: one add per pixel.
    fn floor_ceiling(&mut self, dx: f32, dy: f32, plx: f32, ply: f32) {
        let (w, h) = (self.w, self.h);
        let half = h as f32 / 2.0;
        let (r0x, r0y) = (dx - plx, dy - ply);
        let (r1x, r1y) = (dx + plx, dy + ply);
        let tm = (T - 1) as i32;
        for y in (h / 2 + 1)..h {
            let p = y as f32 - half;
            let row_dist = half / p;
            let sx = row_dist * (r1x - r0x) / w as f32;
            let sy = row_dist * (r1y - r0y) / w as f32;
            let mut fx = self.x + row_dist * r0x;
            let mut fy = self.y + row_dist * r0y;
            let fog = self.fog_k(row_dist);
            let cy = h - y - 1;
            // lighting sampled every 4 px and held: floor light varies slowly
            let mut l = [1.0f32; 3];
            for x in 0..w {
                if x & 3 == 0 {
                    l = self.light_at(fx, fy);
                }
                let tx = ((fx * T as f32) as i32 & tm) as usize;
                let ty = ((fy * T as f32) as i32 & tm) as usize;
                let (ft, ct) = if self.opt.textures {
                    (self.tex.floor[ty * T + tx], self.tex.ceil[ty * T + tx])
                } else {
                    (rgba(90, 85, 78, 255), rgba(50, 45, 40, 255))
                };
                self.fb[y * w + x] = Self::lit(ft, l, fog);
                // ceiling is a little darker: lights hang below it
                self.fb[cy * w + x] = Self::lit(ct, [l[0] * 0.8, l[1] * 0.8, l[2] * 0.8], fog);
                fx += sx;
                fy += sy;
            }
        }
        // the horizon row itself
        let mid = h / 2;
        for x in 0..w {
            self.fb[mid * w + x] = rgba(FOG[0] as u8, FOG[1] as u8, FOG[2] as u8, 255);
        }
    }

    fn walls(&mut self, dx: f32, dy: f32, plx: f32, ply: f32) {
        let (w, h) = (self.w, self.h);
        let hf = h as f32;
        for x in 0..w {
            let cam = 2.0 * x as f32 / w as f32 - 1.0;
            let (rx, ry) = (dx + plx * cam, dy + ply * cam);
            self.stats.rays += 1;
            let hit = match cast(&self.map, self.x, self.y, rx, ry, 512) {
                Some(hit) => hit,
                None => {
                    self.zbuf[x] = f32::INFINITY;
                    continue;
                }
            };
            self.stats.dda_steps += hit.steps;
            let dist = hit.dist.max(1e-4);
            self.zbuf[x] = dist;
            let line_h = hf / dist;
            let start = (hf - line_h) / 2.0;
            let y0 = start.max(0.0) as usize;
            let y1 = ((hf + line_h) / 2.0).min(hf) as usize;
            let tex = &self.tex.walls[(hit.wall as usize - 1) % self.tex.walls.len()];
            let tx = (hit.wall_x * T as f32) as usize;
            let step = T as f32 / line_h;
            let mut tpos = (y0 as f32 - start) * step;
            // light at the hit point, nudged off the wall toward the viewer
            let (hx, hy) = (self.x + rx * dist * 0.995, self.y + ry * dist * 0.995);
            let mut l = self.light_at(hx, hy);
            if hit.side == 1 {
                l = [l[0] * 0.78, l[1] * 0.78, l[2] * 0.78]; // cheap directional cue
            }
            let fog = self.fog_k(dist);
            for y in y0..y1 {
                let ty = (tpos as usize).min(T - 1);
                tpos += step;
                let texel = if self.opt.textures { tex[ty * T + tx] } else { rgba(140, 140, 140, 255) };
                self.fb[y * w + x] = Self::lit(texel, l, fog);
            }
        }
    }

    /// Billboard sprites: transform into camera space with the inverse of the
    /// [plane dir] matrix, sort far-to-near, draw column stripes that are in
    /// front of the wall z-buffer, skipping transparent texels.
    fn sprites(&mut self, dx: f32, dy: f32, plx: f32, ply: f32) {
        let (w, h) = (self.w, self.h);
        let inv_det = 1.0 / (plx * dy - dx * ply);
        let mut order: Vec<(f32, usize)> = self.map.sprites.iter().enumerate()
            .map(|(i, s)| ((s.x - self.x).powi(2) + (s.y - self.y).powi(2), i))
            .collect();
        order.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for &(_, i) in &order {
            let s = self.map.sprites[i];
            let (sx, sy) = (s.x - self.x, s.y - self.y);
            let tx = inv_det * (dy * sx - dx * sy);
            let depth = inv_det * (-ply * sx + plx * sy);
            if depth <= 0.05 {
                continue; // behind the camera
            }
            let screen_x = (w as f32 / 2.0) * (1.0 + tx / depth);
            let size = h as f32 / depth;
            let x0 = (screen_x - size / 2.0).max(0.0) as i32;
            let x1 = (screen_x + size / 2.0).min(w as f32) as i32;
            if x1 <= x0 {
                continue;
            }
            let y_start = h as f32 / 2.0 - size / 2.0;
            let y0 = y_start.max(0.0) as usize;
            let y1 = (h as f32 / 2.0 + size / 2.0).min(h as f32) as usize;
            let kind = s.kind as usize;
            let emissive = matches!(s.kind, SpriteKind::Lamp | SpriteKind::Exit);
            let l = if emissive { [1.1, 1.1, 1.1] } else { self.light_at(s.x, s.y) };
            let fog = self.fog_k(depth);
            let mut drew = false;
            for stripe in x0..x1 {
                let st = stripe as usize;
                if depth >= self.zbuf[st] {
                    continue;
                }
                let tex_x = (((stripe as f32 - (screen_x - size / 2.0)) / size) * T as f32) as usize;
                let tex_x = tex_x.min(T - 1);
                for y in y0..y1 {
                    let tex_y = ((((y as f32 - y_start) / size) * T as f32) as usize).min(T - 1);
                    let texel = self.tex.sprites[kind][tex_y * T + tex_x];
                    if texel >> 24 == 0 {
                        continue;
                    }
                    self.fb[y * w + st] = Self::lit(texel, l, fog);
                    drew = true;
                }
            }
            if drew {
                self.stats.sprites_drawn += 1;
            }
        }
    }

    fn put(&mut self, x: i32, y: i32, c: u32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.fb[y as usize * self.w + x as usize] = c;
        }
    }

    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: u32) {
        // Bresenham
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            self.put(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }

    fn minimap(&mut self, dx: f32, dy: f32, plx: f32, ply: f32) {
        let (mw, mh) = (self.map.w, self.map.h);
        let cell = ((self.h / 3) / mh.max(1)).clamp(2, 8) as i32;
        let (ox, oy) = (6, 6);
        let wall_cols = [rgba(150, 70, 52, 255), rgba(125, 128, 135, 255), rgba(130, 90, 55, 255), rgba(100, 108, 120, 255)];
        for y in 0..mh {
            for x in 0..mw {
                let v = self.map.cells[y * mw + x];
                let c = if v == 0 { rgba(22, 24, 28, 255) } else { wall_cols[(v as usize - 1) % 4] };
                for py in 0..cell {
                    for px in 0..cell {
                        self.put(ox + x as i32 * cell + px, oy + y as i32 * cell + py, c);
                    }
                }
            }
        }
        let to_px = |wx: f32, wy: f32| (ox + (wx * cell as f32) as i32, oy + (wy * cell as f32) as i32);
        // view frustum: a few rays out to their wall hits
        let (px, py) = to_px(self.x, self.y);
        for k in 0..=8 {
            let cam = k as f32 / 4.0 - 1.0;
            let (rx, ry) = (dx + plx * cam, dy + ply * cam);
            if let Some(hit) = cast(&self.map, self.x, self.y, rx, ry, 512) {
                let (ex, ey) = to_px(self.x + rx * hit.dist, self.y + ry * hit.dist);
                self.line(px, py, ex, ey, rgba(230, 200, 90, 255));
            }
        }
        let sprites: Vec<(f32, f32, SpriteKind)> = self.map.sprites.iter().map(|s| (s.x, s.y, s.kind)).collect();
        for (sx, sy, kind) in sprites {
            let (qx, qy) = to_px(sx, sy);
            let c = match kind {
                SpriteKind::Exit => rgba(90, 255, 140, 255),
                SpriteKind::Lamp => rgba(255, 200, 120, 255),
                _ => rgba(150, 150, 150, 255),
            };
            for (ax, ay) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                self.put(qx + ax, qy + ay, c);
            }
        }
        for ax in -1..=1 {
            for ay in -1..=1 {
                self.put(px + ax, py + ay, rgba(255, 255, 255, 255));
            }
        }
    }
}
