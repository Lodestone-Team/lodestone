//! The player's skin and cape, changed through the Minecraft services: what the
//! account has ([`wardrobe`]), uploading a skin ([`upload_skin`]), going back to the
//! default one ([`reset_skin`]) and wearing or taking off a cape ([`set_cape`]).
//!
//! Every call takes the account's Minecraft access token and answers with the
//! wardrobe as it is afterwards.

use http_client::{HttpClient, Method};
use serde::Deserialize;
use serde_json::json;

use super::Endpoints;
use crate::{
    Secret,
    http::{self, RequestError, Response},
};

/// A skin file may not be larger than this. Mojang's own are a few kilobytes.
pub const MAX_SKIN_BYTES: usize = 1024 * 1024;

/// The shape of the player model a skin is drawn for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SkinModel {
    /// Arms 4 pixels wide ("Steve").
    #[default]
    Classic,
    /// Arms 3 pixels wide ("Alex").
    Slim,
}

impl SkinModel {
    pub const ALL: [SkinModel; 2] = [SkinModel::Classic, SkinModel::Slim];

    /// The name the services take when a skin is uploaded.
    fn variant(self) -> &'static str {
        match self {
            SkinModel::Classic => "classic",
            SkinModel::Slim => "slim",
        }
    }

    fn parse(variant: &str) -> Self {
        if variant.eq_ignore_ascii_case("slim") {
            SkinModel::Slim
        } else {
            SkinModel::Classic
        }
    }
}

/// A cape the account owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cape {
    pub id: String,
    /// Its name, such as "Migrator".
    pub name: String,
    pub url: String,
    pub active: bool,
}

/// What the player wears, and the capes they can pick from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wardrobe {
    /// The skin's texture. `None` with the default skin.
    pub skin_url: Option<String>,
    pub model: SkinModel,
    pub capes: Vec<Cape>,
}

impl Wardrobe {
    pub fn active_cape(&self) -> Option<&Cape> {
        self.capes.iter().find(|cape| cape.active)
    }
}

/// Why a skin or cape couldn't be read or changed. Never holds a token.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WardrobeError {
    #[error("couldn't reach the Minecraft services: {0}")]
    Network(String),
    #[error("the Minecraft services are busy; try again in a minute")]
    RateLimited,
    /// The token was refused: the account has to sign in again.
    #[error("the sign-in has expired")]
    SignedOut,
    /// The services said no, with their reason when they gave one.
    #[error("the Minecraft services refused ({status}){}", .message.as_ref().map(|m| format!(": {m}")).unwrap_or_default())]
    Refused {
        status: u16,
        message: Option<String>,
    },
}

/// Why a file can't be a skin.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SkinFileError {
    #[error("the file is larger than 1 MB")]
    TooLarge,
    #[error("the file isn't a PNG image")]
    NotPng,
    #[error("the image is {width}×{height}; a skin is 64×64 or 64×32")]
    WrongSize { width: u32, height: u32 },
}

/// Checks that `png` is a skin the services take: a PNG of 64×64, or 64×32 as skins
/// were before Minecraft 1.8.
pub fn check_skin_file(png: &[u8]) -> Result<(), SkinFileError> {
    if png.len() > MAX_SKIN_BYTES {
        return Err(SkinFileError::TooLarge);
    }
    let reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| SkinFileError::NotPng)?;
    if (width, height) == (64, 64) || (width, height) == (64, 32) {
        Ok(())
    } else {
        Err(SkinFileError::WrongSize { width, height })
    }
}

#[derive(Deserialize)]
struct ProfileBody {
    #[serde(default)]
    skins: Vec<SkinBody>,
    #[serde(default)]
    capes: Vec<CapeBody>,
}

#[derive(Deserialize)]
struct SkinBody {
    #[serde(default)]
    state: String,
    url: String,
    #[serde(default)]
    variant: String,
}

#[derive(Deserialize)]
struct CapeBody {
    id: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    alias: String,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default, rename = "errorMessage")]
    error_message: Option<String>,
}

fn from_profile(body: ProfileBody) -> Wardrobe {
    let skin = body.skins.iter().find(|skin| skin.state == "ACTIVE");
    Wardrobe {
        skin_url: skin.map(|skin| skin.url.clone()),
        model: skin.map_or(SkinModel::Classic, |skin| SkinModel::parse(&skin.variant)),
        capes: body
            .capes
            .into_iter()
            .map(|cape| Cape {
                active: cape.state == "ACTIVE",
                name: if cape.alias.is_empty() {
                    cape.id.clone()
                } else {
                    cape.alias
                },
                id: cape.id,
                url: cape.url,
            })
            .collect(),
    }
}

fn refused(response: &Response) -> WardrobeError {
    match response.status {
        401 => WardrobeError::SignedOut,
        429 => WardrobeError::RateLimited,
        status => WardrobeError::Refused {
            status,
            message: response
                .json::<ErrorBody>()
                .ok()
                .and_then(|body| body.error_message)
                .filter(|message| !message.trim().is_empty()),
        },
    }
}

fn network(error: impl std::fmt::Display) -> WardrobeError {
    WardrobeError::Network(error.to_string())
}

async fn send(
    client: &dyn HttpClient,
    request: anyhow::Result<http_client::Request<http_client::AsyncBody>>,
) -> Result<Response, WardrobeError> {
    let response = http::send(client, request.map_err(network)?)
        .await
        .map_err(|error| match error {
            RequestError::Transport(error) => network(error),
            RequestError::TooLarge => network("the answer was too large"),
        })?;
    if response.is_success() {
        Ok(response)
    } else {
        Err(refused(&response))
    }
}

/// What the account wears and owns.
pub async fn wardrobe(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &Secret,
) -> Result<Wardrobe, WardrobeError> {
    let url = http::join(&endpoints.minecraft, "minecraft/profile");
    let response = send(client, http::get(&url, Some(token))).await?;
    response
        .json()
        .map(from_profile)
        .map_err(|_| network("the profile couldn't be read"))
}

/// The wardrobe after a change: the services answer a change with the profile, and
/// are asked for it when they don't.
async fn after_change(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &Secret,
    response: Response,
) -> Result<Wardrobe, WardrobeError> {
    match response.json::<ProfileBody>() {
        Ok(body) if !response.body.is_empty() => Ok(from_profile(body)),
        _ => wardrobe(client, endpoints, token).await,
    }
}

/// A `multipart/form-data` body with the model and the skin file, and its content
/// type.
fn skin_form(png: &[u8], model: SkinModel) -> (String, Vec<u8>) {
    // a line that isn't in the file
    let boundary = (0u32..)
        .map(|n| format!("lodestone-skin-{n}"))
        .find(|candidate| {
            !png.windows(candidate.len())
                .any(|window| window == candidate.as_bytes())
        })
        .unwrap_or_default();
    let mut body = Vec::with_capacity(png.len() + 512);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{}\r\n",
            model.variant()
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// Uploads `png` as the account's skin, for `model`. The file is checked first
/// ([`check_skin_file`]); a file that fails is never sent.
pub async fn upload_skin(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &Secret,
    png: &[u8],
    model: SkinModel,
) -> Result<Wardrobe, WardrobeError> {
    check_skin_file(png).map_err(|error| WardrobeError::Refused {
        status: 400,
        message: Some(error.to_string()),
    })?;
    let url = http::join(&endpoints.minecraft, "minecraft/profile/skins");
    let (content_type, body) = skin_form(png, model);
    let request = http::with_body(Method::POST, &url, token, &content_type, body);
    let response = send(client, request).await?;
    after_change(client, endpoints, token, response).await
}

/// Goes back to the default skin.
pub async fn reset_skin(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &Secret,
) -> Result<Wardrobe, WardrobeError> {
    let url = http::join(&endpoints.minecraft, "minecraft/profile/skins/active");
    let request = http::without_body(Method::DELETE, &url, token);
    let response = send(client, request).await?;
    after_change(client, endpoints, token, response).await
}

/// Wears the cape `cape_id`, or none.
pub async fn set_cape(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &Secret,
    cape_id: Option<&str>,
) -> Result<Wardrobe, WardrobeError> {
    let url = http::join(&endpoints.minecraft, "minecraft/profile/capes/active");
    let request = match cape_id {
        Some(id) => http::with_body(
            Method::PUT,
            &url,
            token,
            "application/json",
            json!({ "capeId": id }).to_string().into_bytes(),
        ),
        None => http::without_body(Method::DELETE, &url, token),
    };
    let response = send(client, request).await?;
    after_change(client, endpoints, token, response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin;

    fn png(width: u32, height: u32) -> Vec<u8> {
        skin::encode_png(&image::RgbaImage::new(width, height)).unwrap()
    }

    #[test]
    fn only_skin_sized_pngs_pass() {
        assert_eq!(check_skin_file(&png(64, 64)), Ok(()));
        assert_eq!(check_skin_file(&png(64, 32)), Ok(()));
        assert_eq!(
            check_skin_file(&png(128, 128)),
            Err(SkinFileError::WrongSize {
                width: 128,
                height: 128
            })
        );
        assert_eq!(
            check_skin_file(&png(32, 64)),
            Err(SkinFileError::WrongSize {
                width: 32,
                height: 64
            })
        );
        assert_eq!(
            check_skin_file(b"GIF89a not a png"),
            Err(SkinFileError::NotPng)
        );
        assert_eq!(check_skin_file(b""), Err(SkinFileError::NotPng));
        assert_eq!(
            check_skin_file(&vec![0; MAX_SKIN_BYTES + 1]),
            Err(SkinFileError::TooLarge)
        );
    }

    #[test]
    fn the_upload_names_the_model_and_carries_the_file() {
        let file = png(64, 64);
        let (content_type, body) = skin_form(&file, SkinModel::Slim);
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\nslim\r\n"
        )));
        assert!(
            text.contains(
                "name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
        );
        assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
        // the file's bytes, whole
        assert!(body.windows(file.len()).any(|window| window == file));
    }

    #[test]
    fn the_profile_says_what_is_worn() {
        let body: ProfileBody = serde_json::from_value(json!({
            "id": "069a79f444e94726a5befca90e38aaf5",
            "name": "Notch",
            "skins": [
                {"id": "a", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/old", "variant": "CLASSIC"},
                {"id": "b", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/new", "variant": "SLIM"},
            ],
            "capes": [
                {"id": "c1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/cape1", "alias": "Migrator"},
                {"id": "c2", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/cape2", "alias": ""},
            ],
        }))
        .unwrap();
        let wardrobe = from_profile(body);
        assert_eq!(
            wardrobe.skin_url.as_deref(),
            Some("http://textures.minecraft.net/texture/new")
        );
        assert_eq!(wardrobe.model, SkinModel::Slim);
        assert_eq!(wardrobe.capes[0].name, "Migrator");
        assert_eq!(
            wardrobe.active_cape().map(|cape| cape.id.as_str()),
            Some("c2")
        );
        // a cape without a name goes by its id
        assert_eq!(wardrobe.capes[1].name, "c2");

        // the default skin: nothing active, the classic model
        let plain = from_profile(serde_json::from_value(json!({"skins": []})).unwrap());
        assert_eq!(plain, Wardrobe::default());
    }

    #[test]
    fn refusals_keep_the_services_reason() {
        let response = |status: u16, body: &str| Response {
            status,
            body: body.as_bytes().to_vec(),
        };
        assert_eq!(refused(&response(401, "")), WardrobeError::SignedOut);
        assert_eq!(refused(&response(429, "")), WardrobeError::RateLimited);
        assert_eq!(
            refused(&response(400, r#"{"errorMessage":"Invalid skin image"}"#)),
            WardrobeError::Refused {
                status: 400,
                message: Some("Invalid skin image".into())
            }
        );
        assert_eq!(
            refused(&response(500, "oops")).to_string(),
            "the Minecraft services refused (500)"
        );
    }
}
