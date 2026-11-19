//! Native render benchmark: cargo run --release --bin bench
use raycast_rs::engine::{Game, FWD, TURN_R};
use raycast_rs::map::Map;
use std::time::Instant;

fn run(w: usize, h: usize, setup: impl Fn(&mut Game)) -> (f64, u32) {
    let mut g = Game::new(Map::maze(12, 12, 7), w, h);
    setup(&mut g);
    for _ in 0..30 {
        g.update(1.0 / 60.0, FWD, 0.0);
        g.render();
    }
    let n = 300;
    let mut steps = 0;
    let t0 = Instant::now();
    for i in 0..n {
        g.update(1.0 / 60.0, if i % 4 == 0 { FWD | TURN_R } else { FWD }, 0.0);
        g.render();
        steps += g.stats.dda_steps;
    }
    (t0.elapsed().as_secs_f64() * 1000.0 / n as f64, steps / n)
}

fn main() {
    println!("{:<34} {:>9} {:>8} {:>10}", "configuration", "ms/frame", "fps", "dda/frame");
    for (w, h) in [(320, 180), (640, 360), (960, 540), (1920, 1080)] {
        let (ms, steps) = run(w, h, |_| {});
        println!("{:<34} {:>9.3} {:>8.0} {:>10}", format!("{w}x{h} full (lights, fog, textures)"), ms, 1000.0 / ms, steps);
    }
    let cases: [(&str, fn(&mut Game)); 4] = [
        ("640x360 no lighting", |g| g.opt.lighting = false),
        ("640x360 no fog", |g| g.opt.fog = false),
        ("640x360 flat colours (no textures)", |g| g.opt.textures = false),
        ("640x360 no minimap", |g| g.opt.minimap = false),
    ];
    for (name, f) in cases {
        let (ms, steps) = run(640, 360, f);
        println!("{:<34} {:>9.3} {:>8.0} {:>10}", name, ms, 1000.0 / ms, steps);
    }
}
