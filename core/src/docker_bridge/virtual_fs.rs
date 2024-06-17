use std::path::{Path, PathBuf};

use crate::util::scoped_join_win_safe;

/// relative_path must start with a component that matches one of the roots
///
/// For ex.
///
/// `["/home/user", "/home/user/other"]`` and `"other/file.txt"`` is valid
///
/// The function will match the "other" component with the second root
/// and return ("/home/user/other/file.txt", "/home/user/other", "other")
///
/// no backlinks are allowed in the relative_path, the function will return None
pub fn get_virtual_path(
    virtual_roots: &[PathBuf],
    relative_path: &Path,
) -> Option<(PathBuf, PathBuf, String)> {
    let safe_relative_path = scoped_join_win_safe("/", relative_path).ok()?;
    let request_mount_point = safe_relative_path
        .components()
        .nth(1)?
        .as_os_str()
        .to_str()?;
    let request_path_without_mount_point = safe_relative_path
        .strip_prefix("/".to_string() + request_mount_point)
        .ok()?;
    let mount_point = virtual_roots
        .iter()
        .find(|m| m.ends_with(request_mount_point))?;
    let path = mount_point.join(request_path_without_mount_point);
    Some((path, mount_point.clone(), request_mount_point.to_string()))
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
        let (virtual_path, _, mount_point) = get_virtual_path(&roots, relative_path).unwrap();
        assert_eq!(virtual_path, Path::new("/user/other/file.txt"));
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
