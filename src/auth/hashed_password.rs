use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct HashedPassword(String);

impl ToString for HashedPassword {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<HashedPassword> for String {
    fn from(hashed_password: HashedPassword) -> Self {
        hashed_password.to_string()
    }
}

impl AsRef<str> for HashedPassword {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub fn hash_password(password: impl AsRef<str>) -> HashedPassword {
    HashedPassword(
        Argon2::default()
            .hash_password(
                password.as_ref().as_bytes(),
                &SaltString::generate(&mut OsRng),
            )
            .unwrap()
            .to_string(),
    )
}
