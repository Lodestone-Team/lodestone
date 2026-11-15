//! Where the launcher sends people to download Lodestone Host, or Lodestone Core
//! alone. The links match the website's Host page (`website/src/host`).
//!
//! Lodestone Host is a GitHub release of its own, tagged `host-vX.Y.Z`
//! ([`crate::update::HOST_TAG_PREFIX`]), with its installers under names that never
//! change: `LodestoneHost.dmg` (macOS) and `LodestoneHostSetup.exe` (Windows). The
//! download button takes the newest published stable one for this computer, and
//! opens the list of Host releases when there is none or GitHub can't be reached.

use http_client::HttpClient;

use crate::update::{self, HOST_TAG_PREFIX, Release, UpdateError};

/// The list of Lodestone Host releases, newest first. The website links here too.
pub const HOST_RELEASES_PAGE: &str =
    "https://github.com/Lodestone-Team/lodestone/releases?q=host-v&expanded=true";

/// Lodestone CLI's newest release. Lodestone CLI installs Lodestone Core, keeps it
/// updated and starts it: it's how the project tells people to run Core on its own
/// (the website's "All downloads", and the note under a core started directly).
pub const CORE_ONLY_PAGE: &str = "https://github.com/Lodestone-Team/lodestone_cli/releases/latest";

/// The Lodestone Host installer for `os` (`std::env::consts::OS`). Lodestone Host has
/// no app for other systems, such as Linux.
pub fn host_installer(os: &str) -> Option<&'static str> {
    match os {
        "macos" => Some("LodestoneHost.dmg"),
        "windows" => Some("LodestoneHostSetup.exe"),
        _ => None,
    }
}

/// Whether Lodestone Host has an app for this computer. Without one, Lodestone CLI
/// is the way to host ([`CORE_ONLY_PAGE`]).
pub fn host_app_available() -> bool {
    host_installer(std::env::consts::OS).is_some()
}

/// Of `releases` (newest first, as GitHub lists them), the download address of the
/// newest published stable Lodestone Host release's installer for `os`. Drafts, betas
/// and other products' releases are skipped, as on the website.
pub fn pick_host_download(releases: &[Release], os: &str) -> Option<String> {
    let installer = host_installer(os)?;
    releases
        .iter()
        .filter(|release| {
            !release.draft && !release.prerelease && release.tag_name.starts_with(HOST_TAG_PREFIX)
        })
        .find_map(|release| {
            release
                .assets
                .iter()
                .find(|asset| asset.name == installer)
                .map(|asset| asset.browser_download_url.clone())
        })
        .filter(|url| url.starts_with("https://"))
}

/// What "Download Lodestone Host" opens, given the list of releases (or why it
/// couldn't be read): the installer, or the list of Host releases when there's no
/// published one for `os`. Systems without a Host app get Lodestone CLI.
pub fn host_download_target(releases: Result<&[Release], &UpdateError>, os: &str) -> String {
    if host_installer(os).is_none() {
        return CORE_ONLY_PAGE.to_string();
    }
    releases
        .ok()
        .and_then(|releases| pick_host_download(releases, os))
        .unwrap_or_else(|| HOST_RELEASES_PAGE.to_string())
}

/// Looks up what "Download Lodestone Host" opens on this computer
/// ([`host_download_target`]). Run it inside [`crate::http::in_http_runtime`].
pub async fn host_download_url(http: &dyn HttpClient) -> String {
    let os = std::env::consts::OS;
    if host_installer(os).is_none() {
        return CORE_ONLY_PAGE.to_string();
    }
    let releases = update::fetch_host_releases(http).await;
    host_download_target(releases.as_deref(), os)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::ReleaseAsset;

    const DOWNLOAD: &str = "https://github.com/Lodestone-Team/lodestone/releases/download";

    fn release(tag: &str, draft: bool, prerelease: bool, files: &[&str]) -> Release {
        Release {
            tag_name: tag.into(),
            draft,
            prerelease,
            assets: files
                .iter()
                .map(|name| ReleaseAsset {
                    name: (*name).into(),
                    browser_download_url: format!("{DOWNLOAD}/{tag}/{name}"),
                })
                .collect(),
        }
    }

    const BOTH: &[&str] = &[
        "LodestoneHost.dmg",
        "LodestoneHostSetup.exe",
        "latest.json",
        "lodestone-host-3700-windows-x86_64-standalone.exe",
    ];

    #[test]
    fn each_system_gets_its_installer() {
        assert_eq!(host_installer("macos"), Some("LodestoneHost.dmg"));
        assert_eq!(host_installer("windows"), Some("LodestoneHostSetup.exe"));
        assert_eq!(host_installer("linux"), None);
        let releases = [release("host-v1.0.0", false, false, BOTH)];
        assert_eq!(
            pick_host_download(&releases, "macos").as_deref(),
            Some(format!("{DOWNLOAD}/host-v1.0.0/LodestoneHost.dmg").as_str())
        );
        assert_eq!(
            pick_host_download(&releases, "windows").as_deref(),
            Some(format!("{DOWNLOAD}/host-v1.0.0/LodestoneHostSetup.exe").as_str())
        );
        assert_eq!(pick_host_download(&releases, "linux"), None);
    }

    #[test]
    fn takes_the_newest_published_stable_host_release() {
        let releases = [
            release("host-v1.2.0", true, false, BOTH),
            release("host-v1.2.0-beta.1", false, true, BOTH),
            release(
                "launcher-v1.5.0",
                false,
                false,
                &["LodestoneLauncherSetup.exe"],
            ),
            release(
                "v0.5.1",
                false,
                false,
                &["lodestone_core_windows_x86_64.exe"],
            ),
            release("host-dev-3720-ab12cd3", false, true, BOTH),
            // Only the Mac build is in: Windows takes the one before.
            release("host-v1.1.0", false, false, &["LodestoneHost.dmg"]),
            release("host-v1.0.0", false, false, BOTH),
        ];
        assert_eq!(
            pick_host_download(&releases, "macos").as_deref(),
            Some(format!("{DOWNLOAD}/host-v1.1.0/LodestoneHost.dmg").as_str())
        );
        assert_eq!(
            pick_host_download(&releases, "windows").as_deref(),
            Some(format!("{DOWNLOAD}/host-v1.0.0/LodestoneHostSetup.exe").as_str())
        );
    }

    #[test]
    fn falls_back_to_the_list_of_host_releases() {
        // None published yet: drafts and betas only, and other products'.
        let unpublished = [
            release("host-v1.0.0", true, false, BOTH),
            release("host-v1.0.0-beta.1", false, true, BOTH),
            release(
                "launcher-v1.5.0",
                false,
                false,
                &["LodestoneLauncherSetup.exe"],
            ),
        ];
        for os in ["macos", "windows"] {
            assert_eq!(
                host_download_target(Ok(&unpublished), os),
                HOST_RELEASES_PAGE
            );
            assert_eq!(host_download_target(Ok(&[]), os), HOST_RELEASES_PAGE);
            assert_eq!(
                host_download_target(Err(&UpdateError::RateLimited), os),
                HOST_RELEASES_PAGE
            );
        }
        let published = [release("host-v1.0.0", false, false, BOTH)];
        assert_eq!(
            host_download_target(Ok(&published), "macos"),
            format!("{DOWNLOAD}/host-v1.0.0/LodestoneHost.dmg")
        );
    }

    #[test]
    fn systems_without_a_host_app_get_lodestone_cli() {
        let published = [release("host-v1.0.0", false, false, BOTH)];
        assert_eq!(
            host_download_target(Ok(&published), "linux"),
            CORE_ONLY_PAGE
        );
        assert_eq!(
            host_download_target(Err(&UpdateError::RateLimited), "linux"),
            CORE_ONLY_PAGE
        );
    }

    #[test]
    fn only_https_downloads_are_opened() {
        let mut odd = release("host-v1.0.0", false, false, &["LodestoneHost.dmg"]);
        odd.assets[0].browser_download_url = "file:///Applications/Calculator.app".into();
        assert_eq!(pick_host_download(&[odd], "macos"), None);
    }
}
