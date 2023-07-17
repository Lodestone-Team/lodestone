use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::util::rand_alphanumeric;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct UserSecret(String);

impl Default for UserSecret {
    fn default() -> Self {
        Self(rand_alphanumeric(32))
    }
}

impl ToString for UserSecret {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<UserSecret> for String {
    fn from(user_secret: UserSecret) -> Self {
        user_secret.to_string()
    }
}

impl AsRef<str> for UserSecret {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
