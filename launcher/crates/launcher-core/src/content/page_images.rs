//! The images on a project's page (its description and gallery): which hosts the app
//! loads them from, and fetching them.
//!
//! A page's images are loaded only from a short list of hosts, so a description can't
//! make the app contact any server it names (a tracking pixel, say):
//!
//! - Modrinth's CDN and CurseForge's, where the platforms keep uploaded images;
//! - GitHub (`raw.githubusercontent.com` and the image hosts behind README pictures)
//!   and imgur (`i.imgur.com`), where many authors keep their screenshots.
//!
//! Links that only point at an image (`github.com/<owner>/<repo>/blob/<path>?raw=true`,
//! `imgur.com/<id>.png`) are rewritten to the file itself. Requests carry no cookies and
//! follow at most [`MAX_REDIRECTS`] redirects, each to an allowed host. Other images
//! show their alt text, linked to the image.

use std::sync::Arc;

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use url::Url;

/// Images larger than this are not shown.
pub const MAX_PAGE_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// Redirects followed for one image (GitHub's attachments redirect once, to a signed
/// URL).
pub const MAX_REDIRECTS: usize = 3;

/// Hosts an image URL in a description may name.
const HOSTS: &[&str] = &[
    lodestone_minecraft::modrinth::CDN_HOST,
    super::curseforge::MEDIA_HOST,
    "raw.githubusercontent.com",
    "user-images.githubusercontent.com",
    "private-user-images.githubusercontent.com",
    "camo.githubusercontent.com",
    "i.imgur.com",
];

/// Hosts only reached through a redirect from an allowed URL: GitHub's attachments
/// (`github.com/user-attachments/assets/<id>`) redirect to a signed URL on GitHub's
/// own storage bucket.
const REDIRECT_HOSTS: &[&str] = &[
    "github-production-user-asset-6210df.s3.amazonaws.com",
    "objects.githubusercontent.com",
];

/// `url` as the app loads it, if it is an image on an allowed host (or a link that only
/// points at one): HTTPS, no port or user name.
pub fn image_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    if parsed.scheme() != "https"
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    let host = parsed.host_str()?;
    if HOSTS.contains(&host) {
        return Some(parsed.to_string());
    }
    let segments: Vec<&str> = parsed.path_segments()?.collect();
    match (host, segments.as_slice()) {
        // A file in a repository, from its page or its raw link.
        ("github.com" | "www.github.com", [owner, repo, "blob" | "raw", rest @ ..])
            if !owner.is_empty() && !repo.is_empty() && rest.len() >= 2 =>
        {
            Some(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{}",
                rest.join("/")
            ))
        }
        // An image attached to an issue or README: redirects to a signed URL.
        ("github.com" | "www.github.com", ["user-attachments", "assets", id]) if !id.is_empty() => {
            Some(parsed.to_string())
        }
        // An image's page on imgur with the file's extension: the file itself.
        ("imgur.com" | "www.imgur.com" | "m.imgur.com", [file])
            if file.contains('.') && !file.starts_with('.') =>
        {
            Some(format!("https://i.imgur.com/{file}"))
        }
        _ => None,
    }
}

/// Whether a redirect may go to `url`.
fn redirect_allowed(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .is_some_and(|h| HOSTS.contains(&h) || REDIRECT_HOSTS.contains(&h))
}

/// Where `url` is fetched from: itself, or `<mirror>/<host><path>` in tests.
fn mirrored(url: &Url, mirror: Option<&Url>) -> String {
    match mirror {
        None => url.to_string(),
        Some(base) => {
            let path = match url.query() {
                Some(query) => format!("{}?{query}", url.path()),
                None => url.path().to_string(),
            };
            format!(
                "{}/{}{path}",
                base.as_str().trim_end_matches('/'),
                url.host_str().unwrap_or_default()
            )
        }
    }
}

/// The bytes of a page image, at most [`MAX_PAGE_IMAGE_BYTES`]. `url` must be one
/// [`image_url`] allows (it is checked again). In tests, `mirror` serves every host as
/// `<mirror>/<host>/<path>`.
pub async fn fetch(
    http: Arc<dyn HttpClient>,
    url: &str,
    user_agent: &str,
    mirror: Option<&Url>,
) -> Result<Vec<u8>, String> {
    let checked = image_url(url).ok_or_else(|| format!("{url} isn't an allowed image"))?;
    let mut current = Url::parse(&checked).map_err(|err| err.to_string())?;
    for _ in 0..=MAX_REDIRECTS {
        let request = Request::builder()
            .method(Method::GET)
            .uri(mirrored(&current, mirror))
            .header("User-Agent", user_agent)
            .header("Accept", "image/*")
            .follow_redirects(RedirectPolicy::NoFollow)
            .timeout(crate::http::REQUEST_TIMEOUT)
            .body(AsyncBody::empty())
            .map_err(|err| err.to_string())?;
        let response = http.send(request).await.map_err(|err| format!("{err:#}"))?;
        let status = response.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("a redirect without a location")?;
            let next = current.join(location).map_err(|err| err.to_string())?;
            if !redirect_allowed(&next) {
                return Err(format!(
                    "refusing a redirect to {}",
                    next.host_str().unwrap_or("an unknown host")
                ));
            }
            current = next;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}"));
        }
        let mut body = Vec::new();
        response
            .into_body()
            .take(MAX_PAGE_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .await
            .map_err(|err| err.to_string())?;
        if body.len() > MAX_PAGE_IMAGE_BYTES {
            return Err("the image is too large".into());
        }
        return Ok(body);
    }
    Err("too many redirects".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_hosts() {
        for ok in [
            "https://cdn.modrinth.com/data/AANobbMI/images/a.png",
            "https://media.forgecdn.net/attachments/1/2/shot.png",
            "https://raw.githubusercontent.com/o/r/main/shot.png",
            "https://user-images.githubusercontent.com/1/2.png",
            "https://camo.githubusercontent.com/abc/def",
            "https://i.imgur.com/AbC123.gif",
        ] {
            assert_eq!(image_url(ok).as_deref(), Some(ok), "{ok}");
        }
        for bad in [
            "http://i.imgur.com/x.png",
            "https://i.imgur.com:8443/x.png",
            "https://user@i.imgur.com/x.png",
            "https://i.imgur.com.evil.com/x.png",
            "https://evil.com/i.imgur.com/x.png",
            "https://img.shields.io/badge/x.svg",
            "https://github.com/o/r",
            "https://github.com/o/r/issues/1",
            "https://imgur.com/a/album",
            "https://imgur.com/gallery/x",
            "https://objects.githubusercontent.com/x.png",
            "https://github-production-user-asset-6210df.s3.amazonaws.com/x.png",
            "javascript:alert(1)",
        ] {
            assert_eq!(image_url(bad), None, "{bad}");
        }
    }

    #[test]
    fn links_to_images_become_the_files() {
        assert_eq!(
            image_url("https://github.com/o/r/blob/main/docs/shot.png?raw=true").as_deref(),
            Some("https://raw.githubusercontent.com/o/r/main/docs/shot.png")
        );
        assert_eq!(
            image_url("https://github.com/o/r/raw/main/shot.png").as_deref(),
            Some("https://raw.githubusercontent.com/o/r/main/shot.png")
        );
        assert_eq!(
            image_url("https://github.com/user-attachments/assets/1234-abcd").as_deref(),
            Some("https://github.com/user-attachments/assets/1234-abcd")
        );
        assert_eq!(
            image_url("https://imgur.com/AbC123.png").as_deref(),
            Some("https://i.imgur.com/AbC123.png")
        );
    }

    #[test]
    fn redirects_stay_on_image_hosts() {
        let ok = |u: &str| redirect_allowed(&Url::parse(u).unwrap());
        assert!(ok(
            "https://private-user-images.githubusercontent.com/1/2.png?jwt=x"
        ));
        assert!(ok("https://objects.githubusercontent.com/x"));
        assert!(ok(
            "https://github-production-user-asset-6210df.s3.amazonaws.com/1/2.png?X-Amz-Signature=x"
        ));
        assert!(!ok("https://other-bucket.s3.amazonaws.com/x.png"));
        assert!(ok("https://i.imgur.com/removed.png"));
        assert!(!ok("https://imgur.com/AbC123"));
        assert!(!ok("http://i.imgur.com/x.png"));
        assert!(!ok("https://evil.com/x.png"));
    }

    #[test]
    fn mirrors_every_host() {
        let mirror = Url::parse("http://127.0.0.1:9/m/").unwrap();
        let url = Url::parse("https://i.imgur.com/a.png?x=1").unwrap();
        assert_eq!(
            mirrored(&url, Some(&mirror)),
            "http://127.0.0.1:9/m/i.imgur.com/a.png?x=1"
        );
        assert_eq!(mirrored(&url, None), "https://i.imgur.com/a.png?x=1");
    }
}
