//! The built-in skins' images, drawn in code so they cost nothing to ship.

use image::{Rgba, RgbaImage};

/// A small, repeatable hash, for the pixel art's speckles.
fn noise(x: u32, y: u32, seed: u32) -> u32 {
    let mut n = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_147_483_647));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^ (n >> 16)
}

fn rgb(hex: u32) -> Rgba<u8> {
    Rgba([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255])
}

fn mix(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    let c = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Rgba([c(0), c(1), c(2), 255])
}

/// The built-in images, by name.
pub const NAMES: [&str; 8] = [
    "dirt",
    "sky",
    "sunset",
    "night",
    "glass",
    "aurora",
    "grid",
    "grid-dawn",
];

/// Built-in image `name` (one of [`NAMES`]).
pub fn draw(name: &str) -> Option<RgbaImage> {
    Some(match name {
        "dirt" => dirt(),
        "sky" => sky(Hills::DAY),
        "sunset" => sky(Hills::SUNSET),
        "night" => sky(Hills::NIGHT),
        "glass" => glass(GLASS_BLOBS),
        "aurora" => glass(AURORA_BLOBS),
        "grid" => grid(0xFF2BD6, 0x22D3EE, 0x2A0A36),
        "grid-dawn" => grid(0xFFB84D, 0xFF4F8B, 0x3A1030),
        _ => return None,
    })
}

/// The colours of a [`sky`] scene.
struct Hills {
    top: u32,
    bottom: u32,
    /// How much the ground is darkened, 0 to 1.
    shade: f32,
    /// Square clouds (day), or stars (night).
    stars: bool,
    clouds: u32,
}

impl Hills {
    const DAY: Hills = Hills {
        top: 0x5FA8F0,
        bottom: 0xCBE6FF,
        shade: 0.0,
        stars: false,
        clouds: 0xFFFFFF,
    };
    const SUNSET: Hills = Hills {
        top: 0x2E2457,
        bottom: 0xF59E6B,
        shade: 0.35,
        stars: false,
        clouds: 0xFFC9A3,
    };
    const NIGHT: Hills = Hills {
        top: 0x050816,
        bottom: 0x1E2A55,
        shade: 0.6,
        stars: true,
        clouds: 0xFFFFFF,
    };
}

/// A 16 × 16 dirt block, each pixel 4 × 4.
fn dirt() -> RgbaImage {
    let palette = [0x7A5536, 0x6B4A2F, 0x8B623F, 0x5C3F28, 0x966B47].map(rgb);
    RgbaImage::from_fn(64, 64, |x, y| {
        let n = noise(x / 4, y / 4, 7) as usize;
        palette[n % palette.len()]
    })
}

/// Blocky hills of grass and dirt under a sky: pale with square clouds by day,
/// glowing at sunset, starry at night.
fn sky(scene: Hills) -> RgbaImage {
    let (w, h) = (1600u32, 640u32);
    let block = 40;
    let (top, bottom) = (rgb(scene.top), rgb(scene.bottom));
    let night = rgb(0x05060C);
    let heights: Vec<u32> = (0..w / block + 1)
        .map(|col| {
            let wave = ((col as f32 * 0.35).sin() * 1.6 + (col as f32 * 0.11).cos() * 1.2) as i32;
            (5 + wave + (noise(col, 0, 3) % 2) as i32).clamp(3, 8) as u32
        })
        .collect();
    RgbaImage::from_fn(w, h, |x, y| {
        let col = (x / block) as usize;
        let ground = h - heights[col] * block;
        let px = (x / 8, y / 8);
        if y >= ground {
            let depth = (y - ground) / block;
            let speck = noise(px.0, px.1, 11) % 5;
            let base = match depth {
                0 if y - ground < 14 => [0x5DB83B, 0x4FA332, 0x6CC748, 0x58B036, 0x4C9C30],
                0..=2 => [0x7A5536, 0x6B4A2F, 0x8B623F, 0x5C3F28, 0x966B47],
                _ => [0x7D7D7D, 0x6E6E6E, 0x8A8A8A, 0x747474, 0x666666],
            };
            mix(rgb(base[speck as usize]), night, scene.shade)
        } else {
            let sky = mix(top, bottom, y as f32 / ground.max(1) as f32);
            if scene.stars {
                let star = noise(x / 4, y / 4, 23).is_multiple_of(97) && y < ground - block;
                return if star {
                    mix(sky, rgb(0xFFFFFF), 0.8)
                } else {
                    sky
                };
            }
            // square clouds
            let (cx, cy) = (x / 32, y / 32);
            let cloud =
                (2..=4).contains(&cy) && noise(cx / 3, cy / 2, 5).is_multiple_of(7) && y < h / 2;
            if cloud {
                mix(sky, rgb(scene.clouds), 0.85)
            } else {
                sky
            }
        }
    })
}

type Blob = (f32, f32, f32, u32);

const GLASS_BLOBS: [Blob; 4] = [
    (0.25, 0.30, 0.45, 0x6E56CF),
    (0.75, 0.25, 0.40, 0x2BB3C0),
    (0.60, 0.80, 0.50, 0xE0607E),
    (0.10, 0.85, 0.35, 0x3B6FD8),
];

const AURORA_BLOBS: [Blob; 4] = [
    (0.20, 0.20, 0.55, 0x1FB58F),
    (0.70, 0.15, 0.45, 0x3B82F6),
    (0.85, 0.75, 0.45, 0x8B5CF6),
    (0.30, 0.90, 0.40, 0x0EA5A4),
];

/// Soft colour blobs on dark blue, the Glass skin's backdrop. Drawn small: it is
/// soft anyway, and scaled up when shown.
fn glass(blobs: [Blob; 4]) -> RgbaImage {
    let (w, h) = (600u32, 400u32);
    let blobs = blobs.map(|(x, y, r, c)| (x, y, r, rgb(c)));
    let base = rgb(0x121624);
    RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        blobs.iter().fold(base, |color, (bx, by, r, c)| {
            let d = ((fx - bx).powi(2) + (fy - by).powi(2) * 0.6).sqrt();
            let t = (1.0 - d / r).max(0.0);
            mix(color, *c, t * t * 0.85)
        })
    })
}

/// A neon grid going to the horizon, on black: rows in `rows_hex`, columns in
/// `columns_hex`, a glow of `glow_hex` above the horizon.
fn grid(rows_hex: u32, columns_hex: u32, glow_hex: u32) -> RgbaImage {
    let (w, h) = (1600u32, 640u32);
    let horizon = (h as f32 * 0.45) as u32;
    let ground = (h - horizon) as f32;
    let (black, pink, cyan) = (rgb(0x07070C), rgb(rows_hex), rgb(columns_hex));
    // the rows' heights on screen: closer together towards the horizon
    let rows: Vec<f32> = (0..24)
        .map(|n| horizon as f32 + ground / (1.0 + n as f32 * 0.55))
        .collect();
    let centre = w as f32 / 2.0;
    RgbaImage::from_fn(w, h, |x, y| {
        if y < horizon {
            // a glow rising from the horizon
            let t = 1.0 - (horizon - y) as f32 / horizon as f32;
            return mix(black, rgb(glow_hex), t.powi(3));
        }
        // 0 at the horizon, 1 at the bottom
        let d = (y - horizon) as f32 / ground;
        let floor = mix(rgb(0x14051C), black, d);
        if y <= horizon + 1 {
            return pink;
        }
        let width = 0.8 + d * 1.6;
        let on_row = rows.iter().any(|r| (y as f32 - r).abs() < width);
        // columns meeting at the middle of the horizon, 180 px apart at the bottom
        let spacing = 180.0 * d;
        let offset = x as f32 - centre;
        let column = (offset / spacing).round();
        let on_column = (offset - column * spacing).abs() < width;
        if on_row {
            mix(floor, pink, 0.55 + d * 0.4)
        } else if on_column {
            mix(floor, cyan, 0.45 + d * 0.45)
        } else {
            floor
        }
    })
}
