//! Procedural 64x64 textures and sprites (no asset files). Pixels are
//! packed RGBA little-endian (r | g<<8 | b<<16 | a<<24), i.e. exactly the
//! byte order of a canvas ImageData, so the framebuffer can be blitted as-is.

pub const T: usize = 64;

#[inline]
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16 | (a as u32) << 24
}

fn hash(x: u32, y: u32, s: u32) -> u32 {
    let mut h = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ s.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

fn noise(x: u32, y: u32, s: u32) -> f32 {
    (hash(x, y, s) & 0xffff) as f32 / 65535.0
}

fn shade(c: [f32; 3], k: f32) -> u32 {
    let f = |v: f32| (v * k).clamp(0.0, 255.0) as u8;
    rgba(f(c[0]), f(c[1]), f(c[2]), 255)
}

pub struct Textures {
    pub walls: Vec<Vec<u32>>, // index = wall id - 1
    pub floor: Vec<u32>,
    pub ceil: Vec<u32>,
    pub sprites: Vec<Vec<u32>>, // Barrel, Pillar, Lamp, Exit (alpha 0 = transparent)
}

impl Textures {
    pub fn generate() -> Textures {
        let mut brick = vec![0; T * T];
        let mut stone = vec![0; T * T];
        let mut wood = vec![0; T * T];
        let mut metal = vec![0; T * T];
        let mut floor = vec![0; T * T];
        let mut ceil = vec![0; T * T];
        for y in 0..T {
            for x in 0..T {
                let (xu, yu) = (x as u32, y as u32);
                let n = noise(xu, yu, 1);
                // brick: 16x8 bricks, staggered rows, 1px mortar
                let row = y / 8;
                let bx = (x + if row % 2 == 1 { 8 } else { 0 }) % T;
                let mortar = y % 8 == 0 || bx % 16 == 0;
                let tint = noise((bx / 16) as u32, row as u32, 7) * 0.35 + 0.8;
                brick[y * T + x] = if mortar { shade([150.0, 140.0, 128.0], 0.75 + 0.2 * n) }
                    else { shade([150.0, 62.0, 44.0], tint * (0.85 + 0.25 * n)) };
                // stone: irregular 16px blocks with darker seams
                let sx = x / 16;
                let sy = (y + (sx % 2) * 8) / 16;
                let seam = x % 16 == 0 || (y + (sx % 2) * 8) % 16 == 0;
                let k = noise(sx as u32, sy as u32, 3) * 0.3 + 0.75 + 0.15 * n;
                stone[y * T + x] = if seam { shade([70.0, 72.0, 78.0], 0.8) } else { shade([128.0, 130.0, 136.0], k) };
                // wood: vertical planks with sinusoidal grain
                let plank = x / 13;
                let grain = ((y as f32 * 0.35 + (plank as f32) * 3.1 + (x % 13) as f32 * 0.9).sin() * 0.5 + 0.5) * 0.25;
                wood[y * T + x] = if x % 13 == 0 { shade([60.0, 38.0, 20.0], 1.0) }
                    else { shade([132.0, 88.0, 50.0], 0.8 + grain + 0.1 * n) };
                // metal: 32px panels, rivets at corners, brushed noise
                let (px, py) = (x % 32, y % 32);
                let rivet = (px == 4 || px == 27) && (py == 4 || py == 27);
                let edge = px == 0 || py == 0;
                metal[y * T + x] = if rivet { shade([210.0, 214.0, 220.0], 1.0) }
                    else if edge { shade([55.0, 60.0, 68.0], 1.0) }
                    else { shade([96.0, 104.0, 116.0], 0.85 + 0.25 * noise(xu, yu / 3, 9)) };
                // floor: 32px stone tiles; ceiling: dark boards
                let tile = (x / 32 + y / 32) % 2;
                floor[y * T + x] = if x % 32 == 0 || y % 32 == 0 { shade([50.0, 48.0, 46.0], 1.0) }
                    else { shade(if tile == 0 { [96.0, 90.0, 82.0] } else { [84.0, 80.0, 74.0] }, 0.85 + 0.25 * n) };
                ceil[y * T + x] = if y % 16 == 0 { shade([20.0, 18.0, 16.0], 1.0) }
                    else { shade([58.0, 50.0, 42.0], 0.8 + 0.3 * noise(xu / 8, yu, 5)) };
            }
        }
        Textures { walls: vec![brick, stone, wood, metal], floor, ceil, sprites: vec![barrel(), pillar(), lamp(), exit()] }
    }
}

fn barrel() -> Vec<u32> {
    let mut s = vec![0; T * T];
    for y in 18..T {
        for x in 16..48 {
            let u = (x as f32 - 31.5) / 16.0; // -1..1 across the barrel
            let light = (1.0 - u * u).sqrt() * 0.75 + 0.25; // cylinder shading
            let band = y == 24 || y == 25 || y == 44 || y == 45 || y == 58 || y == 59;
            let c = if band { [70.0, 70.0, 76.0] } else { [120.0, 72.0, 36.0] };
            s[y * T + x] = shade(c, light * (0.9 + 0.2 * noise(x as u32, y as u32 / 2, 4)));
        }
    }
    s
}

fn pillar() -> Vec<u32> {
    let mut s = vec![0; T * T];
    for y in 0..T {
        let (x0, x1) = if y < 6 || y > 57 { (18, 46) } else { (22, 42) };
        for x in x0..x1 {
            let u = (x as f32 - 31.5) / ((x1 - x0) as f32 / 2.0);
            let light = (1.0 - u * u).max(0.0).sqrt() * 0.7 + 0.3;
            let flute = if (x - x0) % 5 == 0 && y >= 6 && y <= 57 { 0.8 } else { 1.0 };
            s[y * T + x] = shade([170.0, 166.0, 156.0], light * flute);
        }
    }
    s
}

fn lamp() -> Vec<u32> {
    let mut s = vec![0; T * T];
    for y in 20..T {
        for x in 30..34 {
            s[y * T + x] = shade([60.0, 60.0, 64.0], 1.0);
        }
    }
    for y in 4..22 {
        for x in 20..44 {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 12.5).powi(2)).sqrt();
            if d < 9.5 {
                let k = 1.3 - d / 12.0;
                s[y * T + x] = shade([255.0, 210.0, 140.0], k.max(0.6));
            }
        }
    }
    s
}

fn exit() -> Vec<u32> {
    let mut s = vec![0; T * T];
    for y in 0..T {
        for x in 0..T {
            let d = ((x as f32 - 31.5).powi(2) + ((y as f32 - 34.0) * 0.8).powi(2)).sqrt();
            if (16.0..26.0).contains(&d) {
                let swirl = ((y as f32 - 34.0).atan2(x as f32 - 31.5) * 4.0 + d * 0.5).sin() * 0.25 + 0.85;
                s[y * T + x] = shade([90.0, 255.0, 140.0], swirl);
            } else if d < 16.0 {
                s[y * T + x] = shade([30.0, 120.0, 60.0], 0.6 + 0.4 * noise(x as u32, y as u32, 11));
            }
        }
    }
    s
}
