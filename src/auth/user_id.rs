use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct UserId(String);

impl From<String> for UserId {
    fn from(uuid: String) -> Self {
        Self(uuid)
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self(format!("USER_{}", uuid::Uuid::new_v4()))
    }
}

// implement partial eq for all types that can be converted to string
impl<T: AsRef<str>> PartialEq<T> for UserId {
    fn eq(&self, other: &T) -> bool {
        self.0 == other.as_ref()
    }
}

impl AsRef<UserId> for UserId {
    fn as_ref(&self) -> &UserId {
        self
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

use std::{fmt::Display, hash::Hash};
impl Hash for UserId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[test]
fn test_user_id() {
    let user_id1 = UserId::default();
    // serializing
    let user_id_str = serde_json::to_string(&user_id1).unwrap();
    println!("{}", user_id_str);
    // deserializing
    let user_id2: UserId = serde_json::from_str(&user_id_str).unwrap();
    assert_eq!(user_id1, user_id2);
}
