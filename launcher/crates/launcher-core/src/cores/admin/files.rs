//! The Files tab: an instance's file manager (`utils/apis.ts`, `data/FileSystem.ts`).
//! Paths are relative to the instance's folder, in the core's own separator, and go
//! in URLs as URL-safe Base64 without padding (`Base64.encode(path, true)`).
//!
//! Work package B (see `launcher/docs/host-parity.md`, §5).

use std::{path::Path, time::Duration};

use base64::Engine as _;
use http_client::Method;
use serde::{Deserialize, Serialize};

use super::{AdminClient, segment};
use crate::cores::{CoreError, client::MAX_JSON_BYTES};
use crate::game::download::{CancelToken, Progress};

/// The largest file the core reads for the editor (`MAX_READ_BYTES` in the core's
/// `global_fs.rs`).
pub const MAX_READ_BYTES: u64 = 16 * 1024 * 1024;
/// The largest body the core takes for a file it writes (`MAX_WRITE_BODY_BYTES`).
pub const MAX_WRITE_BYTES: usize = 64 * 1024 * 1024;
/// How long zipping a folder for download may take (the dashboard waits 60 s).
pub const ZIP_FOR_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long an upload may take (the dashboard has no limit).
pub const UPLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// The largest download saved (the core streams any file).
pub const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// The archives the core unzips (`supportedZip`).
pub const ARCHIVE_EXTENSIONS: [&str; 3] = ["zip", "gz", "tgz"];

/// What an entry of a folder is (`FileType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    #[default]
    #[serde(other)]
    Unknown,
}

/// An entry of a folder (`ClientFile`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientFile {
    pub name: String,
    #[serde(default)]
    pub file_stem: String,
    #[serde(default)]
    pub extension: Option<String>,
    /// Relative to the instance's folder.
    pub path: String,
    #[serde(default)]
    pub size: Option<u64>,
    /// Unix seconds.
    #[serde(default)]
    pub creation_time: Option<u64>,
    #[serde(default)]
    pub modification_time: Option<u64>,
    #[serde(default)]
    pub file_type: FileType,
}

impl ClientFile {
    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Whether the core can unzip it.
    pub fn is_archive(&self) -> bool {
        self.file_type == FileType::File
            && self
                .extension
                .as_deref()
                .is_some_and(|ext| ARCHIVE_EXTENSIONS.contains(&ext))
    }
}

/// Folders first, then files, each by name (`fileSorter`: by type, then name).
pub fn sort_files(files: &mut [ClientFile]) {
    let rank = |file: &ClientFile| match file.file_type {
        FileType::Directory => 0,
        FileType::File => 1,
        FileType::Unknown => 2,
    };
    files.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name)));
}

/// How to unzip (`UnzipOption`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum UnzipOption {
    /// Into the archive's folder.
    Normal,
    /// Into the archive's folder, without a single top folder the archive holds.
    Smart,
    /// Into a folder named after the archive.
    ToDirectoryWithFileName,
    ToDir(String),
}

/// The separator of the core's paths: `/`, or `\` for a core on Windows, from an
/// instance's absolute path (`getDirectorySeparator`).
pub fn separator(instance_path: &str) -> char {
    if instance_path.contains('/') {
        '/'
    } else {
        '\\'
    }
}

/// `name` in `directory` (`joinPath`): `.` and `a` give `.\a` or `./a`.
pub fn join(directory: &str, name: &str, separator: char) -> String {
    if directory.is_empty() {
        name.to_string()
    } else if directory.ends_with(separator) {
        format!("{directory}{name}")
    } else {
        format!("{directory}{separator}{name}")
    }
}

/// The folder `path` is in (`parentPath`): `.` at the top.
pub fn parent(path: &str, separator: char) -> String {
    match path.rsplit_once(separator) {
        Some((parent, _)) if !parent.is_empty() => parent.to_string(),
        _ => ".".to_string(),
    }
}

/// A path's last part.
pub fn file_name(path: &str, separator: char) -> &str {
    path.rsplit(separator).next().unwrap_or(path)
}

/// A path as one URL segment.
fn b64(path: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path.as_bytes())
}

fn fs(uuid: &str, path: &str, action: &str) -> String {
    format!("instance/{}/fs/{}/{action}", segment(uuid), b64(path))
}

/// A `multipart/form-data` body with a `file` field per file, as the dashboard's
/// upload sends it. Returns the body and its content type.
pub fn multipart_files(files: &[(String, Vec<u8>)]) -> (Vec<u8>, String) {
    let boundary = format!(
        "lodestone-host-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    );
    let mut body = Vec::new();
    for (name, bytes) in files {
        // quotes and line breaks would end the header
        let safe: String = name
            .chars()
            .map(|c| if c == '"' || c.is_control() { '_' } else { c })
            .collect();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

impl AdminClient {
    /// `GET …/ls`: the folder's entries, folders first, then by name.
    pub async fn list_files(&self, uuid: &str, path: &str) -> Result<Vec<ClientFile>, CoreError> {
        let mut files: Vec<ClientFile> = self.get(&fs(uuid, path, "ls")).await?;
        sort_files(&mut files);
        Ok(files)
    }

    /// `GET …/read`: a text file's contents (at most [`MAX_READ_BYTES`]).
    pub async fn read_file(&self, uuid: &str, path: &str) -> Result<String, CoreError> {
        let bytes = self
            .send_raw(
                Method::GET,
                &fs(uuid, path, "read"),
                None,
                MAX_READ_BYTES + 1024,
            )
            .await?;
        String::from_utf8(bytes).map_err(|_| CoreError::Invalid("The file isn't text.".into()))
    }

    /// `PUT …/write`: replaces a file's contents.
    pub async fn write_file(&self, uuid: &str, path: &str, text: &str) -> Result<(), CoreError> {
        if text.len() > MAX_WRITE_BYTES {
            return Err(CoreError::Invalid("The file is too large to save.".into()));
        }
        self.send_raw(
            Method::PUT,
            &fs(uuid, path, "write"),
            Some((text.as_bytes().to_vec(), "text/plain; charset=utf-8".into())),
            MAX_JSON_BYTES,
        )
        .await
        .map(drop)
    }

    /// `PUT …/new`: an empty file.
    pub async fn new_file(&self, uuid: &str, path: &str) -> Result<(), CoreError> {
        self.call(Method::PUT, &fs(uuid, path, "new"), None::<()>)
            .await
    }

    /// `PUT …/mkdir`.
    pub async fn new_folder(&self, uuid: &str, path: &str) -> Result<(), CoreError> {
        self.call(Method::PUT, &fs(uuid, path, "mkdir"), None::<()>)
            .await
    }

    /// `DELETE …/rm` for a file, `…/rmdir` for a folder (with what it holds).
    pub async fn delete_path(&self, uuid: &str, path: &str, dir: bool) -> Result<(), CoreError> {
        let action = if dir { "rmdir" } else { "rm" };
        self.call(Method::DELETE, &fs(uuid, path, action), None::<()>)
            .await
    }

    /// `PUT …/{source}/move/{destination}`: moves or renames.
    pub async fn move_path(
        &self,
        uuid: &str,
        source: &str,
        destination: &str,
    ) -> Result<(), CoreError> {
        let path = format!(
            "instance/{}/fs/{}/move/{}",
            segment(uuid),
            b64(source),
            b64(destination)
        );
        self.call(Method::PUT, &path, None::<()>).await
    }

    /// `PUT /instance/{uuid}/fs/cpr`: copies files and folders into `destination`.
    pub async fn copy_paths(
        &self,
        uuid: &str,
        sources: &[String],
        destination: &str,
    ) -> Result<(), CoreError> {
        let body = serde_json::json!({
            "relative_paths_source": sources,
            "relative_path_dest": destination,
        });
        self.call(
            Method::PUT,
            &format!("instance/{}/fs/cpr", segment(uuid)),
            Some(body),
        )
        .await
    }

    /// `PUT /instance/{uuid}/fs/zip`: zips `targets` into `destination` in the
    /// background; a `FSOperationCompleted` event says how it went.
    pub async fn zip_paths(
        &self,
        uuid: &str,
        targets: &[String],
        destination: &str,
    ) -> Result<(), CoreError> {
        let body = serde_json::json!({
            "target_relative_paths": targets,
            "destination_relative_path": destination,
        });
        self.call(
            Method::PUT,
            &format!("instance/{}/fs/zip", segment(uuid)),
            Some(body),
        )
        .await
    }

    /// `PUT …/unzip`: unzips in the background; a `FSOperationCompleted` event says
    /// how it went.
    pub async fn unzip(
        &self,
        uuid: &str,
        path: &str,
        option: &UnzipOption,
    ) -> Result<(), CoreError> {
        self.call(Method::PUT, &fs(uuid, path, "unzip"), Some(option))
            .await
    }

    /// `PUT …/upload`: uploads files (name, contents) into the folder `directory`.
    pub async fn upload_files(
        &self,
        uuid: &str,
        directory: &str,
        files: &[(String, Vec<u8>)],
    ) -> Result<(), CoreError> {
        let body = multipart_files(files);
        self.send_slow(
            Method::PUT,
            &fs(uuid, directory, "upload"),
            Some(body),
            MAX_JSON_BYTES,
            UPLOAD_TIMEOUT,
        )
        .await
        .map(drop)
    }

    /// `GET …/url`: a key for `GET /file/{key}`. The core zips a folder first.
    pub async fn download_key(
        &self,
        uuid: &str,
        path: &str,
        dir: bool,
    ) -> Result<String, CoreError> {
        let timeout = if dir {
            ZIP_FOR_DOWNLOAD_TIMEOUT
        } else {
            super::super::client::REQUEST_TIMEOUT
        };
        let bytes = self
            .send_slow(Method::GET, &fs(uuid, path, "url"), None, 4096, timeout)
            .await?;
        // a plain string, or (from some cores) a JSON one
        let text = String::from_utf8_lossy(&bytes).trim().to_string();
        Ok(serde_json::from_str::<String>(&text).unwrap_or(text))
    }

    /// Downloads a file (a folder as a zip) into `out`.
    pub async fn download_to(
        &self,
        uuid: &str,
        file: &ClientFile,
        out: &Path,
    ) -> Result<u64, CoreError> {
        let key = self.download_key(uuid, &file.path, file.is_dir()).await?;
        self.core()
            .download_file(
                &key,
                out,
                MAX_DOWNLOAD_BYTES,
                &Progress::default(),
                &CancelToken::new(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_follow_the_cores_separator() {
        assert_eq!(separator("/srv/lodestone/instances/a"), '/');
        assert_eq!(separator("C:\\lodestone\\instances\\a"), '\\');
        assert_eq!(join(".", "a", '/'), "./a");
        assert_eq!(join(".", "a", '\\'), ".\\a");
        assert_eq!(join("config/", "a", '/'), "config/a");
        assert_eq!(join("", "a", '/'), "a");
        assert_eq!(parent("config/paper.yml", '/'), "config");
        assert_eq!(parent("server.properties", '/'), ".");
        assert_eq!(parent("./config", '/'), ".");
        assert_eq!(file_name("config/paper.yml", '/'), "paper.yml");
    }

    #[test]
    fn paths_are_url_safe_base64() {
        assert_eq!(b64("."), "Lg");
        assert_eq!(b64("a/b?c"), "YS9iP2M");
        assert!(!b64("config/ü>?").contains(['+', '/', '=']));
    }

    #[test]
    fn folders_come_first() {
        let file = |name: &str, file_type| ClientFile {
            name: name.into(),
            file_stem: name.into(),
            extension: None,
            path: name.into(),
            size: None,
            creation_time: None,
            modification_time: None,
            file_type,
        };
        let mut files = vec![
            file("b.txt", FileType::File),
            file("world", FileType::Directory),
            file("a.txt", FileType::File),
            file("config", FileType::Directory),
        ];
        sort_files(&mut files);
        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["config", "world", "a.txt", "b.txt"]);
    }

    #[test]
    fn unzip_options_are_the_cores() {
        assert_eq!(
            serde_json::to_value(UnzipOption::Smart).unwrap(),
            serde_json::json!("Smart")
        );
        assert_eq!(
            serde_json::to_value(UnzipOption::ToDir("x".into())).unwrap(),
            serde_json::json!({ "ToDir": "x" })
        );
        let file: ClientFile = serde_json::from_value(serde_json::json!({
            "name": "a.zip", "file_stem": "a", "extension": "zip", "path": "a.zip",
            "size": 3, "creation_time": null, "modification_time": 1, "file_type": "File"
        }))
        .unwrap();
        assert!(file.is_archive());
    }

    #[test]
    fn multipart_has_a_field_per_file() {
        let (body, content_type) = multipart_files(&[
            ("a.txt".into(), b"one".to_vec()),
            ("b\".txt".into(), b"two".to_vec()),
        ]);
        let body = String::from_utf8(body).unwrap();
        let boundary = content_type.split("boundary=").nth(1).unwrap();
        assert_eq!(body.matches(&format!("--{boundary}\r\n")).count(), 2);
        assert!(body.contains("filename=\"b_.txt\""));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
    }
}
