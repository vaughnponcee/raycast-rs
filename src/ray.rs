//! Ray / grid intersection with DDA (Amanatides & Woo traversal).
//!
//! For a ray p + t·d, the distance (in t) between successive vertical grid
//! lines is |1/d.x| and between horizontal lines |1/d.y|. DDA keeps the
//! t of the next x-line and next y-line crossing and always steps the smaller,
//! visiting every cell the ray passes through, in order, with no gaps.

use crate::map::Map;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Perpendicular distance to the wall plane (t along `dir` when `dir` is
    /// the camera ray, i.e. distance projected onto the view direction when
    /// the ray is `dir + plane*cam_x`). Using this instead of Euclidean
    /// distance is what removes the fish-eye effect.
    pub dist: f32,
    /// 0 = hit a vertical grid line (an x-side), 1 = a horizontal one (y-side)
    pub side: u8,
    pub cell_x: i32,
    pub cell_y: i32,
    /// wall cell value (texture id)
    pub wall: u8,
    /// where along the wall face the ray hit, in [0, 1)
    pub wall_x: f32,
    /// DDA steps taken (for stats / tests)
    pub steps: u32,
}

/// Cast from (px, py) along (dx, dy). `dx`/`dy` need not be normalized; the
/// returned `dist` is in units of that vector's length (t). Stops after
/// `max_steps` cells (returns None) so a ray can never loop forever.
pub fn cast(map: &Map, px: f32, py: f32, dx: f32, dy: f32, max_steps: u32) -> Option<Hit> {
    let mut cx = px.floor() as i32;
    let mut cy = py.floor() as i32;
    // |1/d|; an axis-parallel ray never crosses lines on the other axis
    let delta_x = if dx == 0.0 { f32::INFINITY } else { (1.0 / dx).abs() };
    let delta_y = if dy == 0.0 { f32::INFINITY } else { (1.0 / dy).abs() };
    let (step_x, mut side_x) = if dx < 0.0 {
        (-1, (px - cx as f32) * delta_x)
    } else {
        (1, (cx as f32 + 1.0 - px) * delta_x)
    };
    let (step_y, mut side_y) = if dy < 0.0 {
        (-1, (py - cy as f32) * delta_y)
    } else {
        (1, (cy as f32 + 1.0 - py) * delta_y)
    };
    let mut steps = 0;
    loop {
        let side;
        if side_x < side_y {
            side_x += delta_x;
            cx += step_x;
            side = 0;
        } else {
            side_y += delta_y;
            cy += step_y;
            side = 1;
        }
        steps += 1;
        let wall = map.get(cx, cy);
        if wall != 0 {
            // we overshot by one delta: back it out to get the crossing t
            let dist = if side == 0 { side_x - delta_x } else { side_y - delta_y };
            let mut wall_x = if side == 0 { py + dist * dy } else { px + dist * dx };
            wall_x -= wall_x.floor();
            // mirror so textures read left-to-right on every face
            if (side == 0 && dx < 0.0) || (side == 1 && dy > 0.0) {
                wall_x = 1.0 - wall_x;
            }
            return Some(Hit { dist, side, cell_x: cx, cell_y: cy, wall, wall_x: wall_x.min(0.99999), steps });
        }
        if steps >= max_steps {
            return None;
        }
    }
}
