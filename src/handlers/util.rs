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
        base64::decode_engine(
            input,
            &base64::engine::fast_portable::FastPortable::from(
                &base64::alphabet::URL_SAFE,
                base64::engine::fast_portable::NO_PAD,
            ),
        )
        .context("Failed to decode base64")?,
    )
    .context("Invalid UTF-8")?)
}
