//! Users, permissions, the profile, API keys and Minecraft links: work package D (see
//! `launcher/docs/host-parity.md`). After the dashboard's `data/UserInfo.ts`,
//! `data/ApiKeys.ts` and `data/MinecraftLinks.ts`.

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::{
    AdminClient,
    models::{PublicUser, UserPermission},
    segment,
};
use crate::cores::CoreError;

impl AdminClient {
    /// `GET /user/info`: the signed-in user.
    pub async fn me(&self) -> Result<PublicUser, CoreError> {
        self.get("user/info").await
    }

    /// `GET /user/list`: every user (the owner's list).
    pub async fn users(&self) -> Result<Vec<PublicUser>, CoreError> {
        self.get("user/list").await
    }

    /// `POST /user`: a new user, without permissions.
    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<PublicUser, CoreError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        self.json(Method::POST, "user", Some(Body { username, password }))
            .await
    }

    /// `DELETE /user/{uid}`.
    pub async fn delete_user(&self, uid: &str) -> Result<(), CoreError> {
        self.call(
            Method::DELETE,
            &format!("user/{}", segment(uid)),
            None::<()>,
        )
        .await
    }

    /// `PUT /user/{uid}/password`. Users changing their own password give the old one;
    /// the owner changing someone else's gives `None`.
    pub async fn change_password(
        &self,
        uid: &str,
        old_password: Option<&str>,
        new_password: &str,
    ) -> Result<(), CoreError> {
        #[derive(Serialize)]
        struct Body<'a> {
            uid: &'a str,
            old_password: Option<&'a str>,
            new_password: &'a str,
        }
        self.call(
            Method::PUT,
            &format!("user/{}/password", segment(uid)),
            Some(Body {
                uid,
                old_password,
                new_password,
            }),
        )
        .await
    }

    /// `PUT /user/{uid}/rename`.
    pub async fn rename_user(&self, uid: &str, username: &str) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("user/{}/rename", segment(uid)),
            Some(username),
        )
        .await
    }

    /// `PUT /user/owner_account`: gives the owner, who has no password yet (Lodestone
    /// Host's first run), a username and a password, and lets other devices use the
    /// core. Only once, and only signed in (not with an API key). Returns the owner.
    pub async fn set_up_owner_account(
        &self,
        username: &str,
        password: &str,
    ) -> Result<PublicUser, CoreError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        self.json(
            Method::PUT,
            "user/owner_account",
            Some(Body { username, password }),
        )
        .await
    }

    /// `PUT /user/{uid}/update_perm`: the user's whole permission set.
    pub async fn set_permissions(
        &self,
        uid: &str,
        permissions: &UserPermission,
    ) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("user/{}/update_perm", segment(uid)),
            Some(permissions),
        )
        .await
    }

    /// `GET /user/api_keys`: the signed-in user's keys.
    pub async fn own_api_keys(&self) -> Result<Vec<ApiKeyInfo>, CoreError> {
        self.get("user/api_keys").await
    }

    /// `GET /user/{uid}/api_keys`: another user's keys (owners and admins).
    pub async fn user_api_keys(&self, uid: &str) -> Result<Vec<ApiKeyInfo>, CoreError> {
        self.get(&format!("user/{}/api_keys", segment(uid))).await
    }

    /// `POST /user/api_keys`: a key for the signed-in user. The reply holds the key
    /// itself, the only time the core sends it.
    pub async fn create_api_key(
        &self,
        request: &CreateApiKeyRequest,
    ) -> Result<CreatedApiKey, CoreError> {
        self.json(Method::POST, "user/api_keys", Some(request))
            .await
    }

    /// `DELETE /user/api_keys/{id}`, or `DELETE /user/{uid}/api_keys/{id}` for
    /// another user's key.
    pub async fn revoke_api_key(&self, uid: Option<&str>, id: &str) -> Result<(), CoreError> {
        let path = match uid {
            Some(uid) => format!("user/{}/api_keys/{}", segment(uid), segment(id)),
            None => format!("user/api_keys/{}", segment(id)),
        };
        self.call(Method::DELETE, &path, None::<()>).await
    }

    /// `GET /user/minecraft_links`: the signed-in user's Minecraft accounts.
    pub async fn own_minecraft_links(&self) -> Result<Vec<MinecraftLink>, CoreError> {
        self.get("user/minecraft_links").await
    }

    /// `GET /minecraft_links`: every user's Minecraft accounts (the owner's).
    pub async fn all_minecraft_links(&self) -> Result<Vec<MinecraftLink>, CoreError> {
        self.get("minecraft_links").await
    }

    /// `DELETE /user/minecraft_links/{uuid}`, or `DELETE /minecraft_links/{uuid}` for
    /// the owner removing anyone's (`any_user`).
    pub async fn unlink_minecraft(
        &self,
        minecraft_uuid: &str,
        any_user: bool,
    ) -> Result<(), CoreError> {
        let base = if any_user {
            "minecraft_links"
        } else {
            "user/minecraft_links"
        };
        self.call(
            Method::DELETE,
            &format!("{base}/{}", segment(minecraft_uuid)),
            None::<()>,
        )
        .await
    }
}

/// What an API key may do (`ApiKeyScope`).
#[allow(clippy::large_enum_variant)] // a value read once, not stored in bulk
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiKeyScope {
    Full,
    Custom {
        permissions: UserPermission,
    },
    /// A Lodestone Launcher device.
    Launcher {
        #[serde(default)]
        console_read: bool,
        #[serde(default)]
        console_write: bool,
    },
    #[serde(other)]
    Unknown,
}

/// A key as the core shows it: never the key itself (`ApiKeyInfo`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    /// `lsk_<id>_…`
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub uid: String,
    /// Unix seconds.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    pub scope: ApiKeyScope,
}

impl ApiKeyInfo {
    /// Whether the key has expired at `now` (unix seconds).
    pub fn expired(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }
}

/// The body of `POST /user/api_keys` (`CreateApiKeyRequest`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    /// Unix seconds, or `None` for a key that never expires.
    pub expires_at: Option<i64>,
    pub scope: ApiKeyScope,
}

/// The reply to `POST /user/api_keys` (`CreatedApiKey`). No `Debug`: it holds the key.
#[derive(Clone, Deserialize)]
pub struct CreatedApiKey {
    pub key: String,
    pub info: ApiKeyInfo,
}

/// A user's Minecraft account (`MinecraftLink`, and `MinecraftLinkInfo` with the
/// user's name in the owner's list).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftLink {
    pub uid: String,
    /// In the owner's list only.
    #[serde(default)]
    pub username: Option<String>,
    pub minecraft_uuid: String,
    pub minecraft_name: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub linked_at: i64,
}

/// The permissions an admin has on every instance without being granted them, as the
/// core's `User::can_perform_action` decides.
const ADMIN_IMPLICIT: [Permission; 9] = [
    Permission::ViewInstance,
    Permission::StartInstance,
    Permission::StopInstance,
    Permission::Console,
    Permission::InstanceSettings,
    Permission::ReadResource,
    Permission::ReadFiles,
    Permission::CreateInstance,
    Permission::DeleteInstance,
];

/// A permission the editors list (`permissionDescriptions.ts`), in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    CreateInstance,
    DeleteInstance,
    ReadGlobalFile,
    ViewInstance,
    StartInstance,
    StopInstance,
    Console,
    InstanceSettings,
    ReadResource,
    ReadFiles,
    WriteResource,
    WriteFiles,
    WriteGlobalFile,
    ManagePermission,
}

impl Permission {
    /// The permissions anyone may be given.
    pub const NORMAL: [Permission; 10] = [
        Permission::CreateInstance,
        Permission::DeleteInstance,
        Permission::ReadGlobalFile,
        Permission::ViewInstance,
        Permission::StartInstance,
        Permission::StopInstance,
        Permission::Console,
        Permission::InstanceSettings,
        Permission::ReadResource,
        Permission::ReadFiles,
    ];

    /// The unsafe ones, which safe mode keeps off.
    pub const UNSAFE: [Permission; 4] = [
        Permission::WriteResource,
        Permission::WriteFiles,
        Permission::WriteGlobalFile,
        Permission::ManagePermission,
    ];

    /// Whether it applies to chosen instances (a list) rather than the whole core.
    pub fn per_instance(self) -> bool {
        self.list(&UserPermission::default()).is_some()
    }

    /// Its instance list in `p`, for a per-instance permission.
    pub fn list(self, p: &UserPermission) -> Option<&Vec<String>> {
        Some(match self {
            Permission::ViewInstance => &p.can_view_instance,
            Permission::StartInstance => &p.can_start_instance,
            Permission::StopInstance => &p.can_stop_instance,
            Permission::Console => &p.can_access_instance_console,
            Permission::InstanceSettings => &p.can_access_instance_setting,
            Permission::ReadResource => &p.can_read_instance_resource,
            Permission::ReadFiles => &p.can_read_instance_file,
            Permission::WriteResource => &p.can_write_instance_resource,
            Permission::WriteFiles => &p.can_write_instance_file,
            _ => return None,
        })
    }

    fn list_mut(self, p: &mut UserPermission) -> Option<&mut Vec<String>> {
        Some(match self {
            Permission::ViewInstance => &mut p.can_view_instance,
            Permission::StartInstance => &mut p.can_start_instance,
            Permission::StopInstance => &mut p.can_stop_instance,
            Permission::Console => &mut p.can_access_instance_console,
            Permission::InstanceSettings => &mut p.can_access_instance_setting,
            Permission::ReadResource => &mut p.can_read_instance_resource,
            Permission::ReadFiles => &mut p.can_read_instance_file,
            Permission::WriteResource => &mut p.can_write_instance_resource,
            Permission::WriteFiles => &mut p.can_write_instance_file,
            _ => return None,
        })
    }

    fn flag_mut(self, p: &mut UserPermission) -> Option<&mut bool> {
        Some(match self {
            Permission::CreateInstance => &mut p.can_create_instance,
            Permission::DeleteInstance => &mut p.can_delete_instance,
            Permission::ReadGlobalFile => &mut p.can_read_global_file,
            Permission::WriteGlobalFile => &mut p.can_write_global_file,
            Permission::ManagePermission => &mut p.can_manage_permission,
            _ => return None,
        })
    }

    /// Its value in `p`, for a core-wide permission.
    pub fn flag(self, p: &UserPermission) -> Option<bool> {
        let mut p = p.clone();
        self.flag_mut(&mut p).map(|flag| *flag)
    }

    /// Turns a core-wide permission on or off.
    pub fn set_flag(self, p: &mut UserPermission, value: bool) {
        if let Some(flag) = self.flag_mut(p) {
            *flag = value;
        }
    }

    /// Gives or takes a per-instance permission for one instance, keeping the others.
    pub fn set_instance(self, p: &mut UserPermission, instance: &str, value: bool) {
        if let Some(list) = self.list_mut(p) {
            list.retain(|id| id != instance);
            if value {
                list.push(instance.to_string());
            }
        }
    }

    /// Whether `user` has it (on `instance`, for a per-instance one), and so can give
    /// it to an API key with a custom scope (`canGrantToApiKey`).
    pub fn grantable_by(self, user: &PublicUser, instance: Option<&str>) -> bool {
        if user.is_owner || (user.is_admin && ADMIN_IMPLICIT.contains(&self)) {
            return true;
        }
        match self.list(&user.permissions) {
            Some(list) => instance.is_some_and(|i| list.iter().any(|id| id == i)),
            None => self.flag(&user.permissions).unwrap_or(false),
        }
    }
}

/// How many permissions `p` grants, and how many distinct instances they name
/// (`countPermissions`), for "Custom: 3 servers, 5 permissions".
pub fn count_permissions(p: &UserPermission) -> (usize, usize) {
    let mut instances = std::collections::BTreeSet::new();
    let lists = [
        &p.can_view_instance,
        &p.can_start_instance,
        &p.can_stop_instance,
        &p.can_access_instance_console,
        &p.can_access_instance_setting,
        &p.can_read_instance_resource,
        &p.can_write_instance_resource,
        &p.can_access_instance_macro,
        &p.can_read_instance_file,
        &p.can_write_instance_file,
    ];
    let flags = [
        p.can_create_instance,
        p.can_delete_instance,
        p.can_read_global_file,
        p.can_write_global_file,
        p.can_manage_permission,
        p.can_install_extension,
    ];
    let mut granted = flags.iter().filter(|flag| **flag).count();
    for list in lists {
        if !list.is_empty() {
            granted += 1;
        }
        instances.extend(list.iter());
    }
    (granted, instances.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_api_keys_and_their_scopes() {
        let keys: Vec<ApiKeyInfo> = serde_json::from_value(serde_json::json!([
            { "id": "a", "name": "CI", "display": "lsk_a_…", "uid": "u", "created_at": 1,
              "expires_at": null, "last_used_at": 5, "scope": { "type": "full" } },
            { "id": "b", "name": "Phone", "display": "lsk_b_…", "uid": "u", "created_at": 1,
              "expires_at": 10, "last_used_at": null,
              "scope": { "type": "launcher", "console_read": true } },
            { "id": "c", "name": "Bot", "display": "lsk_c_…", "uid": "u", "created_at": 1,
              "expires_at": null, "last_used_at": null,
              "scope": { "type": "custom", "permissions": { "can_view_instance": ["i"] } } },
            { "id": "d", "name": "New", "uid": "u", "scope": { "type": "robot" } },
        ]))
        .unwrap();
        assert_eq!(keys[0].scope, ApiKeyScope::Full);
        assert_eq!(
            keys[1].scope,
            ApiKeyScope::Launcher {
                console_read: true,
                console_write: false
            }
        );
        assert!(keys[1].expired(10) && !keys[1].expired(9));
        assert!(!keys[0].expired(i64::MAX));
        assert!(matches!(keys[2].scope, ApiKeyScope::Custom { .. }));
        assert_eq!(keys[3].scope, ApiKeyScope::Unknown);
    }

    #[test]
    fn a_new_key_is_sent_as_the_core_reads_it() {
        let request = CreateApiKeyRequest {
            name: "CI".into(),
            expires_at: None,
            scope: ApiKeyScope::Full,
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({ "name": "CI", "expires_at": null, "scope": { "type": "full" } })
        );
    }

    #[test]
    fn permissions_change_one_at_a_time() {
        let mut p = UserPermission::default();
        Permission::StartInstance.set_instance(&mut p, "a", true);
        Permission::StartInstance.set_instance(&mut p, "b", true);
        Permission::StartInstance.set_instance(&mut p, "a", true);
        assert_eq!(p.can_start_instance, ["b", "a"]);
        Permission::StartInstance.set_instance(&mut p, "b", false);
        assert_eq!(p.can_start_instance, ["a"]);
        Permission::ManagePermission.set_flag(&mut p, true);
        assert_eq!(Permission::ManagePermission.flag(&p), Some(true));
        assert!(Permission::ViewInstance.per_instance());
        assert!(!Permission::CreateInstance.per_instance());
        assert_eq!(count_permissions(&p), (2, 1));
    }

    #[test]
    fn keys_get_only_what_their_user_has() {
        let mut user = PublicUser {
            uid: "u".into(),
            username: "u".into(),
            is_owner: false,
            is_admin: false,
            permissions: UserPermission {
                can_stop_instance: vec!["a".into()],
                can_read_global_file: true,
                ..Default::default()
            },
            has_password: true,
        };
        assert!(Permission::StopInstance.grantable_by(&user, Some("a")));
        assert!(!Permission::StopInstance.grantable_by(&user, Some("b")));
        assert!(!Permission::StopInstance.grantable_by(&user, None));
        assert!(Permission::ReadGlobalFile.grantable_by(&user, None));
        assert!(!Permission::CreateInstance.grantable_by(&user, None));
        user.is_admin = true;
        assert!(Permission::CreateInstance.grantable_by(&user, None));
        assert!(Permission::ViewInstance.grantable_by(&user, Some("b")));
        assert!(!Permission::WriteFiles.grantable_by(&user, Some("b")));
    }

    #[test]
    fn reads_both_kinds_of_minecraft_link() {
        let own: MinecraftLink = serde_json::from_value(serde_json::json!({
            "uid": "u", "minecraft_uuid": "x", "minecraft_name": "Steve",
            "verified": true, "linked_at": 3,
        }))
        .unwrap();
        assert_eq!(own.username, None);
        let all: MinecraftLink = serde_json::from_value(serde_json::json!({
            "uid": "u", "username": "peter", "minecraft_uuid": "x",
            "minecraft_name": "Steve", "verified": true, "linked_at": 3,
        }))
        .unwrap();
        assert_eq!(all.username.as_deref(), Some("peter"));
    }
}
