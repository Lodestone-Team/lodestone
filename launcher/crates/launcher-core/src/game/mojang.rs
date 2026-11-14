//! Mojang's and the mod loaders' download hosts, and the one place URLs from version
//! files are checked.
//!
//! Every URL the installer fetches comes from a Mojang file (the version manifest, a
//! version's details, an asset index, the Java runtime manifest), a mod loader's
//! (Fabric's and Quilt's meta, Forge's and NeoForge's installers) or Azul's list of
//! Java builds (Zulu 8 for Apple silicon, [`super::zulu`]). [`MojangUrls::resolve`]
//! only lets through HTTPS URLs on those hosts ([`HOSTS`], [`LOADER_HOSTS`] and
//! [`JAVA_HOSTS`]), upgrading old `http://` links. Tests use [`MojangUrls::mirror`], which serves each host from a path on a
//! loopback mock server, so the recorded files can be used with their real URLs.

use url::Url;

use lodestone_minecraft::versions::mojang::VERSION_MANIFEST_V2_URL;

/// The Java runtime manifest the official launcher reads, on piston-meta.
pub const RUNTIME_MANIFEST_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// The hosts files are downloaded from.
pub const HOSTS: &[&str] = &[
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "launchermeta.mojang.com",
    "launcher.mojang.com",
    "libraries.minecraft.net",
    "resources.download.minecraft.net",
];

/// The mod loaders' metadata and Maven hosts.
pub const LOADER_HOSTS: &[&str] = &[
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "files.minecraftforge.net",
    "maven.minecraftforge.net",
    "maven.neoforged.net",
];

/// Azul's metadata API and CDN, for the Java 8 Mojang doesn't build for Apple silicon.
pub const JAVA_HOSTS: &[&str] = &["api.azul.com", "cdn.azul.com"];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UrlError {
    #[error("{0} is not a valid URL")]
    Invalid(String),
    #[error("{0} is not on one of Mojang's, the mod loaders' or Azul's download servers")]
    NotMojang(String),
}

/// Checks and maps the URLs the installer downloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MojangUrls {
    /// Where each host is served from in tests; `None` in production.
    mirror: Option<Url>,
}

impl MojangUrls {
    pub fn production() -> Self {
        Self { mirror: None }
    }

    /// Serves `https://<host>/<path>` from `<base>/<host>/<path>`.
    pub fn mirror(base: &Url) -> Self {
        Self {
            mirror: Some(base.clone()),
        }
    }

    pub fn version_manifest(&self) -> String {
        self.resolve(VERSION_MANIFEST_V2_URL)
            .expect("a built-in Mojang URL")
    }

    pub fn runtime_manifest(&self) -> String {
        self.resolve(RUNTIME_MANIFEST_URL)
            .expect("a built-in Mojang URL")
    }

    /// The base of asset object URLs.
    pub fn resources_base(&self) -> String {
        self.resolve(super::assets::RESOURCES_URL)
            .expect("a built-in Mojang URL")
    }

    /// The URL to download `url` from, if it is on one of Mojang's, the mod loaders'
    /// or Azul's hosts.
    pub fn resolve(&self, url: &str) -> Result<String, UrlError> {
        let parsed = Url::parse(url).map_err(|_| UrlError::Invalid(url.to_string()))?;
        let host = parsed
            .host_str()
            .filter(|host| {
                HOSTS.contains(host) || LOADER_HOSTS.contains(host) || JAVA_HOSTS.contains(host)
            })
            .ok_or_else(|| UrlError::NotMojang(url.to_string()))?;
        if !matches!(parsed.scheme(), "https" | "http")
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(UrlError::NotMojang(url.to_string()));
        }
        let path_and_query = match parsed.query() {
            Some(query) => format!("{}?{query}", parsed.path()),
            None => parsed.path().to_string(),
        };
        Ok(match &self.mirror {
            None => format!("https://{host}{path_and_query}"),
            Some(base) => format!(
                "{}/{host}{path_and_query}",
                base.as_str().trim_end_matches('/')
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_allows_mojang_hosts_only() {
        let urls = MojangUrls::production();
        assert_eq!(
            urls.resolve("https://piston-data.mojang.com/v1/objects/abc/client.jar")
                .unwrap(),
            "https://piston-data.mojang.com/v1/objects/abc/client.jar"
        );
        // Old files link over plain HTTP; downloads always use HTTPS.
        assert_eq!(
            urls.resolve("http://launcher.mojang.com/v1/objects/abc/client-1.7.xml")
                .unwrap(),
            "https://launcher.mojang.com/v1/objects/abc/client-1.7.xml"
        );
        assert_eq!(
            urls.resolve("https://maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar")
                .unwrap(),
            "https://maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar"
        );
        for bad in [
            "https://evil.example/client.jar",
            "https://maven.fabricmc.net.evil.example/x.jar",
            "https://cdn.azul.com.evil.example/zulu/bin/x.zip",
            "https://piston-data.mojang.com.evil.example/x",
            "https://piston-data.mojang.com:8443/x",
            "https://user@piston-data.mojang.com/x",
            "file:///etc/passwd",
            "ftp://libraries.minecraft.net/x",
        ] {
            assert!(urls.resolve(bad).is_err(), "{bad}");
        }
        assert!(matches!(
            urls.resolve("not a url"),
            Err(UrlError::Invalid(_))
        ));
        assert_eq!(
            urls.version_manifest(),
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
        );
    }

    #[test]
    fn mirror_maps_each_host_to_a_path() {
        let urls = MojangUrls::mirror(&Url::parse("http://127.0.0.1:5000/").unwrap());
        assert_eq!(
            urls.resolve("https://libraries.minecraft.net/org/lwjgl/lwjgl.jar?x=1")
                .unwrap(),
            "http://127.0.0.1:5000/libraries.minecraft.net/org/lwjgl/lwjgl.jar?x=1"
        );
        assert_eq!(
            urls.resources_base(),
            "http://127.0.0.1:5000/resources.download.minecraft.net/"
        );
        assert!(urls.resolve("https://example.com/x").is_err());
    }
}
