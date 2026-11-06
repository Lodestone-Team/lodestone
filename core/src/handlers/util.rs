use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use color_eyre::eyre::Context;

use crate::error::Error;

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
