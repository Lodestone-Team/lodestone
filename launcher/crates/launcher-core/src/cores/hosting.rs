//! "Host this modpack": a launcher instance becomes a server on a Lodestone core.
//!
//! 1. The instance is exported as a `.mrpack` ([`crate::export`]), with its Modrinth
//!    files by URL and their `env` from Modrinth, so the core leaves out client-only
//!    mods (`env.server: unsupported`).
//! 2. The user signs in once: the device key can only view, start and stop servers, so
//!    these calls use a session token from `POST /user/login`, which is dropped after.
//! 3. A free game port is found with the core's port manager (`/check/port/{port}`).
//! 4. The pack goes to `POST /modpack/upload`, then `POST /instance/create_modpack`
//!    makes the server, with `auto_start: false`. The server is never started here:
//!    its owner accepts the Minecraft EULA by creating it, and starts it themselves.
//! 5. The launcher instance is linked to the new server ([`ServerLink`]).

use std::{fs, path::Path};

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::{
    CoreClient, CoreError,
    client::{Auth, MAX_JSON_BYTES, check, segment},
    models::LoginReply,
    pack::{LinkedFile, ServerLink},
};
use crate::{
    Secret,
    export::ExportPlan,
    game::{download::sha1_hex, loaders::LoaderKind},
    instances::Instance,
};

/// The core's own limit for an uploaded pack.
pub const MAX_UPLOAD_BYTES: u64 = lodestone_minecraft::mrpack::MAX_PACK_FILE_BYTES;
/// The first game port tried, and how many after it.
pub const FIRST_PORT: u16 = 25565;
pub const PORTS_TRIED: u16 = 100;
/// The server's memory, in MB, unless the user picks other values.
pub const DEFAULT_MIN_RAM: u32 = 1024;
pub const DEFAULT_MAX_RAM: u32 = 4096;
/// How long an upload may take.
pub const UPLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Whether a launcher instance can be hosted: the core makes servers from Fabric
/// packs only.
pub fn check_hostable(instance: &Instance) -> Result<(), String> {
    match instance.loader.kind() {
        Some(LoaderKind::Fabric) => Ok(()),
        Some(kind) => Err(format!(
            "Lodestone Core can host Fabric modpacks only, and this instance uses {}.",
            kind.label()
        )),
        None => Err(
            "Lodestone Core can host Fabric modpacks only. Create a vanilla server in the dashboard instead."
                .into(),
        ),
    }
}

/// What the core says about an uploaded pack.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadedPack {
    pub name: String,
    pub game_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub server_file_count: u32,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
    #[serde(default)]
    pub upload_id: Option<String>,
}

/// The new server's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSettings {
    pub name: String,
    pub port: u16,
    /// In MB.
    pub min_ram: u32,
    pub max_ram: u32,
}

#[derive(Deserialize)]
struct PortStatus {
    is_in_use: bool,
    is_allocated: bool,
}

/// A `multipart/form-data` body with one `file` field, as the dashboard sends it.
/// Returns the body and its content type.
pub fn multipart_file(file_name: &str, bytes: &[u8]) -> (Vec<u8>, String) {
    let boundary = format!(
        "lodestone-launcher-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    );
    // the name is the pack's, but quotes and line breaks would end the header
    let safe_name: String = file_name
        .chars()
        .map(|c| if c == '"' || c.is_control() { '_' } else { c })
        .collect();
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe_name}\"\r\nContent-Type: application/x-modrinth-modpack+zip\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

impl CoreClient {
    /// Signs in for a few requests: the session token, which the caller drops when done.
    /// It is never stored.
    pub async fn session(&self, username: &str, password: &Secret) -> Result<Secret, CoreError> {
        let login: LoginReply = self
            .json(
                Method::POST,
                "user/login",
                Auth::Basic(username, password),
                None,
            )
            .await?;
        Ok(Secret::new(login.token))
    }

    /// The first game port from `first` that is neither in use on the core's machine nor
    /// given to another server.
    pub async fn free_port(&self, session: &Secret, first: u16) -> Result<u16, CoreError> {
        for port in first..first.saturating_add(PORTS_TRIED) {
            let status: PortStatus = self
                .json(
                    Method::GET,
                    &format!("check/port/{port}"),
                    Auth::Bearer(session),
                    None,
                )
                .await?;
            if !status.is_in_use && !status.is_allocated {
                return Ok(port);
            }
        }
        Err(CoreError::Server {
            status: 409,
            message: format!(
                "No free game port between {first} and {}.",
                first.saturating_add(PORTS_TRIED - 1)
            ),
        })
    }

    /// Uploads a `.mrpack` to the core, which reads and keeps it for an hour.
    pub async fn upload_pack(
        &self,
        session: &Secret,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<UploadedPack, CoreError> {
        let (body, content_type) = multipart_file(file_name, bytes);
        let response = self
            .send_typed(
                Method::POST,
                "modpack/upload",
                Auth::Bearer(session),
                Some((body, content_type)),
                None,
                MAX_JSON_BYTES,
                UPLOAD_TIMEOUT,
            )
            .await?;
        check(&response, Auth::Bearer(session))?;
        serde_json::from_slice(&response.body).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// Creates a server from an uploaded pack. The core sets it up in the background
    /// and never starts it. Returns the new server's id.
    pub async fn create_server(
        &self,
        session: &Secret,
        upload_id: &str,
        settings: &HostSettings,
    ) -> Result<String, CoreError> {
        let body = serde_json::json!({
            "source": { "type": "upload", "upload_id": upload_id },
            "name": settings.name,
            "port": settings.port,
            "min_ram": settings.min_ram,
            "max_ram": settings.max_ram,
            "auto_start": false,
            "restart_on_crash": false,
        });
        let uuid: String = self
            .json(
                Method::POST,
                "instance/create_modpack",
                Auth::Bearer(session),
                Some(body.to_string().into_bytes()),
            )
            .await?;
        segment(&uuid)?;
        Ok(uuid)
    }
}

/// The mods the server gets from the pack, as the link records them: the instance's
/// jars in `mods/` that the pack lists or carries, except client-only ones. When the
/// instance later plays on the server, its client pack (a part of these) is then in
/// step without downloading anything again.
pub fn linked_files(plan: &ExportPlan) -> Vec<LinkedFile> {
    let mut files: Vec<LinkedFile> = plan
        .index
        .files
        .iter()
        .filter(|f| f.env.server != "unsupported")
        .filter_map(|f| {
            let name = f.path.strip_prefix("mods/")?;
            (!name.contains('/')).then(|| LinkedFile {
                name: name.to_string(),
                sha1: f.hashes.sha1.to_ascii_lowercase(),
            })
        })
        .collect();
    for (zip_path, disk) in &plan.overrides {
        let Some(name) = zip_path.strip_prefix("overrides/mods/") else {
            continue;
        };
        if name.contains('/') || !name.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        if let Ok(bytes) = fs::read(disk) {
            files.push(LinkedFile {
                name: name.to_string(),
                sha1: sha1_hex(&bytes),
            });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files.dedup_by(|a, b| a.name == b.name);
    files
}

/// The link from the hosted instance to its new server.
pub fn host_link(
    core_uuid: &str,
    core_url: &str,
    server_uuid: &str,
    server_name: &str,
    plan: &ExportPlan,
) -> ServerLink {
    ServerLink {
        core_uuid: core_uuid.to_string(),
        core_url: core_url.to_string(),
        server_uuid: server_uuid.to_string(),
        server_name: server_name.to_string(),
        pack_etag: None,
        files: linked_files(plan),
    }
}

/// Reads an exported pack for the upload, refusing one larger than the core takes.
pub fn read_pack_file(path: &Path) -> Result<Vec<u8>, String> {
    let size = fs::metadata(path)
        .map_err(|err| format!("{}: {err}", path.display()))?
        .len();
    if size > MAX_UPLOAD_BYTES {
        return Err(format!(
            "The modpack is {} MB; Lodestone Core takes packs up to {} MB. Remove large files from the instance first.",
            size / (1024 * 1024),
            MAX_UPLOAD_BYTES / (1024 * 1024)
        ));
    }
    fs::read(path).map_err(|err| format!("{}: {err}", path.display()))
}
