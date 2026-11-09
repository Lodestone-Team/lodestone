//! Share codes: an instance's Modrinth content as one string another Lodestone
//! Launcher can recreate the instance from.
//!
//! A code is `LDS1:` and the base64url (no padding) of a deflated JSON payload with the
//! Minecraft version, the loader, an optional name, and every mod, resource pack and
//! shader pack that came from Modrinth as `project/version` (pinned to that exact
//! version). A `-` in front marks a mod or pack that is turned off; resource packs are
//! listed highest priority first, as the game lists them, and the shader pack in use
//! has a `*` in front.
//!
//! Only content Modrinth serves goes in: files added by hand, local folders and
//! CurseForge-only files are left out (see [`SharePlan::left_out`]), and so are
//! worlds, configs, options, accounts and paths. A code is untrusted input: it is
//! limited in size before and after it is inflated, its fields are checked against a
//! strict shape, and installing it downloads only through the usual Modrinth path,
//! which checks every file's hash.

use std::io::{Read, Write};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{
    ContentError, ContentKind, InstanceDirs, Modrinth, Target, files,
    install::{self, InstallPlan, InstallProgress, PlannedFile},
    listing, options, packs,
    provenance::Provenance,
    scan,
};
use crate::game::download::{CancelToken, Downloader};
use crate::instances::Loader;

/// What every share code starts with; the digit is the format's version.
pub const PREFIX: &str = "LDS1:";

/// The longest code accepted, in characters.
pub const MAX_CODE_CHARS: usize = 256 * 1024;
/// The largest payload a code may inflate to.
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
/// The most items a code may list.
pub const MAX_ITEMS: usize = 5000;
const MAX_NAME_CHARS: usize = 100;

/// Why a code can't be read.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShareError {
    #[error("That isn't a Lodestone share code.")]
    NotACode,
    #[error("This share code is from a newer Lodestone Launcher.")]
    NewerFormat,
    #[error("The share code is too long.")]
    TooLarge,
    #[error("The share code is damaged: {0}")]
    Damaged(String),
}

/// One piece of content, pinned to a Modrinth version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedItem {
    pub kind: ContentKind,
    pub project: String,
    pub version: String,
    /// A mod or resource pack that is on, or the shader pack in use.
    pub enabled: bool,
}

/// What a share code holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareCode {
    pub name: Option<String>,
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// Mods, then resource packs (highest priority first), then shader packs.
    pub items: Vec<SharedItem>,
}

/// The JSON inside a code, with short keys.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    g: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    l: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lv: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    m: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    r: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    s: Vec<String>,
}

impl ShareCode {
    /// How many items of `kind` it lists.
    pub fn count(&self, kind: ContentKind) -> usize {
        self.items.iter().filter(|i| i.kind == kind).count()
    }

    pub fn encode(&self) -> String {
        let entries = |kind: ContentKind| -> Vec<String> {
            self.items
                .iter()
                .filter(|i| i.kind == kind)
                .map(|i| {
                    let mark = match (kind, i.enabled) {
                        (ContentKind::ShaderPack, true) => "*",
                        (ContentKind::ShaderPack, false) => "",
                        (_, true) => "",
                        (_, false) => "-",
                    };
                    format!("{mark}{}/{}", i.project, i.version)
                })
                .collect()
        };
        let payload = Payload {
            v: 1,
            n: self.name.clone(),
            g: self.game_version.clone(),
            l: loader_key(self.loader).map(str::to_string),
            lv: self.loader_version.clone(),
            m: entries(ContentKind::Mod),
            r: entries(ContentKind::ResourcePack),
            s: entries(ContentKind::ShaderPack),
        };
        let json = serde_json::to_vec(&payload).unwrap_or_default();
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        let _ = encoder.write_all(&json);
        let deflated = encoder.finish().unwrap_or_default();
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(deflated)
        )
    }

    /// Reads a code, checking everything in it. Whitespace around it (and inside, as
    /// when it was wrapped in a message) is ignored.
    pub fn decode(code: &str) -> Result<Self, ShareError> {
        if code.len() > MAX_CODE_CHARS {
            return Err(ShareError::TooLarge);
        }
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let Some((head, body)) = code.split_once(':') else {
            return Err(ShareError::NotACode);
        };
        match head {
            "LDS1" => {}
            other if other.starts_with("LDS") && other[3..].parse::<u32>().is_ok() => {
                return Err(ShareError::NewerFormat);
            }
            _ => return Err(ShareError::NotACode),
        }
        let deflated = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| ShareError::Damaged("it isn't valid base64".into()))?;
        let mut json = Vec::new();
        flate2::read::DeflateDecoder::new(deflated.as_slice())
            .take(MAX_PAYLOAD_BYTES as u64 + 1)
            .read_to_end(&mut json)
            .map_err(|_| ShareError::Damaged("it can't be unpacked".into()))?;
        if json.len() > MAX_PAYLOAD_BYTES {
            return Err(ShareError::TooLarge);
        }
        let payload: Payload =
            serde_json::from_slice(&json).map_err(|err| ShareError::Damaged(err.to_string()))?;
        if payload.v != 1 {
            return Err(ShareError::NewerFormat);
        }
        Self::from_payload(payload)
    }

    fn from_payload(payload: Payload) -> Result<Self, ShareError> {
        let damaged = |what: &str| ShareError::Damaged(what.to_string());
        if !is_version_text(&payload.g) {
            return Err(damaged("the Minecraft version"));
        }
        let loader = match payload.l.as_deref() {
            None => Loader::None,
            Some(key) => loader_from_key(key).ok_or_else(|| damaged("the loader"))?,
        };
        if let Some(version) = &payload.lv
            && !is_version_text(version)
        {
            return Err(damaged("the loader version"));
        }
        if loader != Loader::None && payload.lv.is_none() {
            return Err(damaged("the loader version"));
        }
        let name = match payload.n {
            Some(name) => {
                let name: String = name.chars().filter(|c| !c.is_control()).collect();
                let name = name.trim().to_string();
                if name.chars().count() > MAX_NAME_CHARS {
                    return Err(damaged("the name"));
                }
                (!name.is_empty()).then_some(name)
            }
            None => None,
        };
        let total = payload.m.len() + payload.r.len() + payload.s.len();
        if total > MAX_ITEMS {
            return Err(ShareError::TooLarge);
        }
        let mut items = Vec::with_capacity(total);
        for (kind, list) in [
            (ContentKind::Mod, payload.m),
            (ContentKind::ResourcePack, payload.r),
            (ContentKind::ShaderPack, payload.s),
        ] {
            for entry in list {
                let (enabled, rest) = match (kind, entry.as_bytes().first()) {
                    (ContentKind::ShaderPack, Some(b'*')) => (true, &entry[1..]),
                    (ContentKind::ShaderPack, _) => (false, entry.as_str()),
                    (_, Some(b'-')) => (false, &entry[1..]),
                    _ => (true, entry.as_str()),
                };
                let (project, version) = rest
                    .split_once('/')
                    .filter(|(p, v)| is_modrinth_id(p) && is_modrinth_id(v))
                    .ok_or_else(|| damaged("an item"))?;
                items.push(SharedItem {
                    kind,
                    project: project.to_string(),
                    version: version.to_string(),
                    enabled,
                });
            }
        }
        if items
            .iter()
            .filter(|i| i.kind == ContentKind::ShaderPack && i.enabled)
            .count()
            > 1
        {
            return Err(damaged("more than one shader pack in use"));
        }
        Ok(Self {
            name,
            game_version: payload.g,
            loader,
            loader_version: payload.lv,
            items,
        })
    }
}

/// A Modrinth project or version id: short; letters and digits, and `-` or `_` after
/// the first character. Never a path.
fn is_modrinth_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A Minecraft or loader version: short, without separators or anything odd.
fn is_version_text(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+ ".contains(&b))
}

fn loader_key(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::None => None,
        Loader::Fabric => Some("fabric"),
        Loader::Quilt => Some("quilt"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    }
}

fn loader_from_key(key: &str) -> Option<Loader> {
    Some(match key {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        _ => return None,
    })
}

/// Why a file isn't in a share code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOutReason {
    /// Not from Modrinth: added by hand, or a folder.
    NotOnModrinth,
    /// From CurseForge, which a share code can't download from.
    CurseForge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    pub kind: ContentKind,
    pub file_name: String,
    pub reason: LeftOutReason,
}

/// A code for an instance, and what it leaves out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharePlan {
    pub code: ShareCode,
    pub left_out: Vec<LeftOut>,
}

/// The share code for the instance in `dirs`. With `api`, files not matched to
/// Modrinth yet are looked up first, so they aren't left out by mistake.
pub async fn collect(
    api: Option<&Modrinth>,
    dirs: &InstanceDirs,
    target: &Target,
    name: Option<String>,
    loader: Loader,
    loader_version: Option<String>,
) -> Result<SharePlan, ContentError> {
    let mut items = Vec::new();
    let mut left_out = Vec::new();
    for kind in ContentKind::ALL {
        let (files, provenance, _) = listing::refresh(api, dirs, kind).await?;
        let enabled_packs = if kind == ContentKind::ResourcePack {
            options::PackOptions::read(&dirs.game)
                .map(|o| o.enabled_files(&target.game_version))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let shader = if kind == ContentKind::ShaderPack {
            listing::shader_state(dirs)?.selection.pack
        } else {
            None
        };
        let mut found = Vec::new();
        for file in &files {
            let entry = provenance.current(file);
            let source = entry.and_then(|e| e.modrinth.as_ref());
            let Some(source) = source.filter(|_| !file.is_dir) else {
                left_out.push(LeftOut {
                    kind,
                    file_name: file.file_name.clone(),
                    reason: if entry.is_some_and(|e| e.curseforge.is_some()) {
                        LeftOutReason::CurseForge
                    } else {
                        LeftOutReason::NotOnModrinth
                    },
                });
                continue;
            };
            let enabled = match kind {
                ContentKind::Mod => file.enabled_by_name(),
                ContentKind::ResourcePack => enabled_packs.contains(&file.file_name),
                ContentKind::ShaderPack => shader.as_deref() == Some(file.file_name.as_str()),
            };
            found.push((
                file.file_name.clone(),
                SharedItem {
                    kind,
                    project: source.project_id.clone(),
                    version: source.version_id.clone(),
                    enabled,
                },
            ));
        }
        if kind == ContentKind::ResourcePack {
            // Enabled packs in the game's order (highest priority first), then the rest.
            let rank = |name: &str| {
                enabled_packs
                    .iter()
                    .position(|n| n == name)
                    .unwrap_or(usize::MAX)
            };
            found.sort_by_key(|(name, _)| rank(name));
        }
        items.extend(found.into_iter().map(|(_, item)| item));
    }
    Ok(SharePlan {
        code: ShareCode {
            name,
            game_version: target.game_version.clone(),
            loader,
            loader_version: (loader != Loader::None).then_some(loader_version).flatten(),
            items,
        },
        left_out,
    })
}

/// What installing a code did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShareOutcome {
    /// Titles of what was installed.
    pub installed: Vec<String>,
    /// What couldn't be installed: a title (or the project id) and why.
    pub failed: Vec<(String, String)>,
}

/// Installs the content of `code` into the (new) instance in `dirs`: every listed
/// version exactly, then turns off what was off, orders the resource packs and picks
/// the shader pack. Items Modrinth no longer has are reported, not fatal.
pub async fn install(
    api: &Modrinth,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    code: &ShareCode,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<ShareOutcome, ContentError> {
    let mut outcome = ShareOutcome::default();
    let ids: Vec<String> = code.items.iter().map(|i| i.version.clone()).collect();
    let versions = api.versions_by_id(&ids).await?;
    let project_ids: Vec<String> = {
        let mut ids: Vec<String> = code.items.iter().map(|i| i.project.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let projects = api.projects(&project_ids).await.unwrap_or_default();
    let mut planned = Vec::new();
    for item in &code.items {
        let project = projects.iter().find(|p| p.id == item.project).cloned();
        let title = project
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| item.project.clone());
        match versions
            .iter()
            .find(|v| v.id == item.version && v.project_id == item.project)
        {
            Some(version) => planned.push(PlannedFile {
                kind: item.kind,
                version: version.clone(),
                project,
                depth: 0,
            }),
            None => outcome
                .failed
                .push((title, "No longer on Modrinth".to_string())),
        }
    }
    let result = install::execute(
        api,
        downloader,
        dirs,
        target,
        InstallPlan {
            files: planned,
            skipped: Vec::new(),
        },
        progress,
        cancel,
    )
    .await?;
    outcome.installed = result
        .installed
        .into_iter()
        .map(|(title, _)| title)
        .collect();
    outcome
        .failed
        .extend(result.skipped.into_iter().map(|s| (s.project, s.reason)));
    apply_states(dirs, target, code)?;
    Ok(outcome)
}

/// Turns off the mods and packs that were off, enables the packs in order and picks
/// the shader pack, by the files the install recorded for each project.
fn apply_states(
    dirs: &InstanceDirs,
    target: &Target,
    code: &ShareCode,
) -> Result<(), ContentError> {
    let provenance = Provenance::load(&dirs.instance);
    let file_of = |kind: ContentKind, project: &str| -> Option<String> {
        provenance
            .by_project(kind)
            .remove(project)
            .and_then(|names| names.into_iter().next())
    };
    for item in code
        .items
        .iter()
        .filter(|i| i.kind == ContentKind::Mod && !i.enabled)
    {
        if let Some(name) = file_of(ContentKind::Mod, &item.project) {
            files::set_mod_enabled(dirs, &name, false)?;
        }
    }
    // Enabling puts a pack on top, so the lowest priority goes first.
    let packs_scanned = scan::scan(dirs, ContentKind::ResourcePack)?;
    for item in code
        .items
        .iter()
        .filter(|i| i.kind == ContentKind::ResourcePack && i.enabled)
        .rev()
    {
        if let Some(name) = file_of(ContentKind::ResourcePack, &item.project) {
            let formats = packs_scanned
                .iter()
                .find(|f| f.file_name == name)
                .and_then(|f| f.meta.formats);
            let compatibility = packs::compatibility(formats.as_ref(), &target.game_version);
            files::set_pack_enabled(dirs, target, &name, true, compatibility)?;
        }
    }
    if let Some(item) = code
        .items
        .iter()
        .find(|i| i.kind == ContentKind::ShaderPack && i.enabled)
        && let Some(name) = file_of(ContentKind::ShaderPack, &item.project)
        && let Some(loader) = listing::shader_state(dirs)?.loader
    {
        files::select_shader(dirs, loader, Some(&name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ShareCode {
        ShareCode {
            name: Some("Survival".into()),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.14".into()),
            items: vec![
                SharedItem {
                    kind: ContentKind::Mod,
                    project: "AANobbMI".into(),
                    version: "rAfhHfow".into(),
                    enabled: true,
                },
                SharedItem {
                    kind: ContentKind::Mod,
                    project: "P7dR8mSH".into(),
                    version: "fapi0922".into(),
                    enabled: false,
                },
                SharedItem {
                    kind: ContentKind::ResourcePack,
                    project: "faithful".into(),
                    version: "f32v1201".into(),
                    enabled: true,
                },
                SharedItem {
                    kind: ContentKind::ShaderPack,
                    project: "complemen".into(),
                    version: "comp2500".into(),
                    enabled: true,
                },
            ],
        }
    }

    fn code_of(json: &str) -> String {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(json.as_bytes()).unwrap();
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(encoder.finish().unwrap())
        )
    }

    #[test]
    fn round_trips() {
        let code = sample();
        let text = code.encode();
        assert!(text.starts_with(PREFIX));
        assert!(
            text[PREFIX.len()..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "URL-safe: {text}"
        );
        assert_eq!(ShareCode::decode(&text).unwrap(), code);
        // wrapped in a message
        let wrapped = format!("  {}\n{}  ", &text[..20], &text[20..]);
        assert_eq!(ShareCode::decode(&wrapped).unwrap(), code);
    }

    #[test]
    fn vanilla_without_loader() {
        let code = ShareCode {
            name: None,
            game_version: "1.21.4".into(),
            loader: Loader::None,
            loader_version: None,
            items: Vec::new(),
        };
        assert_eq!(ShareCode::decode(&code.encode()).unwrap(), code);
    }

    #[test]
    fn a_large_instance_stays_a_few_kilobytes() {
        // 308 mods with random 8-character ids, like a big modpack
        let mut seed = 7u64;
        let mut id = || {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            (0..8)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    CHARS[(seed >> 33) as usize % CHARS.len()] as char
                })
                .collect::<String>()
        };
        let mut code = sample();
        code.items = (0..308)
            .map(|i| SharedItem {
                kind: ContentKind::Mod,
                project: id(),
                version: id(),
                enabled: i % 20 != 0,
            })
            .collect();
        let text = code.encode();
        assert!(text.len() < 8 * 1024, "{} chars", text.len());
        assert_eq!(ShareCode::decode(&text).unwrap(), code);
    }

    #[test]
    fn rejects_what_isnt_a_code() {
        assert_eq!(ShareCode::decode("hello"), Err(ShareError::NotACode));
        assert_eq!(ShareCode::decode("ABC:xyz"), Err(ShareError::NotACode));
        assert_eq!(ShareCode::decode("LDS2:abc"), Err(ShareError::NewerFormat));
        assert!(matches!(
            ShareCode::decode("LDS1:not*base64"),
            Err(ShareError::Damaged(_))
        ));
        assert!(matches!(
            ShareCode::decode("LDS1:AAAA"),
            Err(ShareError::Damaged(_))
        ));
        let too_long = format!("{PREFIX}{}", "A".repeat(MAX_CODE_CHARS));
        assert_eq!(ShareCode::decode(&too_long), Err(ShareError::TooLarge));
    }

    #[test]
    fn rejects_a_zip_bomb() {
        // a tiny code that inflates past the limit
        let json = format!(
            r#"{{"v":1,"g":"1.20.1","n":"{}"}}"#,
            " ".repeat(2 * MAX_PAYLOAD_BYTES)
        );
        let code = code_of(&json);
        assert!(code.len() < 20_000, "{}", code.len());
        assert_eq!(ShareCode::decode(&code), Err(ShareError::TooLarge));
    }

    #[test]
    fn rejects_bad_fields() {
        for json in [
            r#"{"v":2,"g":"1.20.1"}"#,
            r#"{"v":1,"g":"../../etc"}"#,
            r#"{"v":1,"g":"1.20.1","l":"rift","lv":"1"}"#,
            r#"{"v":1,"g":"1.20.1","l":"fabric"}"#,
            r#"{"v":1,"g":"1.20.1","m":["../evil/x"]}"#,
            r#"{"v":1,"g":"1.20.1","m":["only-one-part"]}"#,
            r#"{"v":1,"g":"1.20.1","s":["*aaaa/bbbb","*cccc/dddd"]}"#,
            r#"{"v":1,"g":"1.20.1","extra":true}"#,
        ] {
            assert!(ShareCode::decode(&code_of(json)).is_err(), "{json}");
        }
        assert_eq!(
            ShareCode::decode(&code_of(r#"{"v":2,"g":"1.20.1"}"#)),
            Err(ShareError::NewerFormat)
        );
    }

    #[test]
    fn rejects_too_many_items() {
        let items: Vec<String> = (0..=MAX_ITEMS).map(|i| format!("\"p{i}/v{i}\"")).collect();
        let json = format!(r#"{{"v":1,"g":"1.20.1","m":[{}]}}"#, items.join(","));
        assert_eq!(
            ShareCode::decode(&code_of(&json)),
            Err(ShareError::TooLarge)
        );
    }
}
