//! Render one frame to a PPM image (no window needed).
//!   cargo run --release --bin snapshot -- out.ppm [width height seed]
//! Searches the maze for a viewpoint with lamps and props in view, so the
//! image shows lighting and sprites rather than a blank corridor.
use raycast_rs::{engine::Game, map::Map, to_ppm};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "frame.ppm".into());
    let w: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(960);
    let h: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(540);
    let seed: u32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(7);
    let map = Map::maze(12, 12, seed);
    let (mw, mh) = (map.w, map.h);
    let mut g = Game::new(map, 160, 90);
    let mut best = (0.0f32, 0.0, 0.0, 0.0);
    for y in 1..mh - 1 {
        for x in 1..mw - 1 {
            if g.map.get(x as i32, y as i32) != 0 {
                continue;
            }
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            if g.map.sprites.iter().any(|s| (s.x - cx).powi(2) + (s.y - cy).powi(2) < 4.0) {
                continue; // a prop within 2 cells would fill the frame
            }
            for k in 0..16 {
                g.x = x as f32 + 0.5;
                g.y = y as f32 + 0.5;
                g.angle = k as f32 * std::f32::consts::TAU / 16.0;
                g.render();
                // prefer several props, some light, and depth (not staring at a wall)
                let mid = g.zbuf[g.w / 2];
                let score = g.stats.sprites_drawn as f32 * 3.0 + g.stats.lights_active as f32 + mid.min(6.0);
                if score > best.0 {
                    best = (score, g.x, g.y, g.angle);
                }
            }
        }
    }
    g.resize(w, h);
    g.x = best.1;
    g.y = best.2;
    g.angle = best.3;
    g.time = 1.3;
    g.render();
    std::fs::write(&out, to_ppm(&g.fb, w, h)).expect("write");
    println!("wrote {out} ({w}x{h}) from ({:.1}, {:.1}) angle {:.2}: rays {}, dda steps {}, sprites drawn {}, lights {}",
        g.x, g.y, g.angle, g.stats.rays, g.stats.dda_steps, g.stats.sprites_drawn, g.stats.lights_active);
}
