use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Error;

use super::{user::Claim, user_secrets::UserSecret};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct JwtToken(String);

impl ToString for JwtToken {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<JwtToken> for String {
    fn from(jwt_token: JwtToken) -> Self {
        jwt_token.to_string()
    }
}

impl AsRef<str> for JwtToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl JwtToken {
    pub fn new(claim: Claim, secret: UserSecret) -> Result<JwtToken, Error> {
        Ok(JwtToken(
            jsonwebtoken::encode(
                &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS512),
                &claim,
                &jsonwebtoken::EncodingKey::from_secret(secret.as_ref().as_bytes()),
            )
            .context("Failed to encode JWT token")?,
        ))
    }
}
