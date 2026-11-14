use std::path::{Component, Path, PathBuf};

/// Splits `relative_path` into its first component, which names one of the container's
/// mounts, and the path inside that mount, without touching the disk (a link on this
/// computer must not change which mount a path is in).
///
/// No backlinks are allowed: a path with `..` (or a drive prefix) gives `None`, as does one
/// with no mount name.
pub fn split_virtual_path(relative_path: &Path) -> Option<(String, PathBuf)> {
    let mut parts = Vec::new();
    for component in relative_path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => parts.push(part),
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    let (mount_name, inside) = parts.split_first()?;
    Some((mount_name.to_str()?.to_string(), inside.iter().collect()))
}

/// relative_path must start with a component that matches one of the roots
///
/// For ex.
///
/// `["/home/user", "/home/user/other"]`` and `"other/file.txt"`` is valid
///
/// The function will match the "other" component with the second root
/// and return ("/home/user/other", "file.txt", "other"): the mount's root, the path inside
/// it (still to be joined to the root without leaving it), and the mount's name.
///
/// no backlinks are allowed in the relative_path, the function will return None
pub fn get_virtual_path(
    virtual_roots: &[PathBuf],
    relative_path: &Path,
) -> Option<(PathBuf, PathBuf, String)> {
    let (mount_name, inside) = split_virtual_path(relative_path)?;
    let mount_point = virtual_roots.iter().find(|m| {
        m.file_name()
            .is_some_and(|name| name == mount_name.as_str())
    })?;
    Some((mount_point.clone(), inside, mount_name))
}
/// Given an absolute path, a virtual root, and a mount point, return the virtual path that is situated at the mount point
///
/// For ex.
///
/// `to_virtual_path("/home/user/other/file.txt", "/home/user", "mount")` will return `Some("mount/other/file.txt")`
///
/// This function is prone to backlinks, so it is recommended to use it with a path that is already validated
pub fn to_virtual_path(
    absolute_path: &Path,
    absolute_virtual_root: &Path,
    mount_point: &str,
) -> Option<PathBuf> {
    Some(PathBuf::from(mount_point).join(absolute_path.strip_prefix(absolute_virtual_root).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_virtual_path() {
        let roots = [PathBuf::from("/home/user"), PathBuf::from("/user/other")];
        let relative_path = Path::new("other/file.txt");
        let (root, inside, mount_point) = get_virtual_path(&roots, relative_path).unwrap();
        assert_eq!(root, Path::new("/user/other"));
        assert_eq!(inside, Path::new("file.txt"));
        assert_eq!(mount_point, "other");
        let (root, inside, _) = get_virtual_path(&roots, Path::new("/user/./a/b")).unwrap();
        assert_eq!(root, Path::new("/home/user"));
        assert_eq!(inside, Path::new("a/b"));
    }

    #[test]
    fn backlinks_and_unknown_mounts_are_refused() {
        let roots = [PathBuf::from("/srv/data")];
        for path in [
            "",
            "/",
            "other/file",
            "data/../data/x",
            "data/a/../../b",
            "../data/x",
        ] {
            assert!(
                get_virtual_path(&roots, Path::new(path)).is_none(),
                "{path}"
            );
        }
        // the name must be the mount's whole last component
        assert!(get_virtual_path(&roots, Path::new("ata/x")).is_none());
    }

    #[test]
    fn test_to_virtual_path() {
        let absolute_path = Path::new("/home/user/other/file.txt");
        let virtual_root = Path::new("/home/user");
        let mount_point = "mount";
        let virtual_path = to_virtual_path(absolute_path, virtual_root, mount_point).unwrap();
        assert_eq!(virtual_path, Path::new("mount/other/file.txt"));
    }
}
