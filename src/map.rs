//! Grid map, seeded maze generation, sprites and lights.

/// xorshift32: tiny, seedable, good enough for level generation.
#[derive(Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    pub fn f32(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpriteKind {
    Barrel,
    Pillar,
    Lamp,
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub x: f32,
    pub y: f32,
    pub kind: SpriteKind,
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub x: f32,
    pub y: f32,
    pub color: [f32; 3],
    pub radius: f32,
    pub flicker: f32, // 0 = steady
}

pub struct Map {
    pub w: usize,
    pub h: usize,
    /// 0 = empty, 1..=4 = wall texture id
    pub cells: Vec<u8>,
    pub sprites: Vec<Sprite>,
    pub lights: Vec<Light>,
    pub start: (f32, f32, f32), // x, y, angle
}

impl Map {
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            1 // outside the map counts as solid wall: rays always terminate
        } else {
            self.cells[y as usize * self.w + x as usize]
        }
    }

    #[inline]
    pub fn solid(&self, x: f32, y: f32) -> bool {
        self.get(x.floor() as i32, y.floor() as i32) != 0
    }

    /// Parse an ASCII map. `#`=brick `S`=stone `W`=wood `M`=metal, `.`/space=floor,
    /// `P`=player start (facing east), `b`=barrel `p`=pillar `L`=lamp `E`=exit.
    pub fn parse(src: &str) -> Map {
        let rows: Vec<&str> = src.lines().filter(|l| !l.trim().is_empty()).collect();
        let h = rows.len();
        let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut m = Map { w, h, cells: vec![0; w * h], sprites: vec![], lights: vec![], start: (1.5, 1.5, 0.0) };
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let cell = match ch {
                    '#' => 1,
                    'S' => 2,
                    'W' => 3,
                    'M' => 4,
                    _ => 0,
                };
                m.cells[y * w + x] = cell;
                match ch {
                    'P' => m.start = (cx, cy, 0.0),
                    'b' => m.sprites.push(Sprite { x: cx, y: cy, kind: SpriteKind::Barrel }),
                    'p' => m.sprites.push(Sprite { x: cx, y: cy, kind: SpriteKind::Pillar }),
                    'L' => m.add_lamp(cx, cy),
                    'E' => m.add_exit(cx, cy),
                    _ => {}
                }
            }
        }
        m
    }

    /// How many of the 8 cells around (x, y) are open floor.
    pub fn open_around(&self, x: usize, y: usize) -> usize {
        let mut n = 0;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if (dx, dy) != (0, 0) && self.get(x as i32 + dx, y as i32 + dy) == 0 {
                    n += 1;
                }
            }
        }
        n
    }

    fn add_lamp(&mut self, x: f32, y: f32) {
        self.sprites.push(Sprite { x, y, kind: SpriteKind::Lamp });
        self.lights.push(Light { x, y, color: [1.0, 0.72, 0.42], radius: 3.2, flicker: 0.12 });
    }

    fn add_exit(&mut self, x: f32, y: f32) {
        self.sprites.push(Sprite { x, y, kind: SpriteKind::Exit });
        self.lights.push(Light { x, y, color: [0.35, 1.0, 0.55], radius: 2.6, flicker: 0.0 });
    }

    /// Perfect maze by iterative depth-first backtracking on a (2n+1)^2 grid,
    /// then extra openings (loops) and a few rooms so it plays less like a corridor.
    pub fn maze(cells_w: usize, cells_h: usize, seed: u32) -> Map {
        let (w, h) = (cells_w * 2 + 1, cells_h * 2 + 1);
        let mut rng = Rng::new(seed);
        let mut g = vec![1u8; w * h];
        let mut visited = vec![false; cells_w * cells_h];
        let mut stack = vec![(0usize, 0usize)];
        visited[0] = true;
        g[w + 1] = 0;
        while let Some(&(cx, cy)) = stack.last() {
            let mut nbrs = [(0usize, 0usize); 4];
            let mut n = 0;
            if cx > 0 && !visited[cy * cells_w + cx - 1] { nbrs[n] = (cx - 1, cy); n += 1; }
            if cx + 1 < cells_w && !visited[cy * cells_w + cx + 1] { nbrs[n] = (cx + 1, cy); n += 1; }
            if cy > 0 && !visited[(cy - 1) * cells_w + cx] { nbrs[n] = (cx, cy - 1); n += 1; }
            if cy + 1 < cells_h && !visited[(cy + 1) * cells_w + cx] { nbrs[n] = (cx, cy + 1); n += 1; }
            if n == 0 {
                stack.pop();
                continue;
            }
            let (nx, ny) = nbrs[rng.below(n as u32) as usize];
            visited[ny * cells_w + nx] = true;
            // carve the cell and the wall between
            g[(ny * 2 + 1) * w + nx * 2 + 1] = 0;
            g[(cy + ny + 1) * w + cx + nx + 1] = 0;
            stack.push((nx, ny));
        }
        // loops: knock out ~8% of interior walls that separate two corridors
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                if g[y * w + x] == 1 && rng.below(100) < 8 {
                    let horiz = g[y * w + x - 1] == 0 && g[y * w + x + 1] == 0;
                    let vert = g[(y - 1) * w + x] == 0 && g[(y + 1) * w + x] == 0;
                    if horiz ^ vert {
                        g[y * w + x] = 0;
                    }
                }
            }
        }
        // rooms: clear a few 3x3 areas
        for _ in 0..(cells_w * cells_h / 20).max(1) {
            let rx = 1 + rng.below((w - 4) as u32) as usize;
            let ry = 1 + rng.below((h - 4) as u32) as usize;
            for y in ry..ry + 3 {
                for x in rx..rx + 3 {
                    g[y * w + x] = 0;
                }
            }
        }
        // texture variety: walls take a texture by region
        for y in 0..h {
            for x in 0..w {
                if g[y * w + x] != 0 {
                    let region = ((x / 6) * 7 + (y / 6) * 13) % 4;
                    g[y * w + x] = [1, 2, 3, 4][region];
                }
            }
        }
        let mut m = Map { w, h, cells: g, sprites: vec![], lights: vec![], start: (1.5, 1.5, 0.0) };
        // face the open neighbour
        m.start.2 = if m.get(2, 1) == 0 { 0.0 } else { std::f32::consts::FRAC_PI_2 };
        // exit at the far corner cell
        m.add_exit((w - 2) as f32 + 0.5, (h - 2) as f32 + 0.5);
        // decorate open floor cells, away from the start
        let mut lamps = 0;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                if m.cells[y * w + x] != 0 || (x < 4 && y < 4) || (x + 3 > w && y + 3 > h) {
                    continue;
                }
                let r = rng.below(1000);
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                // barrels and pillars block movement, so they only go in open rooms:
                // a prop in a one-tile corridor would seal it off
                let open = m.open_around(x, y);
                if r < 22 {
                    m.add_lamp(cx, cy); // lamps don't block: fine anywhere
                    lamps += 1;
                } else if r < 70 && open >= 8 {
                    let kind = if r < 55 { SpriteKind::Barrel } else { SpriteKind::Pillar };
                    m.sprites.push(Sprite { x: cx, y: cy, kind });
                }
            }
        }
        if lamps == 0 {
            m.add_lamp(3.5, 1.5);
        }
        m
    }
}
