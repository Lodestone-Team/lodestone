//! Skin heads for the account box: the 8×8 face of a skin with its hat layer on
//! top, scaled up without smoothing and cached as a PNG in the data directory.
//!
//! Friends' heads come the same way, from the skin their Minecraft profile has on
//! Mojang's public session server ([`cached_profile_head`]): which skin a profile has
//! is cached for a day beside the heads, so the session server is asked about each
//! friend at most once a day.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use base64::Engine as _;
use http_client::HttpClient;
use image::{ImageFormat, ImageReader, Rgba, RgbaImage, imageops};
use url::Url;

use crate::{http, storage};

/// The rendered head is `8 * HEAD_SCALE` pixels square.
pub const HEAD_SCALE: u32 = 8;

/// Skins are 64×64 (or legacy 64×32) PNGs; refuse anything much larger.
const MAX_SKIN_SIDE: u32 = 1024;

#[derive(Debug, thiserror::Error)]
pub enum SkinError {
    #[error("the skin URL is not on the textures host")]
    UntrustedUrl,
    #[error("could not download the skin: {0}")]
    Download(String),
    #[error("the skin is not a valid skin image: {0}")]
    Invalid(String),
    #[error("could not cache the skin: {0}")]
    Cache(#[from] std::io::Error),
}

/// The host that profiles' skin URLs point at.
pub const TEXTURES_HOST: &str = "textures.minecraft.net";

/// The texture hash at the end of a skin URL, which names its cache file. `None`
/// if the URL is not on [`TEXTURES_HOST`] or the hash is not plain hex.
pub fn texture_hash(skin_url: &str) -> Option<String> {
    let url = Url::parse(skin_url).ok()?;
    if url.host_str() != Some(TEXTURES_HOST) || !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let hash = url.path_segments()?.next_back()?;
    (!hash.is_empty() && hash.len() <= 128 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}

/// The URL to download a skin from: its hash on the configured textures base
/// (`https://textures.minecraft.net` in production, a mock server in tests).
/// Profiles list `http://` URLs, so this also upgrades them to HTTPS.
fn download_url(textures: &Url, hash: &str) -> String {
    http::join(textures, &format!("texture/{hash}"))
}

/// Where the rendered head for `hash` is cached.
pub fn head_path(cache_dir: &Path, hash: &str) -> PathBuf {
    cache_dir.join(format!("{hash}.png"))
}

/// Returns the cached head for the skin, downloading and rendering it if needed.
/// Only skins on [`TEXTURES_HOST`] are fetched, from the `textures` base URL.
pub async fn cached_head(
    client: &dyn HttpClient,
    textures: &Url,
    cache_dir: &Path,
    skin_url: &str,
) -> Result<PathBuf, SkinError> {
    let hash = texture_hash(skin_url).ok_or(SkinError::UntrustedUrl)?;
    head_for_hash(client, textures, cache_dir, &hash).await
}

/// How long which skin a profile has is believed before the session server is asked
/// again.
pub const PROFILE_FRESH_FOR: Duration = Duration::from_secs(24 * 60 * 60);

/// Where which skin the profile `uuid` has is cached: its texture hash, or nothing
/// for a profile without a skin of its own.
fn profile_path(cache_dir: &Path, uuid: &[u8; 16]) -> PathBuf {
    cache_dir.join("profiles").join(hex_uuid(uuid))
}

fn hex_uuid(uuid: &[u8; 16]) -> String {
    uuid.iter().map(|b| format!("{b:02x}")).collect()
}

/// The skin URL of the Minecraft profile `uuid`, from the session server at
/// `sessions` (`https://sessionserver.mojang.com`): `None` when there's no such
/// profile, or it has no skin of its own (Steve or Alex).
pub async fn profile_skin_url(
    client: &dyn HttpClient,
    sessions: &Url,
    uuid: &[u8; 16],
) -> Result<Option<String>, SkinError> {
    let url = http::join(
        sessions,
        &format!("session/minecraft/profile/{}", hex_uuid(uuid)),
    );
    let request = http::get(&url, None).map_err(|err| SkinError::Download(err.to_string()))?;
    let response = http::send(client, request)
        .await
        .map_err(|err| SkinError::Download(err.to_string()))?;
    // no such profile: 204 with no body, or 404
    if response.status == 204 || response.status == 404 {
        return Ok(None);
    }
    if !response.is_success() {
        return Err(SkinError::Download(format!("HTTP {}", response.status)));
    }
    let invalid = |what: &str| SkinError::Invalid(format!("the profile's {what}"));
    let profile: serde_json::Value = response.json().map_err(|_| invalid("JSON"))?;
    let Some(textures) = profile["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "textures")
        .and_then(|p| p["value"].as_str())
    else {
        return Ok(None);
    };
    let textures = base64::engine::general_purpose::STANDARD
        .decode(textures)
        .map_err(|_| invalid("textures"))?;
    let textures: serde_json::Value =
        serde_json::from_slice(&textures).map_err(|_| invalid("textures"))?;
    Ok(textures["textures"]["SKIN"]["url"]
        .as_str()
        .map(str::to_string))
}

/// The cached head of the skin the Minecraft profile `uuid` has, asking the session
/// server which one that is at most once a [`PROFILE_FRESH_FOR`] (an answer from
/// before is used when it can't be asked). `None` for a profile with no skin of its
/// own, or none at all.
pub async fn cached_profile_head(
    client: &dyn HttpClient,
    textures: &Url,
    sessions: &Url,
    cache_dir: &Path,
    uuid: &[u8; 16],
) -> Result<Option<PathBuf>, SkinError> {
    let path = profile_path(cache_dir, uuid);
    let cached = std::fs::read_to_string(&path).ok();
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|at| SystemTime::now().duration_since(at).ok())
        .is_some_and(|age| age < PROFILE_FRESH_FOR);
    let hash = match cached {
        Some(hash) if fresh => hash,
        cached => match profile_skin_url(client, sessions, uuid).await {
            Ok(url) => {
                // a skin somewhere other than the textures host is never fetched
                let hash = url.as_deref().and_then(texture_hash).unwrap_or_default();
                storage::write_atomic(&path, hash.as_bytes())?;
                hash
            }
            Err(err) => match cached {
                Some(hash) => hash,
                None => return Err(err),
            },
        },
    };
    let hash = hash.trim();
    if hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || hash.len() > 128 {
        return Ok(None);
    }
    head_for_hash(client, textures, cache_dir, hash)
        .await
        .map(Some)
}

/// The cached head for the skin with texture hash `hash`, downloading and rendering
/// it from `textures` if needed.
async fn head_for_hash(
    client: &dyn HttpClient,
    textures: &Url,
    cache_dir: &Path,
    hash: &str,
) -> Result<PathBuf, SkinError> {
    let path = head_path(cache_dir, hash);
    if path.is_file() {
        return Ok(path);
    }
    let request = http::get(&download_url(textures, hash), None)
        .map_err(|err| SkinError::Download(err.to_string()))?;
    let response = http::send(client, request)
        .await
        .map_err(|err| SkinError::Download(err.to_string()))?;
    if !response.is_success() {
        return Err(SkinError::Download(format!("HTTP {}", response.status)));
    }
    let head = render_head(&response.body, HEAD_SCALE)?;
    storage::write_atomic(&path, &encode_png(&head)?)?;
    Ok(path)
}

/// Renders the head of a skin PNG: the face at (8, 8) with the hat at (40, 8)
/// composited over it, scaled by `scale` with nearest-neighbour sampling.
pub fn render_head(skin_png: &[u8], scale: u32) -> Result<RgbaImage, SkinError> {
    let mut reader = ImageReader::with_format(Cursor::new(skin_png), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SKIN_SIDE);
    limits.max_image_height = Some(MAX_SKIN_SIDE);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let skin = reader
        .decode()
        .map_err(|err| SkinError::Invalid(err.to_string()))?
        .to_rgba8();

    let (width, height) = skin.dimensions();
    let legacy = height * 2 == width;
    if width < 64 || width % 64 != 0 || !(height == width || legacy) {
        return Err(SkinError::Invalid(format!(
            "unexpected size {width}x{height}"
        )));
    }
    // HD skins scale every region by the same factor.
    let unit = width / 64;
    let face = imageops::crop_imm(&skin, 8 * unit, 8 * unit, 8 * unit, 8 * unit).to_image();
    let hat = imageops::crop_imm(&skin, 40 * unit, 8 * unit, 8 * unit, 8 * unit).to_image();

    let mut head = face;
    // Vanilla ignores a legacy skin's hat layer if it has no transparent pixel,
    // since old skins often filled that area with a solid colour.
    let ignore_hat = legacy && hat.pixels().all(|pixel| pixel[3] == 255);
    if !ignore_hat {
        for (dst, src) in head.pixels_mut().zip(hat.pixels()) {
            *dst = over(*src, *dst);
        }
    }
    // Nothing behind the face shows through.
    for pixel in head.pixels_mut() {
        pixel[3] = 255;
    }
    let side = 8 * scale;
    Ok(imageops::resize(
        &head,
        side,
        side,
        imageops::FilterType::Nearest,
    ))
}

/// `src` over `dst`, for straight (not premultiplied) alpha.
fn over(src: Rgba<u8>, dst: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as u32;
    if sa == 255 {
        return src;
    }
    if sa == 0 {
        return dst;
    }
    let da = dst[3] as u32;
    let out_a = sa * 255 + da * (255 - sa);
    let channel = |s: u8, d: u8| {
        ((s as u32 * sa * 255 + d as u32 * da * (255 - sa) + out_a / 2) / out_a) as u8
    };
    Rgba([
        channel(src[0], dst[0]),
        channel(src[1], dst[1]),
        channel(src[2], dst[2]),
        ((out_a + 127) / 255) as u8,
    ])
}

pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, SkinError> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| SkinError::Invalid(err.to_string()))?;
    Ok(bytes)
}

/// A made-up skin, with every face and hat pixel distinct, for tests and the
/// offscreen screenshots. `hat` controls whether the hat layer is drawn.
pub fn sample_skin(hat: bool) -> RgbaImage {
    let mut skin = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
    for y in 0..8 {
        for x in 0..8 {
            // A warm face with darker eyes and a mouth.
            let shade = 150 + (x * 7 + y * 5) as u8;
            let mut face = Rgba([shade, (shade as u32 * 3 / 4) as u8, 90, 255]);
            if y == 4 && (x == 2 || x == 5) {
                face = Rgba([40, 60, 140, 255]);
            }
            if y == 6 && (2..6).contains(&x) {
                face = Rgba([110, 50, 40, 255]);
            }
            skin.put_pixel(8 + x, 8 + y, face);
            if hat && y < 2 {
                // A cap over the top two rows.
                skin.put_pixel(40 + x, 8 + y, Rgba([30, 142, 178, 255]));
            }
        }
    }
    skin
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(image: &RgbaImage) -> Vec<u8> {
        encode_png(image).unwrap()
    }

    #[test]
    fn crops_the_face_and_composites_the_hat() {
        let skin = sample_skin(true);
        let head = render_head(&png(&skin), 1).unwrap();
        assert_eq!(head.dimensions(), (8, 8));
        // The hat covers the top rows; the face shows below.
        assert_eq!(*head.get_pixel(3, 0), Rgba([30, 142, 178, 255]));
        assert_eq!(*head.get_pixel(3, 5), *skin.get_pixel(8 + 3, 8 + 5));

        let scaled = render_head(&png(&skin), 4).unwrap();
        assert_eq!(scaled.dimensions(), (32, 32));
        // Nearest-neighbour: each skin pixel is a 4×4 block.
        for (dx, dy) in [(0, 0), (3, 3), (1, 2)] {
            assert_eq!(
                *scaled.get_pixel(3 * 4 + dx, 5 * 4 + dy),
                *head.get_pixel(3, 5)
            );
        }
    }

    #[test]
    fn blends_a_translucent_hat() {
        let mut skin = sample_skin(false);
        skin.put_pixel(40, 8, Rgba([255, 255, 255, 128]));
        let face = *skin.get_pixel(8, 8);
        let head = render_head(&png(&skin), 1).unwrap();
        let blended = head.get_pixel(0, 0);
        for channel in 0..3 {
            let expected = (255 * 128 + face[channel] as u32 * 127) / 255;
            assert!((blended[channel] as i32 - expected as i32).abs() <= 1);
        }
        assert_eq!(blended[3], 255);
    }

    #[test]
    fn ignores_an_opaque_legacy_hat() {
        let mut legacy = RgbaImage::from_pixel(64, 32, Rgba([0, 0, 0, 255]));
        for y in 0..8 {
            for x in 0..8 {
                legacy.put_pixel(8 + x, 8 + y, Rgba([200, 100, 50, 255]));
            }
        }
        let head = render_head(&png(&legacy), 1).unwrap();
        assert_eq!(*head.get_pixel(4, 4), Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn handles_hd_skins() {
        let skin = imageops::resize(&sample_skin(true), 128, 128, imageops::FilterType::Nearest);
        let head = render_head(&png(&skin), 1).unwrap();
        assert_eq!(head.dimensions(), (8, 8));
        assert_eq!(*head.get_pixel(3, 0), Rgba([30, 142, 178, 255]));
    }

    #[test]
    fn rejects_invalid_skins() {
        assert!(render_head(b"not a png", 1).is_err());
        let odd = png(&RgbaImage::new(30, 30));
        assert!(matches!(render_head(&odd, 1), Err(SkinError::Invalid(_))));
        let huge = png(&RgbaImage::new(2048, 2048));
        assert!(matches!(render_head(&huge, 1), Err(SkinError::Invalid(_))));
    }

    #[test]
    fn only_trusts_the_textures_host() {
        let textures = Url::parse("https://textures.minecraft.net").unwrap();
        assert_eq!(
            texture_hash("http://textures.minecraft.net/texture/1A2b3c").as_deref(),
            Some("1a2b3c")
        );
        for bad in [
            "https://evil.example/texture/abc",
            "https://textures.minecraft.net.evil.example/texture/abc",
            "https://textures.minecraft.net/texture/../../etc",
            "https://textures.minecraft.net/texture/not-hex",
            "https://textures.minecraft.net/",
            "file:///textures.minecraft.net/abc",
            "not a url",
        ] {
            assert_eq!(texture_hash(bad), None, "{bad}");
        }
        assert_eq!(
            download_url(&textures, "abc"),
            "https://textures.minecraft.net/texture/abc"
        );
    }
}
