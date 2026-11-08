//! Parsing of modrinth.com links, and checks for ids that go into Modrinth API URLs.

use crate::Error;

/// The project (and maybe version) a modrinth.com link points to, before it is looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModrinthUrl {
    /// A slug or a project id.
    pub project: String,
    /// A version number or a version id.
    pub version: Option<String>,
}

const PROJECT_PAGES: [&str; 7] = [
    "mod",
    "plugin",
    "modpack",
    "datapack",
    "resourcepack",
    "shader",
    "project",
];

fn not_a_modrinth_link() -> Error {
    Error::Invalid("Not a Modrinth project link".to_string())
}

/// Whether `value` can be used as one path segment of a Modrinth API URL: a slug, a
/// project or version id, or a version number. Rejects anything that could change the
/// meaning of the URL (separators, `?`, `#`, `%`, dot segments) and control characters.
pub fn is_valid_api_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '-' | '_' | '.' | '+' | '!' | '@' | '$' | '(' | ')' | ',' | '\'' | '~'
                )
        })
}

/// Checks a slug, id or version number sent by a client, see [`is_valid_api_segment`].
pub fn check_api_segment<'a>(value: &'a str, what: &str) -> Result<&'a str, Error> {
    if is_valid_api_segment(value) {
        Ok(value)
    } else {
        Err(Error::Invalid(format!("Invalid {what}")))
    }
}

/// Parses a modrinth.com project link:
///
/// - `https://modrinth.com/{mod|plugin|modpack|datapack|resourcepack|shader}/{slug_or_id}`,
///   optionally followed by `/version/{version_number_or_id}`, or by another sub-page such
///   as `/versions` or `/gallery`, which is ignored;
/// - `https://modrinth.com/project/{id}`;
/// - the same with `http://`, without a scheme, or with `www.`.
///
/// Anything else, including other hosts, is rejected with "Not a Modrinth project link".
pub fn parse_modrinth_url(input: &str) -> Result<ParsedModrinthUrl, Error> {
    let input = input.trim();
    if input.is_empty() || input.len() > 2048 {
        return Err(not_a_modrinth_link());
    }
    let with_scheme = if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| not_a_modrinth_link())?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(not_a_modrinth_link());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host != "modrinth.com" && host != "www.modrinth.com" {
        return Err(not_a_modrinth_link());
    }
    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let (page, project) = match segments.as_slice() {
        [page, project, ..] => (*page, *project),
        _ => return Err(not_a_modrinth_link()),
    };
    if !PROJECT_PAGES.contains(&page) || !is_valid_api_segment(project) {
        return Err(not_a_modrinth_link());
    }
    let version = match segments.get(2..) {
        Some(["version", version, ..]) => {
            if !is_valid_api_segment(version) {
                return Err(not_a_modrinth_link());
            }
            Some(version.to_string())
        }
        _ => None,
    };
    Ok(ParsedModrinthUrl {
        project: project.to_string(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(project: &str, version: Option<&str>) -> ParsedModrinthUrl {
        ParsedModrinthUrl {
            project: project.to_string(),
            version: version.map(String::from),
        }
    }

    #[test]
    fn accepts_project_links() {
        for page in [
            "mod",
            "plugin",
            "modpack",
            "datapack",
            "resourcepack",
            "shader",
        ] {
            assert_eq!(
                parse_modrinth_url(&format!("https://modrinth.com/{page}/fabric-api")).unwrap(),
                parsed("fabric-api", None),
                "{page}"
            );
        }
        assert_eq!(
            parse_modrinth_url("https://modrinth.com/project/P7dR8mSH").unwrap(),
            parsed("P7dR8mSH", None)
        );
    }

    #[test]
    fn accepts_links_without_scheme_or_with_www() {
        for url in [
            "modrinth.com/mod/sodium",
            "www.modrinth.com/mod/sodium",
            "https://www.modrinth.com/mod/sodium",
            "http://modrinth.com/mod/sodium",
            "  https://MODRINTH.com/mod/sodium/  ",
            "https://modrinth.com/mod/sodium?tab=x#top",
        ] {
            assert_eq!(
                parse_modrinth_url(url).unwrap(),
                parsed("sodium", None),
                "{url}"
            );
        }
    }

    #[test]
    fn reads_version_and_ignores_sub_pages() {
        assert_eq!(
            parse_modrinth_url("https://modrinth.com/mod/fabric-api/version/0.92.2+1.20.1")
                .unwrap(),
            parsed("fabric-api", Some("0.92.2+1.20.1"))
        );
        assert_eq!(
            parse_modrinth_url("modrinth.com/plugin/luckperms/version/OrIs0S6b/").unwrap(),
            parsed("luckperms", Some("OrIs0S6b"))
        );
        for sub in ["versions", "gallery", "changelog", "settings/links"] {
            assert_eq!(
                parse_modrinth_url(&format!("https://modrinth.com/mod/sodium/{sub}")).unwrap(),
                parsed("sodium", None),
                "{sub}"
            );
        }
        // "/version" with nothing after it is ignored too
        assert_eq!(
            parse_modrinth_url("https://modrinth.com/mod/sodium/version").unwrap(),
            parsed("sodium", None)
        );
    }

    #[test]
    fn rejects_other_hosts_and_paths() {
        for url in [
            "",
            "   ",
            "https://example.com/mod/sodium",
            "https://modrinth.com.evil.com/mod/sodium",
            "https://evilmodrinth.com/mod/sodium",
            "https://api.modrinth.com/v2/project/sodium",
            "https://cdn.modrinth.com/data/abc/versions/x.jar",
            "https://modrinth.com/",
            "https://modrinth.com/mod",
            "https://modrinth.com/mods",
            "https://modrinth.com/user/someone",
            "https://modrinth.com/mods/sodium",
            "https://modrinth.com/mod/..",
            "https://modrinth.com/mod/a%2Fb",
            "https://modrinth.com/mod/sodium/version/a%2F..",
            "https://user:pw@modrinth.com/mod/sodium",
            "https://modrinth.com:8443/mod/sodium",
            "ftp://modrinth.com/mod/sodium",
            "javascript:alert(1)",
            "not a url at all",
        ] {
            let err = parse_modrinth_url(url).unwrap_err();
            assert!(matches!(err, Error::Invalid(_)), "{url}");
            assert_eq!(err.to_string(), "Not a Modrinth project link", "{url}");
        }
    }

    #[test]
    fn api_segments() {
        for ok in ["fabric-api", "P7dR8mSH", "0.92.2+1.20.1", "1.0", "a_b"] {
            assert!(is_valid_api_segment(ok), "{ok}");
        }
        for bad in [
            "", ".", "..", "a/b", "a\\b", "a?b", "a#b", "a%2F", "a b", "a\nb", "ü",
        ] {
            assert!(!is_valid_api_segment(bad), "{bad:?}");
        }
        assert!(!is_valid_api_segment(&"a".repeat(129)));
    }
}
