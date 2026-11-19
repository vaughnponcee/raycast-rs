use raycast_rs::engine::{Game, FWD, PLAYER_RADIUS, TURN_R};
use raycast_rs::map::{Map, Rng};
use raycast_rs::ray::cast;

const BOX: &str = "\
##########
#........#
#........#
#........#
#........#
##########";

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn axis_aligned_rays_hit_exact_distances() {
    let m = Map::parse(BOX);
    // from (2.5, 2.5): east wall face at x=9 -> 6.5; west face at x=1 -> 1.5
    let e = cast(&m, 2.5, 2.5, 1.0, 0.0, 64).unwrap();
    assert!(close(e.dist, 6.5, 1e-5), "{e:?}");
    assert_eq!((e.side, e.cell_x, e.cell_y), (0, 9, 2));
    let wv = cast(&m, 2.5, 2.5, -1.0, 0.0, 64).unwrap();
    assert!(close(wv.dist, 1.5, 1e-5));
    let s = cast(&m, 2.5, 2.5, 0.0, 1.0, 64).unwrap();
    assert!(close(s.dist, 2.5, 1e-5));
    assert_eq!((s.side, s.cell_y), (1, 5));
    let n = cast(&m, 2.5, 2.5, 0.0, -1.0, 64).unwrap();
    assert!(close(n.dist, 1.5, 1e-5));
}

#[test]
fn distance_scales_with_ray_length() {
    // dist is the ray parameter t: doubling the direction halves t
    let m = Map::parse(BOX);
    let a = cast(&m, 2.5, 2.5, 1.0, 0.0, 64).unwrap();
    let b = cast(&m, 2.5, 2.5, 2.0, 0.0, 64).unwrap();
    assert!(close(a.dist, 2.0 * b.dist, 1e-5));
}

#[test]
fn diagonal_ray_through_corner_and_wall_x() {
    let m = Map::parse(BOX);
    // 45 degrees from (1.25, 1.5): hits the east wall x=9 at y = 1.5 + 7.75 > 5 -> hits south wall y=5 first at x = 1.25+3.5 = 4.75
    let h = cast(&m, 1.25, 1.5, 1.0, 1.0, 64).unwrap();
    assert_eq!(h.side, 1);
    assert_eq!(h.cell_y, 5);
    assert!(close(h.dist, 3.5, 1e-5), "{h:?}");
    // wall_x = fractional x of the hit point (4.75 -> .75), mirrored for +y rays
    assert!(close(h.wall_x, 1.0 - 0.75, 1e-5), "{h:?}");
}

#[test]
fn hit_point_lies_on_the_hit_cell_boundary_fuzz() {
    let m = Map::maze(10, 10, 3);
    let mut r = Rng::new(99);
    let mut checked = 0;
    while checked < 5000 {
        let (px, py) = (1.0 + r.f32() * (m.w as f32 - 2.0), 1.0 + r.f32() * (m.h as f32 - 2.0));
        if m.solid(px, py) {
            continue;
        }
        let a = r.f32() * std::f32::consts::TAU;
        let (dx, dy) = (a.cos(), a.sin());
        let h = cast(&m, px, py, dx, dy, 4096).expect("closed map: every ray must hit");
        let (hx, hy) = (px + dx * h.dist, py + dy * h.dist);
        assert_ne!(m.get(h.cell_x, h.cell_y), 0, "hit cell must be solid");
        // the hit point is on the face of the hit cell that the ray entered
        if h.side == 0 {
            let face = if dx > 0.0 { h.cell_x as f32 } else { h.cell_x as f32 + 1.0 };
            assert!(close(hx, face, 2e-3), "x-side: {hx} vs {face}");
            assert!(hy >= h.cell_y as f32 - 2e-3 && hy <= h.cell_y as f32 + 1.0 + 2e-3);
        } else {
            let face = if dy > 0.0 { h.cell_y as f32 } else { h.cell_y as f32 + 1.0 };
            assert!(close(hy, face, 2e-3), "y-side: {hy} vs {face}");
            assert!(hx >= h.cell_x as f32 - 2e-3 && hx <= h.cell_x as f32 + 1.0 + 2e-3);
        }
        // no solid cell between the origin and the hit (sampled along the ray)
        for k in 1..64 {
            let t = h.dist * k as f32 / 64.0 * 0.98;
            assert!(!m.solid(px + dx * t, py + dy * t), "ray passed through a wall");
        }
        assert!((0.0..1.0).contains(&h.wall_x));
        checked += 1;
    }
}

#[test]
fn perpendicular_distance_removes_fisheye() {
    // Facing a flat wall, every column's camera ray must report the same
    // perpendicular distance (the wall is a plane at constant depth).
    // room tall enough (13 rows) that the whole 66 degree FOV lands on the east wall
    let tall: String = std::iter::once("##########".to_string())
        .chain((0..11).map(|_| "#........#".to_string()))
        .chain(std::iter::once("##########".to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    let m = Map::parse(&tall);
    let (dx, dy) = (1.0f32, 0.0f32);
    let (plx, ply) = (0.0f32, 0.66f32);
    let mut dists = vec![];
    for x in 0..40 {
        let cam = 2.0 * x as f32 / 40.0 - 1.0;
        let h = cast(&m, 2.5, 6.5, dx + plx * cam, dy + ply * cam, 64).unwrap();
        if h.side == 0 && h.cell_x == 9 {
            dists.push(h.dist);
        }
    }
    assert!(dists.len() > 20);
    for d in &dists {
        assert!(close(*d, 6.5, 1e-4), "{dists:?}");
    }
}

#[test]
fn outside_the_map_is_solid_and_rays_terminate() {
    let m = Map::parse("....\n....\n....");
    let h = cast(&m, 1.5, 1.5, 1.0, 0.3, 64).unwrap();
    assert_eq!(h.cell_x, 4);
    assert!(cast(&m, 1.5, 1.5, 1.0, 0.0, 1).is_none() || cast(&m, 1.5, 1.5, 1.0, 0.0, 1).unwrap().steps == 1);
}

#[test]
fn maze_is_connected_and_exit_reachable() {
    for seed in [1, 2, 3, 42] {
        let m = Map::maze(9, 7, seed);
        assert_eq!((m.w, m.h), (19, 15));
        let exit = m.sprites.iter().find(|s| s.kind == raycast_rs::map::SpriteKind::Exit).unwrap();
        // BFS from the start over open cells
        let mut seen = vec![false; m.w * m.h];
        let start = (m.start.0 as usize, m.start.1 as usize);
        let mut q = std::collections::VecDeque::from([start]);
        seen[start.1 * m.w + start.0] = true;
        while let Some((x, y)) = q.pop_front() {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if m.get(nx as i32, ny as i32) == 0 && !seen[ny * m.w + nx] {
                    seen[ny * m.w + nx] = true;
                    q.push_back((nx, ny));
                }
            }
        }
        assert!(seen[exit.y as usize * m.w + exit.x as usize], "seed {seed}: exit unreachable");
        // border is closed
        for x in 0..m.w {
            assert_ne!(m.get(x as i32, 0), 0);
            assert_ne!(m.get(x as i32, m.h as i32 - 1), 0);
        }
    }
}

#[test]
fn movement_never_enters_walls_fuzz() {
    let mut g = Game::new(Map::maze(8, 8, 5), 64, 40);
    let mut r = Rng::new(1);
    for _ in 0..5000 {
        let input = r.next() & 0b111111;
        g.update(1.0 / 30.0, input | FWD, (r.f32() - 0.5) * 40.0);
        let rr = PLAYER_RADIUS * 0.999;
        for (ox, oy) in [(-rr, -rr), (rr, -rr), (-rr, rr), (rr, rr)] {
            assert!(!g.map.solid(g.x + ox, g.y + oy), "player clipped into a wall at {},{}", g.x, g.y);
        }
    }
}

#[test]
fn wall_sliding_keeps_parallel_motion() {
    let mut g = Game::new(Map::parse(BOX), 32, 20);
    g.x = 5.0;
    g.y = 1.0 + PLAYER_RADIUS + 0.01; // hugging the north wall
    g.angle = -std::f32::consts::FRAC_PI_4; // diagonal up-right into the wall
    let (x0, y0) = (g.x, g.y);
    for _ in 0..30 {
        g.update(1.0 / 60.0, FWD, 0.0);
    }
    assert!(g.x > x0 + 0.5, "should slide east along the wall");
    assert!(close(g.y, y0, 0.02), "should not move into the wall");
}

#[test]
fn lighting_falls_off_with_distance() {
    let mut g = Game::new(Map::parse(BOX), 64, 40);
    g.x = 2.0;
    g.y = 3.0;
    g.render(); // gathers the torch
    let near = g.light_at(2.2, 3.0)[0];
    let mid = g.light_at(4.0, 3.0)[0];
    let far = g.light_at(8.0, 3.0)[0];
    assert!(near > mid && mid > far, "{near} {mid} {far}");
    assert!(close(far, 0.07, 1e-6), "beyond every radius only ambient remains");
    g.opt.lighting = false;
    assert_eq!(g.light_at(8.0, 3.0), [1.0, 1.0, 1.0]);
}

#[test]
fn render_fills_frame_zbuffer_and_is_deterministic() {
    let mk = || {
        let mut g = Game::new(Map::maze(10, 10, 7), 160, 100);
        for i in 0..90 {
            g.update(1.0 / 60.0, if i % 3 == 0 { FWD | TURN_R } else { FWD }, 0.0);
        }
        g.render();
        g
    };
    let (a, b) = (mk(), mk());
    assert_eq!(a.fb, b.fb);
    assert!(a.fb.iter().all(|p| p >> 24 == 255), "every pixel written and opaque");
    assert!(a.zbuf.iter().all(|z| z.is_finite() && *z > 0.0));
    assert_eq!(a.stats.rays, 160);
}

#[test]
fn odd_sizes_do_not_panic() {
    for (w, h) in [(8, 8), (9, 13), (321, 7), (7, 301), (1000, 3)] {
        let mut g = Game::new(Map::maze(5, 5, 1), w, h);
        g.render();
        g.resize(w + 3, h + 5);
        g.update(0.05, FWD, 3.0);
        g.render();
    }
}

#[test]
fn sprites_occluded_by_walls() {
    // a barrel behind a wall must not be drawn; in the open it must be
    let open = "#######\n#P..b.#\n#######";
    let blocked = "#######\n#P.#b.#\n#######";
    for (src, expect) in [(open, 1), (blocked, 0)] {
        let mut g = Game::new(Map::parse(src), 120, 80);
        g.opt.minimap = false;
        g.render();
        assert_eq!(g.stats.sprites_drawn, expect, "{src}");
    }
}

#[test]
fn reaching_the_exit_wins() {
    let mut g = Game::new(Map::parse("#####\n#P.E#\n#####"), 32, 20);
    assert!(!g.won);
    for _ in 0..120 {
        g.update(1.0 / 60.0, FWD, 0.0);
    }
    assert!(g.won);
}

#[test]
fn c_abi_roundtrip() {
    raycast_rs::rc_init(9, 64, 40, 6, 6);
    assert_eq!(raycast_rs::rc_stat(7), 64.0);
    raycast_rs::rc_frame(1.0 / 60.0, FWD, 0.0);
    assert!(!raycast_rs::rc_fb().is_null());
    assert_eq!(raycast_rs::rc_stat(3), 64.0);
    raycast_rs::rc_resize(80, 50);
    raycast_rs::rc_frame(1.0 / 60.0, 0, 0.0);
    assert_eq!(raycast_rs::rc_stat(3), 80.0);
}
