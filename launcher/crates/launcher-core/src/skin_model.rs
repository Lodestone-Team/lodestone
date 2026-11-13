//! The player as a 3D model, drawn from a skin (and a cape) at any angle: what the
//! "Change skin" dialog turns around when its picture is dragged.
//!
//! The model is Minecraft's own: six boxes (head, body, arms, legs) with the skin's
//! second layer a little outside each, and the cape as a thin box hanging from the
//! shoulders. It is drawn in software into an image: every face is two triangles with
//! a depth buffer, the texture sampled without smoothing. There is no perspective, so
//! textures stay exact.
//!
//! Lengths are in skin pixels. `x` runs to the viewer's right when the player faces
//! the viewer, `y` up from the feet, `z` towards the viewer. The player's right arm
//! is on the viewer's left.

use std::io::Cursor;

use image::{ImageFormat, ImageReader, Rgba, RgbaImage};

use crate::skin::SkinError;

/// A skin or cape may not be larger than this on a side.
const MAX_SIDE: u32 = 1024;

/// How far the second layer stands off the first, and the hat off the head.
const LAYER: f32 = 0.25;
const HAT: f32 = 0.5;

/// How far the cape hangs away from the back, in radians.
const CAPE_TILT: f32 = 0.16;

/// The width and height of the space the model is drawn in, in skin pixels: room for
/// the player turned any way, with the cape.
pub const VIEW_WIDTH: f32 = 22.0;
pub const VIEW_HEIGHT: f32 = 36.0;

type Vec3 = [f32; 3];

/// One corner of a face: where it is, and its place in the texture, in texels.
#[derive(Clone, Copy)]
struct Corner {
    at: Vec3,
    uv: [f32; 2],
}

/// A rectangle of the model with its part of a texture.
struct Face {
    /// Texture left-top, right-top, right-bottom, left-bottom.
    corners: [Corner; 4],
    normal: Vec3,
    /// The part of the texture the face may read: left, top, right, bottom.
    rect: [f32; 4],
    cape: bool,
    /// The second layer and the cape: see-through where the texture is, and seen
    /// from both sides.
    overlay: bool,
}

/// A skin (and a cape), ready to draw from any side.
pub struct PlayerModel {
    skin: RgbaImage,
    cape: Option<RgbaImage>,
    faces: Vec<Face>,
}

fn decode(png: &[u8]) -> Result<RgbaImage, SkinError> {
    let mut reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader
        .decode()
        .map_err(|err| SkinError::Invalid(err.to_string()))?
        .to_rgba8())
}

/// A box of the model. `origin` is where its texture starts and `size` its width,
/// height and depth in texels, as Minecraft lays a box out: the top and bottom over
/// a strip of right side, front, left side and back.
struct Part {
    origin: [f32; 2],
    size: [f32; 3],
    /// The corner with the smallest x, y and z.
    min: Vec3,
    /// How much larger than `size` it is drawn, on every side.
    inflate: f32,
    /// Drawn as the mirror image of its texture: the left arm and leg of an old
    /// 64×32 skin, which only has the right ones.
    mirror: bool,
    cape: bool,
    overlay: bool,
}

/// Adds a part's six faces. `scale` is how many texels a skin pixel is (2 for a
/// 128×128 skin), and `place` moves each corner to where the part hangs.
fn add_part(faces: &mut Vec<Face>, part: &Part, scale: f32, place: &dyn Fn(Vec3) -> Vec3) {
    let [u, v] = part.origin;
    let [w, h, d] = part.size;
    let i = part.inflate;
    let (x0, y0, z0) = (part.min[0] - i, part.min[1] - i, part.min[2] - i);
    let (x1, y1, z1) = (
        part.min[0] + w + i,
        part.min[1] + h + i,
        part.min[2] + d + i,
    );
    // The strip: the side on the player's right, the front, the other side, the back.
    let (mut near_side, mut far_side) = ([u, v + d, d, h], [u + d + w, v + d, d, h]);
    if part.mirror {
        std::mem::swap(&mut near_side, &mut far_side);
    }
    // Each face: its texture rectangle, then where the texture's left-top, right-top,
    // right-bottom and left-bottom corners are, and which way it faces.
    let sides: [([f32; 4], [Vec3; 4], Vec3); 6] = [
        (
            [u + d, v + d, w, h],
            [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]],
            [0.0, 0.0, 1.0],
        ),
        (
            [u + 2.0 * d + w, v + d, w, h],
            [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]],
            [0.0, 0.0, -1.0],
        ),
        (
            near_side,
            [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]],
            [-1.0, 0.0, 0.0],
        ),
        (
            far_side,
            [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]],
            [1.0, 0.0, 0.0],
        ),
        (
            [u + d, v, w, d],
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            [0.0, 1.0, 0.0],
        ),
        (
            [u + d + w, v, w, d],
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            [0.0, -1.0, 0.0],
        ),
    ];
    // which way `place` turns a direction: the difference of two placed points
    let zero = place([0.0, 0.0, 0.0]);
    for (rect, at, normal) in sides {
        let [left, top, width, height] = rect.map(|texels| texels * scale);
        let (mut near, mut far) = (left, left + width);
        if part.mirror {
            std::mem::swap(&mut near, &mut far);
        }
        let uv = [
            [near, top],
            [far, top],
            [far, top + height],
            [near, top + height],
        ];
        let turned = place(normal);
        faces.push(Face {
            corners: std::array::from_fn(|n| Corner {
                at: place(at[n]),
                uv: uv[n],
            }),
            normal: [
                turned[0] - zero[0],
                turned[1] - zero[1],
                turned[2] - zero[2],
            ],
            rect: [left, top, left + width, top + height],
            cape: part.cape,
            overlay: part.overlay,
        });
    }
}

impl PlayerModel {
    /// The model of `skin_png`, with arms 3 pixels wide when `slim`, wearing
    /// `cape_png` if given. A cape that can't be read is left off.
    pub fn new(skin_png: &[u8], slim: bool, cape_png: Option<&[u8]>) -> Result<Self, SkinError> {
        let skin = decode(skin_png)?;
        let (width, height) = skin.dimensions();
        let legacy = height * 2 == width;
        if width < 64 || width % 64 != 0 || !(height == width || legacy) {
            return Err(SkinError::Invalid(format!(
                "unexpected size {width}x{height}"
            )));
        }
        let cape = cape_png
            .and_then(|png| decode(png).ok())
            .filter(|cape| cape.width() >= 22 && cape.height() >= 17);
        let scale = (width / 64) as f32;
        let arm = if slim { 3.0 } else { 4.0 };
        let still = |at: Vec3| at;
        let mut faces = Vec::new();
        let mut part = |origin: [f32; 2], size: [f32; 3], min: Vec3, mirror: bool| {
            add_part(
                &mut faces,
                &Part {
                    origin,
                    size,
                    min,
                    inflate: 0.0,
                    mirror,
                    cape: false,
                    overlay: false,
                },
                scale,
                &still,
            );
        };
        part([0.0, 0.0], [8.0, 8.0, 8.0], [-4.0, 24.0, -4.0], false);
        part([16.0, 16.0], [8.0, 12.0, 4.0], [-4.0, 12.0, -2.0], false);
        part(
            [40.0, 16.0],
            [arm, 12.0, 4.0],
            [-4.0 - arm, 12.0, -2.0],
            false,
        );
        part([0.0, 16.0], [4.0, 12.0, 4.0], [-4.0, 0.0, -2.0], false);
        if legacy {
            part([40.0, 16.0], [arm, 12.0, 4.0], [4.0, 12.0, -2.0], true);
            part([0.0, 16.0], [4.0, 12.0, 4.0], [0.0, 0.0, -2.0], true);
        } else {
            part([32.0, 48.0], [arm, 12.0, 4.0], [4.0, 12.0, -2.0], false);
            part([16.0, 48.0], [4.0, 12.0, 4.0], [0.0, 0.0, -2.0], false);
        }
        let mut layer = |origin: [f32; 2], size: [f32; 3], min: Vec3, inflate: f32| {
            add_part(
                &mut faces,
                &Part {
                    origin,
                    size,
                    min,
                    inflate,
                    mirror: false,
                    cape: false,
                    overlay: true,
                },
                scale,
                &still,
            );
        };
        // Vanilla ignores an old skin's hat when none of it is see-through: old
        // skins often filled that part with a colour.
        let hat_is_solid = (32..64)
            .all(|x| (0..16).all(|y| skin.get_pixel(x * width / 64, y * width / 64)[3] == 255));
        if !(legacy && hat_is_solid) {
            layer([32.0, 0.0], [8.0, 8.0, 8.0], [-4.0, 24.0, -4.0], HAT);
        }
        if !legacy {
            layer([16.0, 32.0], [8.0, 12.0, 4.0], [-4.0, 12.0, -2.0], LAYER);
            layer(
                [40.0, 32.0],
                [arm, 12.0, 4.0],
                [-4.0 - arm, 12.0, -2.0],
                LAYER,
            );
            layer([48.0, 48.0], [arm, 12.0, 4.0], [4.0, 12.0, -2.0], LAYER);
            layer([0.0, 32.0], [4.0, 12.0, 4.0], [-4.0, 0.0, -2.0], LAYER);
            layer([0.0, 48.0], [4.0, 12.0, 4.0], [0.0, 0.0, -2.0], LAYER);
        }
        if let Some(cape) = &cape {
            // A box 10 wide, 16 tall and 1 deep whose texture "front" is the outside
            // of the cape: turned to face backwards, hung from the shoulders and
            // swung a little away from the back.
            let (sin, cos) = CAPE_TILT.sin_cos();
            let hang = move |[x, y, z]: Vec3| {
                // face backwards: x and z change sign
                let (x, y, z) = (-x, y, -z);
                // swing about the top edge, at the shoulders
                let (dy, dz) = (y, z);
                [x, 24.0 + dy * cos + dz * sin, -2.0 + dy * sin + dz * cos]
            };
            add_part(
                &mut faces,
                &Part {
                    origin: [0.0, 0.0],
                    size: [10.0, 16.0, 1.0],
                    min: [-5.0, -16.0, 0.0],
                    inflate: 0.0,
                    mirror: false,
                    cape: true,
                    overlay: true,
                },
                (cape.width() / 64).max(1) as f32,
                &hang,
            );
        }
        Ok(Self { skin, cape, faces })
    }

    /// Draws the model into an image of `width`×`height` pixels with nothing behind
    /// it. `yaw` turns the player about their height, in radians: 0 faces the viewer,
    /// and more turns their right side towards the viewer. `pitch` tips the top of
    /// the head towards the viewer.
    pub fn render(&self, yaw: f32, pitch: f32, width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        if width == 0 || height == 0 {
            return image;
        }
        let mut depth = vec![f32::NEG_INFINITY; (width * height) as usize];
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        // about the middle of the body, so the player turns on the spot
        let turn = |[x, y, z]: Vec3, about: f32| -> Vec3 {
            let y = y - about;
            let (x, z) = (x * cos_yaw + z * sin_yaw, -x * sin_yaw + z * cos_yaw);
            let (y, z) = (y * cos_pitch - z * sin_pitch, y * sin_pitch + z * cos_pitch);
            [x, y, z]
        };
        let scale = (width as f32 / VIEW_WIDTH).min(height as f32 / VIEW_HEIGHT);
        let project = |at: Vec3| -> Vec3 {
            let [x, y, z] = turn(at, 16.0);
            [
                width as f32 / 2.0 + x * scale,
                height as f32 / 2.0 - y * scale,
                z,
            ]
        };
        // light from over the viewer's shoulder
        let light = {
            let l: Vec3 = [0.35, 0.55, 0.76];
            let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
            [l[0] / len, l[1] / len, l[2] / len]
        };
        for face in &self.faces {
            let normal = turn(face.normal, 0.0);
            // the back of a solid box is never seen
            if !face.overlay && normal[2] <= 0.0 {
                continue;
            }
            let texture = match (face.cape, &self.cape) {
                (true, Some(cape)) => cape,
                (true, None) => continue,
                (false, _) => &self.skin,
            };
            let lit = (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2])
                .abs()
                .min(1.0);
            let shade = 0.58 + 0.42 * lit;
            let corners = face.corners.map(|corner| (project(corner.at), corner.uv));
            for triangle in [[0, 1, 2], [0, 2, 3]] {
                fill(
                    &mut image,
                    &mut depth,
                    triangle.map(|n| corners[n]),
                    texture,
                    face.rect,
                    face.overlay,
                    shade,
                );
            }
        }
        image
    }
}

/// Fills one triangle of a face, where it is nearer than what is drawn already.
fn fill(
    image: &mut RgbaImage,
    depth: &mut [f32],
    corners: [(Vec3, [f32; 2]); 3],
    texture: &RgbaImage,
    rect: [f32; 4],
    overlay: bool,
    shade: f32,
) {
    let [(a, ta), (b, tb), (c, tc)] = corners;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let (width, height) = image.dimensions();
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
    let max_x = (a[0].max(b[0]).max(c[0]).ceil() as i64).clamp(0, width as i64) as u32;
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
    let max_y = (a[1].max(b[1]).max(c[1]).ceil() as i64).clamp(0, height as i64) as u32;
    let (tex_width, tex_height) = texture.dimensions();
    for y in min_y..max_y {
        for x in min_x..max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // how much of each corner the pixel is
            let wa = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            let wb = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            let wc = 1.0 - wa - wb;
            if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                continue;
            }
            let z = wa * a[2] + wb * b[2] + wc * c[2];
            let at = (y * width + x) as usize;
            if z <= depth[at] {
                continue;
            }
            // inside the face's own part of the texture, whatever the rounding
            let u = (wa * ta[0] + wb * tb[0] + wc * tc[0]).clamp(rect[0], rect[2] - 0.01);
            let v = (wa * ta[1] + wb * tb[1] + wc * tc[1]).clamp(rect[1], rect[3] - 0.01);
            let (u, v) = (u as u32, v as u32);
            if u >= tex_width || v >= tex_height {
                continue;
            }
            let texel = texture.get_pixel(u, v);
            // the second layer is see-through where the skin leaves it empty
            if overlay && texel[3] < 128 {
                continue;
            }
            depth[at] = z;
            let lit = |channel: u8| (channel as f32 * shade).round().min(255.0) as u8;
            image.put_pixel(
                x,
                y,
                Rgba([lit(texel[0]), lit(texel[1]), lit(texel[2]), 255]),
            );
        }
    }
}

/// The outside of a cape, as it hangs: the 10×16 picture in its texture, `scale`
/// screen pixels for each of its pixels.
pub fn cape_picture(cape_png: &[u8], scale: u32) -> Result<RgbaImage, SkinError> {
    let cape = decode(cape_png)?;
    let unit = cape.width() / 64;
    if unit == 0 || cape.height() < 17 * unit {
        return Err(SkinError::Invalid(format!(
            "unexpected cape size {}x{}",
            cape.width(),
            cape.height()
        )));
    }
    let picture = image::imageops::crop_imm(&cape, unit, unit, 10 * unit, 16 * unit).to_image();
    Ok(image::imageops::resize(
        &picture,
        10 * unit * scale,
        16 * unit * scale,
        image::imageops::FilterType::Nearest,
    ))
}

/// A made-up cape, for tests and the offscreen screenshots: `colour` with a lighter
/// band and a mark, so its picture and its place on the model can be told.
pub fn sample_cape(colour: [u8; 3]) -> RgbaImage {
    let mut cape = RgbaImage::from_pixel(64, 32, Rgba([0, 0, 0, 0]));
    let [r, g, b] = colour;
    for y in 0..17 {
        for x in 0..22 {
            cape.put_pixel(x, y, Rgba([r / 2, g / 2, b / 2, 255]));
        }
    }
    for y in 1..17 {
        for x in 1..11 {
            let band = (4..7).contains(&y);
            let mark = (9..13).contains(&y) && (4..8).contains(&x);
            let pixel = if mark {
                Rgba([250, 240, 200, 255])
            } else if band {
                Rgba([
                    r.saturating_add(50),
                    g.saturating_add(50),
                    b.saturating_add(50),
                    255,
                ])
            } else {
                Rgba([r, g, b, 255])
            };
            cape.put_pixel(x, y, pixel);
        }
    }
    cape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::encode_png;

    const HEAD: Rgba<u8> = Rgba([200, 0, 0, 255]);
    const HEAD_BACK: Rgba<u8> = Rgba([0, 0, 200, 255]);
    const BODY: Rgba<u8> = Rgba([0, 200, 0, 255]);
    const RIGHT_ARM: Rgba<u8> = Rgba([200, 200, 0, 255]);
    const LEFT_ARM: Rgba<u8> = Rgba([0, 200, 200, 255]);
    const LEGS: Rgba<u8> = Rgba([200, 0, 200, 255]);

    /// A skin with one colour for each part, and the back of the head its own.
    fn skin() -> RgbaImage {
        let mut skin = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        let mut fill = |x0: u32, y0: u32, w: u32, h: u32, colour: Rgba<u8>| {
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    skin.put_pixel(x, y, colour);
                }
            }
        };
        fill(0, 0, 32, 16, HEAD);
        fill(24, 8, 8, 8, HEAD_BACK);
        fill(16, 16, 24, 16, BODY);
        fill(40, 16, 16, 16, RIGHT_ARM);
        fill(32, 48, 16, 16, LEFT_ARM);
        fill(0, 16, 16, 16, LEGS);
        fill(16, 48, 16, 16, LEGS);
        skin
    }

    /// The hue of a drawn pixel: which channels are lit, whatever the shading.
    fn hue(pixel: &Rgba<u8>) -> [bool; 3] {
        [pixel[0] > 60, pixel[1] > 60, pixel[2] > 60]
    }

    fn same(pixel: &Rgba<u8>, colour: Rgba<u8>) -> bool {
        pixel[3] == 255 && hue(pixel) == hue(&colour)
    }

    /// The pixel at a place of the view, in skin pixels from its middle.
    fn at(image: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
        let scale = image.width() as f32 / VIEW_WIDTH;
        *image.get_pixel(
            (image.width() as f32 / 2.0 + x * scale) as u32,
            (image.height() as f32 / 2.0 - y * scale) as u32,
        )
    }

    #[test]
    fn the_player_faces_the_viewer_and_turns() {
        let model = PlayerModel::new(&encode_png(&skin()).unwrap(), false, None).unwrap();
        let (w, h) = (VIEW_WIDTH as u32 * 8, VIEW_HEIGHT as u32 * 8);
        let front = model.render(0.0, 0.0, w, h);
        // from the feet at -16 to the top of the head at 16
        assert!(same(&at(&front, 0.0, 12.0), HEAD));
        assert!(same(&at(&front, 0.0, 2.0), BODY));
        assert!(same(&at(&front, -1.0, -10.0), LEGS));
        // the player's right arm is on the viewer's left
        assert!(same(&at(&front, -6.0, 2.0), RIGHT_ARM));
        assert!(same(&at(&front, 6.0, 2.0), LEFT_ARM));
        // nothing beside the head, or outside the arms
        assert_eq!(at(&front, -6.0, 12.0)[3], 0);
        assert_eq!(at(&front, 9.5, 2.0)[3], 0);

        // from behind, the arms have changed sides and the head shows its back
        let back = model.render(std::f32::consts::PI, 0.0, w, h);
        assert!(same(&at(&back, 0.0, 12.0), HEAD_BACK));
        assert!(same(&at(&back, 6.0, 2.0), RIGHT_ARM));
        assert!(same(&at(&back, -6.0, 2.0), LEFT_ARM));

        // a quarter turn shows one arm in front of the body
        let side = model.render(std::f32::consts::FRAC_PI_2, 0.0, w, h);
        assert!(same(&at(&side, 0.0, 2.0), RIGHT_ARM));
        let other = model.render(-std::f32::consts::FRAC_PI_2, 0.0, w, h);
        assert!(same(&at(&other, 0.0, 2.0), LEFT_ARM));
    }

    #[test]
    fn slim_arms_are_a_pixel_narrower() {
        let png = encode_png(&skin()).unwrap();
        let (w, h) = (VIEW_WIDTH as u32 * 8, VIEW_HEIGHT as u32 * 8);
        let wide = PlayerModel::new(&png, false, None)
            .unwrap()
            .render(0.0, 0.0, w, h);
        let slim = PlayerModel::new(&png, true, None)
            .unwrap()
            .render(0.0, 0.0, w, h);
        assert!(same(&at(&wide, -7.5, 2.0), RIGHT_ARM));
        assert_eq!(at(&slim, -7.5, 2.0)[3], 0);
        assert!(same(&at(&slim, -6.5, 2.0), RIGHT_ARM));
    }

    #[test]
    fn the_second_layer_shows_only_where_it_is_painted() {
        let mut skin = skin();
        // a band of jacket across the chest
        let jacket = Rgba([250, 250, 250, 255]);
        for x in 20..28 {
            for y in 36..38 {
                skin.put_pixel(x, y, jacket);
            }
        }
        let model = PlayerModel::new(&encode_png(&skin).unwrap(), false, None).unwrap();
        let front = model.render(0.0, 0.0, VIEW_WIDTH as u32 * 8, VIEW_HEIGHT as u32 * 8);
        // the top two rows of the body are jacket, the rest the body itself
        assert!(same(&at(&front, 0.0, 7.0), jacket));
        assert!(same(&at(&front, 0.0, 2.0), BODY));
    }

    #[test]
    fn an_old_skin_mirrors_its_right_arm_and_leg() {
        let old = image::imageops::crop_imm(&skin(), 0, 0, 64, 32).to_image();
        let model = PlayerModel::new(&encode_png(&old).unwrap(), false, None).unwrap();
        let front = model.render(0.0, 0.0, VIEW_WIDTH as u32 * 8, VIEW_HEIGHT as u32 * 8);
        assert!(same(&at(&front, 6.0, 2.0), RIGHT_ARM));
        assert!(same(&at(&front, 2.0, -10.0), LEGS));
    }

    #[test]
    fn the_cape_hangs_behind() {
        let cape_colour = Rgba([230, 120, 30, 255]);
        let cape = encode_png(&sample_cape([230, 120, 30])).unwrap();
        let model = PlayerModel::new(&encode_png(&skin()).unwrap(), false, Some(&cape)).unwrap();
        let (w, h) = (VIEW_WIDTH as u32 * 8, VIEW_HEIGHT as u32 * 8);
        // from the front the body hides it
        let front = model.render(0.0, 0.0, w, h);
        assert!(same(&at(&front, 0.0, 2.0), BODY));
        // from behind it covers the back, wider than the body by a pixel each side
        let back = model.render(std::f32::consts::PI, 0.0, w, h);
        let seen = at(&back, 0.0, 6.5);
        assert_eq!(seen[3], 255);
        assert!(
            seen[0] > seen[2] + 40,
            "the cape isn't over the back: {seen:?} against {cape_colour:?}"
        );
        // a cape that isn't one is left off, and the player is still drawn
        let model = PlayerModel::new(&encode_png(&skin()).unwrap(), false, Some(b"junk")).unwrap();
        let back = model.render(std::f32::consts::PI, 0.0, w, h);
        assert!(same(&at(&back, 0.0, 6.5), BODY));

        // its picture: the 10×16 outside, enlarged
        let picture = cape_picture(&cape, 4).unwrap();
        assert_eq!(picture.dimensions(), (40, 64));
        assert_eq!(*picture.get_pixel(0, 0), cape_colour);
        assert!(cape_picture(b"junk", 4).is_err());
    }

    #[test]
    fn skins_of_the_wrong_shape_are_refused() {
        let wrong = encode_png(&RgbaImage::new(50, 50)).unwrap();
        assert!(PlayerModel::new(&wrong, false, None).is_err());
        assert!(PlayerModel::new(b"not a png", false, None).is_err());
        // an empty picture is drawn as nothing, not a crash
        let model = PlayerModel::new(&encode_png(&skin()).unwrap(), false, None).unwrap();
        assert_eq!(model.render(0.3, 0.1, 0, 0).dimensions(), (0, 0));
    }
}
