use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use lazy_static::lazy_static;
use rand::{distributions::Alphanumeric, Rng};
use serde::de::DeserializeOwned;

use crate::db::{permission::Permission, user::User};

use super::users::Claim;

lazy_static! {
    pub static ref JWT_SECRET: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect();
}

pub fn decode_token(token: &str) -> Option<User> {
    match decode::<Claim>(
        token,
        &DecodingKey::from_secret(&*JWT_SECRET.as_bytes()),
        &Validation::new(Algorithm::HS512),
    ) {
        Ok(t) => Some(t.claims.user),
        Err(_) => None,
    }
}

pub fn is_authorized(token: &str, instance_uuid: &str, perm: Permission) -> bool {
    match decode::<Claim>(
        token,
        &DecodingKey::from_secret(&*JWT_SECRET.as_bytes()),
        &Validation::new(Algorithm::HS512),
    ) {
        Ok(token) => {
            let user = token.claims.user;
            if user.is_owner {
                return true;
            }

            match perm {
                Permission::CanManageUser | Permission::CanAccessMacro => user.is_owner,
                Permission::CanManagePermission => user.is_admin,
                _ => {
                    user.is_admin
                        || user
                            .permissions
                            .get(&perm)
                            .map(|p| p.contains(instance_uuid))
                            .unwrap_or(false)
                }
            }
        }
        Err(_) => false,
    }
}
