use std::ops::Deref;
use std::path::{Component, Path as FsPath, PathBuf};

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use axum_auth::AuthBearer;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use color_eyre::eyre::{eyre, Context};

use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::CausedBy,
    global_settings::CodeExecutionSwitch,
    prelude::GameInstance,
    types::InstanceUuid,
    AppState,
};

pub fn parse_bearer_token(token: &str) -> Option<String> {
    let mut split = token.split_ascii_whitespace();
    if split.next()? != "Bearer" {
        return None;
    }
    split.next().map(|s| s.to_string())
}

pub fn decode_base64(input: &str) -> Result<String, Error> {
    Ok(String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(input)
            .context("Failed to decode base64")?,
    )
    .context("Invalid UTF-8")?)
}

fn unauthorized() -> Error {
    Error {
        kind: ErrorKind::Unauthorized,
        source: eyre!("Unauthorized"),
    }
}

/// Returns the user that `token` (a bare session JWT or API key) belongs to.
pub async fn authenticate(state: &AppState, token: &str) -> Result<User, Error> {
    state.users_manager.read().await.try_auth_or_err(token)
}

/// Returns the user for a WebSocket `token` query parameter, which has the form
/// `Bearer <token>` because browsers cannot set headers on WebSocket requests.
pub async fn authenticate_query_token(state: &AppState, token: &str) -> Result<User, Error> {
    let token = parse_bearer_token(token).ok_or_else(unauthorized)?;
    authenticate(state, &token).await
}

/// Checks that `user` may perform `action`, taking safe mode into account.
pub async fn require(state: &AppState, user: &User, action: UserAction) -> Result<(), Error> {
    user.try_action(&action, state.safe_mode())
}

/// Fails with 403 while the code execution kill switch is off.
pub fn require_code_execution(state: &AppState) -> Result<(), Error> {
    state.code_execution.check()
}

/// Extensions of files that Deno (macros, instance scripts, atoms) or a plugin runtime may
/// run. Only the owner may create them, and only while code execution is on.
pub const SCRIPT_EXTENSIONS: [&str; 7] = ["ts", "tsx", "js", "jsx", "mjs", "cjs", "lua"];

/// The folder of an instance that holds its macros and scripts (such as `prelaunch.ts`).
pub const MACROS_DIR_NAME: &str = "macros";

/// A file or folder name the way Windows and macOS resolve it: ignoring case and trailing
/// dots and spaces. `None` if the name is not valid UTF-8.
fn normalized_name(name: &std::ffi::OsStr) -> Option<String> {
    name.to_str()
        .map(|name| name.trim_end_matches(['.', ' ']).to_lowercase())
}

/// Whether the file at `path` is a script, judging by its name: a [`SCRIPT_EXTENSIONS`]
/// extension in any case, ignoring trailing dots and spaces. Names that cannot be checked,
/// and names with `:` (Windows alternate data streams such as `run.ts::$DATA` write to the
/// file itself), count as scripts.
pub fn is_script_path(path: &FsPath) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    let Some(name) = normalized_name(name) else {
        return true;
    };
    if name.contains(':') {
        return true;
    }
    FsPath::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SCRIPT_EXTENSIONS.contains(&ext))
}

/// Whether `relative_path`, relative to an instance's root, is the instance's `macros`
/// folder or anything in it. The first component is compared like [`is_script_path`]
/// compares names; one that cannot be checked counts as the macros folder.
pub fn is_in_macros_dir(relative_path: &FsPath) -> bool {
    let first = relative_path
        .components()
        .find_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        });
    let Some(first) = first else {
        return false;
    };
    match normalized_name(first) {
        Some(name) => name == MACROS_DIR_NAME || name.contains(':'),
        None => true,
    }
}

/// Whether creating or changing `relative_path` (relative to an instance's root) creates or
/// changes code that Lodestone may run: a script file, or anything in the macros folder.
pub fn is_code_path(relative_path: &FsPath, is_dir: bool) -> bool {
    is_in_macros_dir(relative_path) || (!is_dir && is_script_path(relative_path))
}

/// Whether `dir` contains a script file at any depth. Runs on a blocking thread and does
/// not follow symbolic links.
pub async fn contains_script(dir: PathBuf) -> Result<bool, Error> {
    tokio::task::spawn_blocking(move || {
        for entry in walkdir::WalkDir::new(&dir).min_depth(1) {
            let entry = entry.context("Failed to walk directory while scanning for scripts")?;
            if !entry.file_type().is_dir() && is_script_path(entry.path()) {
                return Ok(true);
            }
        }
        Ok(false)
    })
    .await
    .context("Failed to scan directory for scripts")?
}

/// The rule for creating or changing code files (see [`is_code_path`]): only the owner, and
/// only while code execution is on, whatever permissions or safe mode setting apply.
pub fn check_code_write(is_owner: bool, code_execution: &CodeExecutionSwitch) -> Result<(), Error> {
    if !is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "Only the owner can create or change script files or anything in a macros folder"
            ),
        });
    }
    code_execution.check()
}

/// [`check_code_write`] for `user`.
pub fn require_code_write(state: &AppState, user: &User) -> Result<(), Error> {
    check_code_write(user.is_owner, &state.code_execution)
}

/// The rule for creating an atom instance (`POST /instance/create_generic`), which runs
/// TypeScript from a URL as a macro: only the owner, and only while code execution is on.
// only `POST /instance/create_generic` uses it, which needs the `deno` feature
#[cfg_attr(not(feature = "deno"), allow(dead_code))]
pub fn check_create_atom(user: &User, code_execution: &CodeExecutionSwitch) -> Result<(), Error> {
    user.owner_only("create atom servers")?;
    code_execution.check()
}

/// The rule for adding a Docker container to the watch list (`PUT /generic_setup_manifest`).
///
/// A Docker instance is not an atom: Lodestone runs no code of its own for it, it only
/// starts, stops and reads an existing container through the Docker daemon. So this needs
/// the normal `CreateInstance` permission, is not owner only, and is not stopped by the
/// code execution kill switch.
pub fn check_watch_docker_container(user: &User, safe_mode: bool) -> Result<(), Error> {
    user.try_action(&UserAction::CreateInstance, safe_mode)
}

/// Whether `instance` is an atom (generic) instance, which runs as a macro. Docker
/// instances (`DOCKER-` UUIDs) are never in `AppState::instances`, so they are never atoms.
pub fn is_atom(instance: &GameInstance) -> bool {
    match instance {
        #[cfg(feature = "deno")]
        GameInstance::GenericInstance(_) => true,
        _ => false,
    }
}

/// Atom (generic) instances run code, so changing or running one is owner only.
pub fn require_owner_for_atom(user: &User, instance: &GameInstance) -> Result<(), Error> {
    if is_atom(instance) {
        user.owner_only("manage atom servers")?;
    }
    Ok(())
}

/// [`require_owner_for_atom`] for the instance `uuid`, if it exists. Docker containers and
/// unknown instances are left to the caller.
pub fn require_owner_if_atom(
    state: &AppState,
    user: &User,
    uuid: &InstanceUuid,
) -> Result<(), Error> {
    match state.instances.get(uuid) {
        Some(instance) => require_owner_for_atom(user, instance.value()),
        None => Ok(()),
    }
}

/// A logged-in user, taken from the `Authorization: Bearer <token>` header. The token is a
/// session JWT or an API key; for a key, the user has only the key's powers.
///
/// A missing or malformed header is rejected the same way `AuthBearer` rejects it; a token
/// that does not belong to a user gets a 401 with the usual JSON error body.
pub struct AuthedUser(pub User);

impl FromRequestParts<AppState> for AuthedUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let AuthBearer(token) = AuthBearer::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        authenticate(state, &token)
            .await
            .map(AuthedUser)
            .map_err(IntoResponse::into_response)
    }
}

impl Deref for AuthedUser {
    type Target = User;

    fn deref(&self) -> &User {
        &self.0
    }
}

impl AuthedUser {
    /// Checks that this user may perform `action`, taking safe mode into account.
    pub async fn require(&self, state: &AppState, action: UserAction) -> Result<(), Error> {
        require(state, &self.0, action).await
    }

    pub fn require_owner(&self) -> Result<(), Error> {
        if self.0.is_owner {
            Ok(())
        } else {
            Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("Only the owner can do this"),
            })
        }
    }

    /// See [`User::caused_by`]: names the API key too, for requests made with one.
    pub fn caused_by(&self) -> CausedBy {
        self.0.caused_by()
    }

    pub fn into_inner(self) -> User {
        self.0
    }
}

/// Returns a clone of the instance, so that no `DashMap` guard is held across an `.await`.
pub fn get_instance(state: &AppState, uuid: &InstanceUuid) -> Result<GameInstance, Error> {
    state
        .instances
        .get(uuid)
        .map(|instance| instance.value().clone())
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Server not found"),
        })
}

/// Returns clones of all instances, for the same reason as [`get_instance`].
pub fn all_instances(state: &AppState) -> Vec<GameInstance> {
    state
        .instances
        .iter()
        .map(|instance| instance.value().clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_paths() {
        for name in [
            "a.ts",
            "a.tsx",
            "a.js",
            "a.jsx",
            "a.mjs",
            "a.cjs",
            "a.lua",
            "dir/prelaunch.ts",
            "RUN.TS",
            "run.ts.",
            "run.ts .",
            "run.ts::$DATA",
            "notes.txt:hidden",
        ] {
            assert!(is_script_path(FsPath::new(name)), "{name}");
        }
        for name in [
            "a.txt",
            "a.json",
            "ts",
            "a.tsv",
            "a.jar",
            "server.properties",
            "macros",
            "",
        ] {
            assert!(!is_script_path(FsPath::new(name)), "{name}");
        }
    }

    #[test]
    fn macros_dir_paths() {
        for path in [
            "macros",
            "macros/prelaunch.ts",
            "macros/x/config.json",
            "./macros/a",
            "MACROS/a",
            "Macros./a",
            "macros /a",
            "macros::$INDEX_ALLOCATION/a",
        ] {
            assert!(is_in_macros_dir(FsPath::new(path)), "{path}");
        }
        for path in [
            "",
            "world/macros",
            "macros2/a",
            "my_macros",
            "plugins/a.jar",
        ] {
            assert!(!is_in_macros_dir(FsPath::new(path)), "{path}");
        }
    }

    #[test]
    fn code_paths() {
        assert!(is_code_path(FsPath::new("prelaunch.ts"), false));
        assert!(is_code_path(FsPath::new("plugins/x/a.js"), false));
        assert!(is_code_path(FsPath::new("macros/readme.txt"), false));
        assert!(is_code_path(FsPath::new("macros"), true));
        assert!(is_code_path(FsPath::new("macros/sub"), true));
        // a folder named like a script is just a folder
        assert!(!is_code_path(FsPath::new("thing.ts"), true));
        assert!(!is_code_path(FsPath::new("world/level.dat"), false));
        assert!(!is_code_path(FsPath::new("world"), true));
    }

    #[test]
    fn code_write_needs_owner_and_switch() {
        let on = CodeExecutionSwitch::new(true);
        let off = CodeExecutionSwitch::new(false);
        assert!(check_code_write(true, &on).is_ok());

        let err = check_code_write(false, &on).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        assert!(err.source.to_string().starts_with("Only the owner can"));

        // the switch applies to the owner too
        let err = check_code_write(true, &off).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        assert_eq!(
            err.source.to_string(),
            crate::global_settings::CODE_EXECUTION_DISABLED_MESSAGE
        );
        assert!(check_code_write(false, &off).is_err());
    }

    fn non_owner(permissions: crate::auth::permission::UserPermission) -> User {
        User::new("user".to_string(), "pw", false, false, permissions)
    }

    #[test]
    fn docker_watch_list_needs_create_instance_only() {
        let off = CodeExecutionSwitch::new(false);
        let creator = non_owner(crate::auth::permission::UserPermission {
            can_create_instance: true,
            ..Default::default()
        });
        let nobody = non_owner(Default::default());

        // a non-owner with CreateInstance may add a container, whether or not code
        // execution is on (the switch is not an input to the rule at all)
        assert!(off.check().is_err());
        assert!(check_watch_docker_container(&creator, false).is_ok());
        assert!(check_watch_docker_container(&creator, true).is_ok());
        let err = check_watch_docker_container(&nobody, false).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));

        // the same user may not create an atom, with the switch on or off
        let on = CodeExecutionSwitch::new(true);
        for switch in [&on, &off] {
            let err = check_create_atom(&creator, switch).unwrap_err();
            assert!(matches!(err.kind, ErrorKind::PermissionDenied));
            assert_eq!(
                err.source.to_string(),
                "Only the owner can create atom servers"
            );
        }
    }

    #[test]
    fn atom_creation_is_owner_only_and_follows_the_switch() {
        let owner = User::new("owner".to_string(), "pw", true, false, Default::default());
        assert!(check_create_atom(&owner, &CodeExecutionSwitch::new(true)).is_ok());
        let err = check_create_atom(&owner, &CodeExecutionSwitch::new(false)).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        assert_eq!(
            err.source.to_string(),
            crate::global_settings::CODE_EXECUTION_DISABLED_MESSAGE
        );
        // the owner may add Docker containers with the switch off too
        assert!(check_watch_docker_container(&owner, true).is_ok());
    }

    #[tokio::test]
    async fn finds_scripts_in_folders() {
        let dir = tempfile::TempDir::with_prefix("contains_script").unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("a/b.ts")).unwrap();
        std::fs::write(root.join("a/data.json"), "{}").unwrap();
        // a folder named like a script is not a script
        assert!(!contains_script(root.clone()).await.unwrap());
        std::fs::write(root.join("a/b.ts/main.JS"), "").unwrap();
        assert!(contains_script(root.clone()).await.unwrap());
    }
}
