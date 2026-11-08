//! Looking up modrinth.com links. The parsing, and the checks for ids that go into Modrinth
//! API URLs, are in [`lodestone_minecraft::modrinth::links`].

pub use lodestone_minecraft::modrinth::links::*;

use super::{ModrinthApi, ModrinthUrlTarget};
use crate::error::Error;

/// Looks up what a modrinth.com link points to: the project's canonical id, slug and type,
/// and the version id if the link names a version.
pub async fn resolve_url(api: &dyn ModrinthApi, input: &str) -> Result<ModrinthUrlTarget, Error> {
    let parsed = parse_modrinth_url(input)?;
    let project = api.project(&parsed.project).await?;
    let version_id = match &parsed.version {
        Some(version) => Some(api.project_version(&project.id, version).await?.id),
        None => None,
    };
    Ok(ModrinthUrlTarget {
        project_id: project.id,
        slug: project.slug,
        project_type: project.project_type,
        version_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[tokio::test]
    async fn resolves_with_project_lookup() {
        use crate::modrinth::fake::{FakeModrinth, FakeVersion};
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("abc123", "v1", "a.jar"));
        let target = resolve_url(&fake, "https://modrinth.com/mod/abc123/version/v1-number")
            .await
            .unwrap();
        assert_eq!(target.project_id, "abc123");
        assert_eq!(target.version_id.as_deref(), Some("v1"));
        let target = resolve_url(&fake, "modrinth.com/project/abc123/gallery")
            .await
            .unwrap();
        assert_eq!(target.version_id, None);
        let err = resolve_url(&fake, "https://modrinth.com/mod/missing")
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
    }
}
