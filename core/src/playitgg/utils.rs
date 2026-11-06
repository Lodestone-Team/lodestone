/*
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

use color_eyre::eyre::eyre;
use playit_api_client::PlayitApi;

use super::playit_secret::*;
use super::PLAYIT_REQUEST_TIMEOUT;
use crate::error::Error;

/// Whether playit.gg accepts `secret`. A request that fails or times out counts as invalid.
pub async fn is_valid_secret_key(secret: String) -> bool {
    let api = PlayitApi::create(API_BASE.to_string(), Some(secret));
    matches!(
        tokio::time::timeout(PLAYIT_REQUEST_TIMEOUT, api.agents_rundata()).await,
        Ok(Ok(_))
    )
}

/// Reads the agent secret from the contents of `playit.toml`.
///
/// Returns `Ok(None)` when the file has no `secret_key`, and an error when it is not valid
/// TOML or the key is not a string.
pub fn parse_secret_key(playit_toml: &str) -> Result<Option<String>, Error> {
    let table: toml::Table =
        toml::from_str(playit_toml).map_err(|e| eyre!("playit.toml is not valid TOML: {e}"))?;
    match table.get("secret_key") {
        None => Ok(None),
        Some(toml::Value::String(key)) => Ok(Some(key.clone())),
        Some(_) => Err(eyre!("secret_key in playit.toml is not a string").into()),
    }
}

pub fn make_client(api_base: String, secret: String) -> PlayitApi {
    PlayitApi::create(api_base, Some(secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_secret_key_handles_bad_files() {
        assert_eq!(
            parse_secret_key("secret_key = \"abc\"\nlast_update = 0").unwrap(),
            Some("abc".to_string())
        );
        // these used to panic at startup
        assert_eq!(parse_secret_key("last_update = 0").unwrap(), None);
        assert!(parse_secret_key("secret_key = ").is_err());
        assert!(parse_secret_key("secret_key = 5").is_err());
        assert_eq!(parse_secret_key("").unwrap(), None);
    }
}
