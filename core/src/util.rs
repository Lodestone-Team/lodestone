use color_eyre::eyre::{eyre, Context, ContextCompat};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::LazyLock;
use std::time::Duration;

use rand::distr::Alphanumeric;
use rand::RngExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use flate2::read::GzDecoder;
use tar::Archive;

use crate::error::{Error, ErrorKind};
use crate::prelude::path_to_tmp;

/// Lodestone's user agent. Some APIs (Modrinth, PaperMC) ask clients to identify
/// themselves. No personal data.
pub const USER_AGENT: &str = concat!(
    "Lodestone/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Lodestone-Team/lodestone)"
);

/// The settings of [`http_client`]: a connection is given up after 10 s and a response
/// that stalls for 60 s, but there is no overall timeout, since downloads can be large.
/// Requests that should end sooner set their own with [`reqwest::RequestBuilder::timeout`].
pub(crate) fn http_client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
}

/// One HTTP client for the core's outgoing requests, so connections are reused. It is
/// built on first use, not at startup: building a client reads the system's proxy settings
/// and CA certificates (on Linux it parses the whole bundle), which a core that makes no
/// request shouldn't pay for. See [`http_client_builder`] for its timeouts.
pub fn http_client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        http_client_builder()
            .build()
            .unwrap_or_else(|_| Client::new())
    });
    &CLIENT
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total: Option<u64>,
    pub downloaded: u64,
    pub step: u64,
    pub download_name: String,
}

/// Extracts a safe file name from a `Content-Disposition` header value.
///
/// The name comes from a remote server, so only its last path segment is kept and it is
/// sanitized. Returns `None` if the header has no usable `filename` parameter.
fn file_name_from_content_disposition(header: &str) -> Option<String> {
    let raw = header.split(';').skip(1).find_map(|param| {
        let (key, value) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("filename")
            .then(|| value.trim().trim_matches('"'))
    })?;
    // Keep only the last segment, for both Unix and Windows separators.
    let last_segment = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let name = sanitize_filename::sanitize(last_segment);
    let name = name.trim();
    if name.is_empty() || name.chars().all(|c| c == '.') {
        None
    } else {
        Some(name.to_string())
    }
}

pub async fn download_file(
    url: &str,
    path: &Path,
    name_override: Option<&str>,
    on_download: &(dyn Fn(DownloadProgress) + Send + Sync),
    overwrite_old: bool,
) -> Result<PathBuf, Error> {
    let lodestone_tmp = path_to_tmp().clone();
    tokio::fs::create_dir_all(&lodestone_tmp)
        .await
        .context("Failed to create tmp dir")?;
    // Dropping the TempPath deletes the partial download if anything below fails.
    let temp_file_path = tempfile::NamedTempFile::new_in(lodestone_tmp)
        .context("Failed to create temporary file")?
        .into_temp_path();
    let mut temp_file = tokio::fs::File::create(&temp_file_path)
        .await
        .context("Failed to create temporary file")?;
    let response = http_client()
        .get(url)
        .send()
        .await
        .context("Failed to send GET request")?;
    response
        .error_for_status_ref()
        .context("Failed to download file")?;
    tokio::fs::create_dir_all(path)
        .await
        .context(format!("Failed to create dir {}", path.display()))?;

    let file_name = match name_override {
        Some(name) => name.to_string(),
        None => response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|h| h.to_str().ok())
            .and_then(file_name_from_content_disposition)
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let dest = path.join(&file_name);
    if !overwrite_old && dest.exists() {
        return Err(eyre!("File {} already exists", dest.display()).into());
    }
    let total_size = response.content_length();

    let mut downloaded: u64 = 0;
    let mut new_downloaded: u64 = 0;
    let threshold = total_size.unwrap_or(500000) / 100;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.context("Failed to read response")?;
        temp_file
            .write_all(&chunk)
            .await
            .context(format!("Failed to write to file {}", file_name))?;
        new_downloaded += chunk.len() as u64;
        let step = new_downloaded - downloaded;
        if step > threshold {
            on_download(DownloadProgress {
                total: total_size,
                downloaded,
                step,
                download_name: file_name.clone(),
            });
            downloaded = new_downloaded;
        }
    }
    temp_file
        .flush()
        .await
        .context(format!("Failed to write to file {}", file_name))?;
    drop(temp_file);
    tokio::fs::rename(&temp_file_path, &dest)
        .await
        .context(format!("Failed to rename file {}", file_name))?;
    Ok(dest)
}

/// List all files in a directory
/// files_or_dir = 0 -> files, 1 -> directories
pub async fn list_dir(
    path: &Path,
    filter_file_or_dir: Option<bool>,
) -> Result<Vec<PathBuf>, Error> {
    let ret: Result<Vec<PathBuf>, Error> = tokio::task::spawn_blocking({
        let path = path.to_owned();
        move || {
            Ok(std::fs::read_dir(&path)
                .context(format!("failed to read directory {}", path.display()))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok())
                .filter(|entry| match filter_file_or_dir {
                    // unwrap is safe because we checked if file_type is ok
                    Some(true) => entry.file_type().unwrap().is_dir(),
                    Some(false) => entry.file_type().unwrap().is_file(),
                    None => true,
                })
                .map(|entry| entry.path())
                .collect())
        }
    })
    .await
    .context("Failed to list directory")?;
    ret
}

pub fn resolve_path_conflict(path: PathBuf, predicate: Option<&dyn Fn(&Path) -> bool>) -> PathBuf {
    let predicate = predicate.unwrap_or(&Path::exists);
    let name = path
        .file_stem()
        .unwrap_or(OsStr::new("unknown"))
        .to_string_lossy()
        .to_string();
    let ext = path.extension().map(|s| s.to_os_string());

    if !predicate(&path) {
        return path;
    }

    for i in 1.. {
        let mut tmp = path.clone();
        let name_with_suffix = match ext {
            Some(ref ext) => format!("{}_{}.{}", name, i, ext.to_string_lossy()),
            None => format!("{}_{}", name, i),
        };
        tmp.set_file_name(&name_with_suffix);
        if !predicate(&tmp) {
            return tmp;
        }
    }

    path // Unreachable code
}

#[derive(Serialize, Deserialize, Debug, Clone, TS, PartialEq, Eq)]
#[ts(export)]
pub enum UnzipOption {
    /// Unzip to the same directory as the file
    Normal,
    /// Unzip to the same directory as the file while avoiding spillage
    Smart,
    /// Unzip to a folder with the same name as the file
    ToDirectoryWithFileName,
    /// Unzip to a custom folder
    ToDir(PathBuf),
}

/// Called for every entry of an archive after it was extracted to a temporary folder and
/// before anything is moved to the destination. It gets the entry's path relative to the
/// archive root and its file type (symbolic links are not followed). Returning an error
/// aborts the extraction and leaves the destination untouched.
pub type UnzipEntryCheck = dyn Fn(&Path, &std::fs::FileType) -> Result<(), Error> + Send + Sync;

pub fn unzip_file(
    file: impl AsRef<Path>,
    unzip_option: UnzipOption,
) -> Result<HashSet<PathBuf>, Error> {
    unzip_file_in(file.as_ref(), unzip_option, path_to_tmp(), None)
}

fn unzip_file_in(
    file: &Path,
    unzip_option: UnzipOption,
    tmp_root: &Path,
    check_entry: Option<&UnzipEntryCheck>,
) -> Result<HashSet<PathBuf>, Error> {
    if !file.exists() {
        return Err(eyre!("File {} does not exist", file.display()).into());
    }

    let file_extension = file
        .extension()
        .ok_or_else(|| eyre!("Failed to get file extension for {}", file.display()))?;
    if file_extension != "gz" && file_extension != "tgz" && file_extension != "zip" {
        return Err(eyre!("Unsupported extension for {}", file.display()).into());
    }

    let parent = file.parent().context(format!(
        "Failed to get parent directory of {}",
        file.display()
    ))?;

    let file_stem = file
        .file_stem()
        .context(format!("Failed to get file stem of {}", file.display()))?;

    std::fs::create_dir_all(tmp_root).context(format!(
        "Failed to create temporary directory {}",
        tmp_root.display()
    ))?;

    let temp_dest_dir = tempfile::tempdir_in(tmp_root).context(
        "Failed to create temporary directory for unzipping. Please make sure you have enough space in your disk",
    )?;
    let temp_dest = temp_dest_dir.path();

    if file_extension == "gz" || file_extension == "tgz" {
        let tar_gz =
            std::fs::File::open(file).context(format!("Failed to open file {}", file.display()))?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive.set_overwrite(true);
        archive
            .unpack(temp_dest)
            .context(format!("Failed to decompress file {}", file.display()))?;
    } else if file_extension == "zip" {
        let zip =
            std::fs::File::open(file).context(format!("Failed to open file {}", file.display()))?;
        let mut archive = zip::ZipArchive::new(zip)
            .context(format!("Failed to decompress file {}", file.display()))?;
        // zip >= 2 creates real symlinks (and rejects ones pointing outside the
        // destination); keep the zip 0.6 behaviour of writing them as regular files.
        archive
            .extract_with_options(
                temp_dest,
                zip::read::ExtractOptions::default()
                    .symlink_action(zip::read::SymlinkExtractAction::ExtractAsFile),
            )
            .context(format!("Failed to decompress file {}", file.display()))?;
    }

    if let Some(check_entry) = check_entry {
        for entry in walkdir::WalkDir::new(temp_dest).min_depth(1) {
            let entry = entry.context("Failed to read the extracted files")?;
            let relative_path = entry
                .path()
                .strip_prefix(temp_dest)
                .context("Failed to strip prefix of an extracted file")?;
            check_entry(relative_path, &entry.file_type())?;
        }
    }

    let mut ret: HashSet<PathBuf> = HashSet::new();

    let temp_dir_content = std::fs::read_dir(temp_dest)
        .context(format!("Failed to read directory {}", temp_dest.display()))?
        .map(|entry| entry.map(|v| v.path()))
        .collect::<Result<Vec<_>, _>>()
        .context(format!("Failed to read directory {}", temp_dest.display()))?;

    let dest = match unzip_option {
        UnzipOption::Normal => parent.to_path_buf(),
        // only make a new folder when the archive would otherwise spill several entries
        UnzipOption::Smart if temp_dir_content.len() > 1 => {
            resolve_path_conflict(parent.join(file_stem), None)
        }
        UnzipOption::Smart => parent.to_path_buf(),
        UnzipOption::ToDirectoryWithFileName => resolve_path_conflict(parent.join(file_stem), None),
        UnzipOption::ToDir(d) => d,
    };

    std::fs::create_dir_all(&dest)
        .context(format!("Failed to create directory {}", dest.display()))?;

    for temp_path in temp_dir_content {
        let entry_path = resolve_path_conflict(
            match temp_path.strip_prefix(temp_dest) {
                Ok(p) => dest.join(p),
                Err(_) => continue,
            },
            None,
        );

        std::fs::rename(&temp_path, &entry_path).context(format!(
            "Failed to move {} to {}",
            temp_path.display(),
            entry_path.display()
        ))?;
        ret.insert(entry_path);
    }

    Ok(ret)
}

pub async fn unzip_file_async(
    file: impl AsRef<Path>,
    unzip_option: UnzipOption,
) -> Result<HashSet<PathBuf>, Error> {
    let file = file.as_ref().to_owned();
    let display = file.display().to_string();
    tokio::task::spawn_blocking(move || unzip_file(file, unzip_option))
        .await
        .context(format!("Failed to unzip file {display} in a blocking task"))?
}

/// Like [`unzip_file_async`], but runs `check_entry` on every extracted entry before
/// anything is moved to the destination.
pub async fn unzip_file_checked_async(
    file: impl AsRef<Path>,
    unzip_option: UnzipOption,
    check_entry: Box<UnzipEntryCheck>,
) -> Result<HashSet<PathBuf>, Error> {
    let file = file.as_ref().to_owned();
    let display = file.display().to_string();
    tokio::task::spawn_blocking(move || {
        unzip_file_in(&file, unzip_option, path_to_tmp(), Some(&*check_entry))
    })
    .await
    .context(format!("Failed to unzip file {display} in a blocking task"))?
}

pub fn zip_files(
    files: &[impl AsRef<Path>],
    dest: impl AsRef<Path>,
    overwrite_dest: bool,
) -> Result<PathBuf, Error> {
    zip_files_in(files, dest.as_ref(), overwrite_dest, path_to_tmp())
}

/// Streams the file at `source` into the archive entry that was just started.
fn copy_file_into_zip<W: Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    source: &Path,
) -> Result<(), Error> {
    let mut file =
        std::fs::File::open(source).context(format!("Failed to open {}", source.display()))?;
    std::io::copy(&mut file, writer)
        .context(format!("Failed to write {} to archive", source.display()))?;
    Ok(())
}

/// Zips `files` into `dest`. Symbolic links inside folders are skipped, so a link cannot
/// pull files from outside the folder into the archive.
fn zip_files_in(
    files: &[impl AsRef<Path>],
    dest: &Path,
    overwrite_dest: bool,
    tmp_root: &Path,
) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(dest.parent().context("Failed to get destination parent")?)
        .context(format!("Failed to create directory {}", dest.display()))?;
    std::fs::create_dir_all(tmp_root).context(format!(
        "Failed to create temporary directory {}",
        tmp_root.display()
    ))?;
    let tmp_archive = tempfile::NamedTempFile::new_in(tmp_root)
        .context("Failed to create temporary file for zipping")?;

    let mut writer = zip::ZipWriter::new(tmp_archive.as_file());
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o775);
    for entry_path in files.iter().map(|f| f.as_ref()) {
        let file_type = std::fs::symlink_metadata(entry_path)
            .context(format!(
                "Failed to read metadata of {}",
                entry_path.display()
            ))?
            .file_type();
        let entry_name = entry_path
            .file_name()
            .ok_or_else(|| eyre!("File to zip has no name"))?
            .to_str()
            .ok_or_else(|| eyre!("File to zip has abnormal name"))?;

        if file_type.is_dir() {
            writer.add_directory(entry_name, options).context(format!(
                "Failed to create {} in archive",
                entry_path.display()
            ))?;
            let entry_parent = entry_path
                .parent()
                .context(format!("Failed to get parent for {}", entry_path.display()))?;

            // min_depth(1): the directory itself was added above (zip rejects duplicate names).
            // WalkDir does not follow symbolic links by default.
            for child_entry in walkdir::WalkDir::new(entry_path).min_depth(1) {
                let child_entry = child_entry.context(format!(
                    "Failed to read the contents of {}",
                    entry_path.display()
                ))?;
                let child_entry_path = child_entry.path();
                let child_entry_dest =
                    child_entry_path
                        .strip_prefix(entry_parent)
                        .context(format!(
                            "Failed to strip prefix for {}",
                            child_entry_path.display()
                        ))?;

                if child_entry.file_type().is_dir() {
                    writer
                        .add_directory(child_entry_dest.to_string_lossy(), options)
                        .context(format!(
                            "Failed to create {} in archive",
                            child_entry_path.display()
                        ))?;
                } else if child_entry.file_type().is_file() {
                    writer
                        .start_file(child_entry_dest.to_string_lossy(), options)
                        .context(format!(
                            "Failed to create {} in archive",
                            child_entry_path.display()
                        ))?;
                    copy_file_into_zip(&mut writer, child_entry_path)?;
                }
            }
        } else if file_type.is_file() {
            writer.start_file(entry_name, options).context(format!(
                "Failed to create {} in archive",
                entry_path.display()
            ))?;
            copy_file_into_zip(&mut writer, entry_path)?;
        } else {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "{} is not a regular file or folder and cannot be zipped",
                    entry_path.display()
                ),
            });
        }
    }

    writer.finish().context("Zip failed")?;
    let dest = if overwrite_dest {
        dest.into()
    } else {
        resolve_path_conflict(dest.into(), None)
    };

    tmp_archive.persist(&dest).map_err(|e| {
        eyre!(
            "Failed to move {} to {}: {}",
            e.file.path().display(),
            dest.display(),
            e.error
        )
    })?;
    Ok(dest)
}

pub async fn zip_files_async(
    files: &[impl AsRef<Path>],
    dest: impl AsRef<Path>,
    overwrite_dest: bool,
) -> Result<PathBuf, Error> {
    let files = files
        .iter()
        .map(|f| f.as_ref().to_owned())
        .collect::<Vec<_>>();
    let dest = dest.as_ref().to_owned();
    tokio::task::spawn_blocking(move || zip_files(&files, &dest, overwrite_dest))
        .await
        .context("Failed to spawn blocking task")?
}

pub fn rand_alphanumeric(len: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// Returns true if a TCP listener can be bound to `127.0.0.1:port`.
pub fn local_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// How long [`port_in_use`] waits for an answer. A listener on this machine answers within
/// a millisecond. A closed port is refused at once on Linux and macOS, but Windows tries
/// again for about 2 seconds before it gives up, which would delay every start of the core.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(100);

/// Returns true if something accepts TCP connections on `port` on this machine.
pub fn port_in_use(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, PORT_PROBE_TIMEOUT).is_ok()
}

// safe_path only works on linux and messes up on windows
// this is a hacky solution
pub fn scoped_join_win_safe<R: AsRef<Path>, U: AsRef<Path>>(
    root: R,
    unsafe_path: U,
) -> Result<PathBuf, Error> {
    let mut ret = safe_path::scoped_join(&root, &unsafe_path).context(format!(
        "Failed to join path {} with {}",
        root.as_ref().display(),
        unsafe_path.as_ref().display()
    ))?;
    if cfg!(windows) {
        // construct a new path
        // that replace the prefix component with the component of the root path
        ret = ret
            .components()
            .skip(1)
            .fold(root.as_ref().to_path_buf(), |mut acc, c| {
                acc.push(c.as_os_str());
                acc
            });
    }
    Ok(ret)
}
pub mod fs {
    use std::path::Path;

    use color_eyre::eyre::{eyre, Context};
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;

    use crate::error::{Error, ErrorKind};

    pub async fn remove_file(file: impl AsRef<Path>) -> Result<(), Error> {
        let file = file.as_ref();
        if file.is_file() {
            tokio::fs::remove_file(file)
                .await
                .context(format!("Failed to remove file at {}", file.display()))?;
        }
        Ok(())
    }

    pub async fn write_all(file: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
        let file = file.as_ref();
        tokio::fs::write(file, data)
            .await
            .context(format!("Failed to write to file at {}", file.display()))?;
        Ok(())
    }

    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), Error> {
        let from = from.as_ref();
        let to = to.as_ref();
        tokio::fs::rename(from, to).await.context(format!(
            "Failed to rename file {} to {}",
            from.display(),
            to.display()
        ))?;
        Ok(())
    }

    pub async fn create_dir_all(dir: impl AsRef<Path>) -> Result<(), Error> {
        let dir = dir.as_ref();
        tokio::fs::create_dir_all(dir)
            .await
            .context(format!("Failed to create directory at {}", dir.display()))?;
        Ok(())
    }

    pub async fn remove_dir_all(dir: impl AsRef<Path>) -> Result<(), Error> {
        let dir = dir.as_ref();
        tokio::fs::remove_dir_all(dir)
            .await
            .context(format!("Failed to remove directory at {}", dir.display()))?;
        Ok(())
    }

    pub async fn read_to_string(file: impl AsRef<Path>) -> Result<String, Error> {
        let file = file.as_ref();
        let data = tokio::fs::read_to_string(file)
            .await
            .context(format!("Failed to read file at {}", file.display()))?;
        Ok(data)
    }

    /// Reads a UTF-8 text file, failing with a `BadRequest` error if it is larger than
    /// `max_bytes`. The limit is enforced while reading, so files that report a wrong size
    /// (such as `/dev/zero` or a log that is still growing) cannot exhaust memory.
    pub async fn read_to_string_capped(
        file: impl AsRef<Path>,
        max_bytes: u64,
    ) -> Result<String, Error> {
        let path = file.as_ref();
        let too_large = || Error {
            kind: ErrorKind::BadRequest,
            source: eyre!(
                "{} is larger than {}, download it instead",
                path.display(),
                super::format_byte(max_bytes)
            ),
        };
        let file = File::open(path)
            .await
            .context(format!("Failed to read file at {}", path.display()))?;
        if let Ok(metadata) = file.metadata().await {
            if metadata.len() > max_bytes {
                return Err(too_large());
            }
        }
        let mut data = Vec::new();
        file.take(max_bytes + 1)
            .read_to_end(&mut data)
            .await
            .context(format!("Failed to read file at {}", path.display()))?;
        if data.len() as u64 > max_bytes {
            return Err(too_large());
        }
        String::from_utf8(data).map_err(|_| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("{} is not a UTF-8 text file", path.display()),
        })
    }

    pub async fn create(file: impl AsRef<Path>) -> Result<File, Error> {
        let file = file.as_ref();
        let file = tokio::fs::File::create(file)
            .await
            .context(format!("Failed to create file at {}", file.display()))?;
        Ok(file)
    }
}
/// Starts a console program (Java) without a console on Windows. DETACHED_PROCESS rather
/// than CREATE_NO_WINDOW, which would still give each server a hidden console and a
/// conhost.exe process of its own. Every caller redirects the stdio, nothing sends the
/// servers console control events (they are stopped with `stop` on stdin, or killed), and
/// a Ctrl+C in the terminal the core runs in still reaches only the core, which stops the
/// servers itself. Java decides its output encoding from whether stdout is a console,
/// which a pipe never is, so the output reads the same.
pub fn dont_spawn_terminal(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(target_os = "windows")]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }

    cmd
}

pub use lodestone_minecraft::util::{format_byte, format_byte_download};

/// A `System` with the CPUs (their names and count, not yet their usage or speed) and the
/// memory. `System::new_all()` would also load every process on the host, including each
/// process's environment variables, which Lodestone never needs. Whoever needs the CPUs'
/// usage refreshes it: setting that up takes about 200 ms on Windows, which every start of
/// the core paid before.
pub fn cpu_and_memory_system() -> sysinfo::System {
    sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::nothing())
            .with_memory(sysinfo::MemoryRefreshKind::everything()),
    )
}

#[cfg(test)]
mod tests {
    use crate::util::{
        file_name_from_content_disposition, resolve_path_conflict, unzip_file_in, zip_files_in,
        UnzipOption,
    };
    use color_eyre::eyre::eyre;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    // Every test gets its own temporary folder instead of the global `path_to_tmp()`, which
    // is shared by all tests in the process and races when they run in parallel.

    fn read(path: impl AsRef<Path>) -> String {
        std::fs::read_to_string(path).unwrap().trim().to_string()
    }

    #[test]
    fn test_unzip_file() {
        let tmp = tempfile::tempdir().unwrap();
        let temp = tempfile::TempDir::with_prefix("test_unzip_file").unwrap();
        let temp_path = temp.path();
        let zip = PathBuf::from("testdata/sample.zip");

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg.txt"));
        test.insert(temp_path.join("amendments.txt"));
        test.insert(temp_path.join("constitution.txt"));

        assert_eq!(
            unzip_file_in(
                &zip,
                UnzipOption::ToDir(temp_path.to_owned()),
                tmp.path(),
                None
            )
            .unwrap(),
            test
        );

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg_1.txt"));
        test.insert(temp_path.join("amendments_1.txt"));
        test.insert(temp_path.join("constitution_1.txt"));

        assert_eq!(
            unzip_file_in(
                &zip,
                UnzipOption::ToDir(temp_path.to_owned()),
                tmp.path(),
                None
            )
            .unwrap(),
            test
        );
    }

    #[test]
    fn test_unzip_file_3() {
        let tmp = tempfile::tempdir().unwrap();
        let temp = tempfile::TempDir::with_prefix("test_unzip_file").unwrap();
        let dest_path = temp.path().to_path_buf();
        let tar_gz = PathBuf::from("testdata/sample.gz");

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample"));

        assert_eq!(
            unzip_file_in(
                &tar_gz,
                UnzipOption::ToDir(dest_path.clone()),
                tmp.path(),
                None
            )
            .unwrap(),
            expected
        );
        assert!(dest_path.join("sample").join("sample.exe").is_file());
        assert!(dest_path.join("sample").join("sample.c").is_file());
        assert!(dest_path.join("sample").join("sample.obj").is_file());

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample_1"));

        assert_eq!(
            unzip_file_in(
                &tar_gz,
                UnzipOption::ToDir(dest_path.to_owned()),
                tmp.path(),
                None
            )
            .unwrap(),
            expected
        );
        assert!(dest_path.join("sample_1").join("sample.exe").is_file());
        assert!(dest_path.join("sample_1").join("sample.c").is_file());
        assert!(dest_path.join("sample_1").join("sample.obj").is_file(),);
    }

    #[test]
    fn test_unzip_check_rejects_before_moving() {
        let tmp = tempfile::tempdir().unwrap();
        let temp = tempfile::TempDir::with_prefix("test_unzip_check").unwrap();
        let dest_path = temp.path().join("dest");
        let tar_gz = PathBuf::from("testdata/sample.gz");

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_by_check = seen.clone();
        let check = move |path: &Path, _: &std::fs::FileType| {
            seen_by_check.lock().unwrap().push(path.to_path_buf());
            if path.extension().is_some_and(|e| e == "exe") {
                Err(eyre!("{} is protected", path.display()).into())
            } else {
                Ok(())
            }
        };
        assert!(unzip_file_in(
            &tar_gz,
            UnzipOption::ToDir(dest_path.clone()),
            tmp.path(),
            Some(&check)
        )
        .is_err());
        // nothing reached the destination, and paths were relative to the archive root
        assert!(!dest_path.exists());
        assert!(seen
            .lock()
            .unwrap()
            .contains(&Path::new("sample").join("sample.exe")));
        // the temporary extraction folder was cleaned up
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn test_port_helpers() {
        use crate::util::{local_port_available, port_in_use};
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!local_port_available(port));
        assert!(port_in_use(port));
        drop(listener);
        assert!(local_port_available(port));
        assert!(!port_in_use(port));
    }

    #[test]
    fn test_resolve_path_conflict() {
        let temp = tempfile::TempDir::with_prefix("test_unzip_file").unwrap();
        let temp_path = temp.path();
        let txt_path = temp_path.join("test.txt");
        assert_eq!(resolve_path_conflict(txt_path.clone(), None), txt_path);
        let txt1_path = temp_path.join("test_1.txt");

        let dir = temp_path.join("test");
        assert_eq!(resolve_path_conflict(dir.clone(), None), dir);

        std::fs::create_dir(&dir).unwrap();
        std::fs::write(&txt_path, "test").unwrap();

        assert_eq!(
            resolve_path_conflict(txt_path.clone(), None),
            temp_path.join("test_1.txt")
        );

        std::fs::write(txt1_path, "test").unwrap();

        assert_eq!(
            resolve_path_conflict(txt_path, None),
            temp_path.join("test_2.txt")
        );

        assert_eq!(resolve_path_conflict(dir, None), temp_path.join("test_1"));
    }

    #[test]
    fn test_zip_files() {
        let tmp = tempfile::tempdir().unwrap();
        let temp = tempfile::TempDir::with_prefix("test_zip_files").unwrap();
        let dest_path = temp.path().to_path_buf();
        let sources = ["testdata/zip_test/test1.txt", "testdata/zip_test/test2"];

        for expected in ["test_dest.zip", "test_dest_1.zip", "test_dest_2.zip"] {
            assert_eq!(
                zip_files_in(
                    &sources,
                    &dest_path.join("test_dest.zip"),
                    false,
                    tmp.path()
                )
                .unwrap(),
                dest_path.join(expected)
            );
        }

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("unzipped").join("test1.txt"));
        expected.insert(dest_path.join("unzipped").join("test2"));

        assert_eq!(
            unzip_file_in(
                &dest_path.join("test_dest_2.zip"),
                UnzipOption::ToDir(dest_path.join("unzipped")),
                tmp.path(),
                None
            )
            .unwrap(),
            expected
        );

        let unzipped = dest_path.join("unzipped");
        assert!(unzipped.join("test2").join("test2").is_dir());
        assert_eq!(read(unzipped.join("test1.txt")), "test1");
        assert_eq!(
            read(unzipped.join("test2").join("test1.txt")),
            "test2_test1"
        );
        assert_eq!(
            read(unzipped.join("test2").join("test2").join("test1.txt")),
            "test2_test2_test1"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_zip_files_skips_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside.txt");
        std::fs::write(&outside, "secret").unwrap();
        let folder = temp.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("inside.txt"), "inside").unwrap();
        std::os::unix::fs::symlink(&outside, folder.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(temp.path(), folder.join("link_dir")).unwrap();

        let zip =
            zip_files_in(&[&folder], &temp.path().join("out.zip"), false, tmp.path()).unwrap();
        let unzipped = temp.path().join("unzipped");
        unzip_file_in(&zip, UnzipOption::ToDir(unzipped.clone()), tmp.path(), None).unwrap();

        assert_eq!(read(unzipped.join("folder").join("inside.txt")), "inside");
        assert!(!unzipped.join("folder").join("link.txt").exists());
        assert!(!unzipped.join("folder").join("link_dir").exists());

        // a symbolic link given directly is refused rather than followed
        assert!(zip_files_in(
            &[folder.join("link.txt")],
            &temp.path().join("link.zip"),
            false,
            tmp.path()
        )
        .is_err());
    }

    #[test]
    fn test_file_name_from_content_disposition() {
        let parse = file_name_from_content_disposition;
        assert_eq!(
            parse("attachment; filename=\"jre.tar.gz\"").as_deref(),
            Some("jre.tar.gz")
        );
        assert_eq!(
            parse("attachment; FileName=jre.zip").as_deref(),
            Some("jre.zip")
        );
        assert_eq!(
            parse("attachment; filename*=UTF-8''x.zip; filename=\"y.zip\"").as_deref(),
            Some("y.zip")
        );
        // path traversal and absolute paths keep only the last segment
        assert_eq!(
            parse("attachment; filename=\"../../x\"").as_deref(),
            Some("x")
        );
        assert_eq!(
            parse("attachment; filename=\"/etc/cron.d/job\"").as_deref(),
            Some("job")
        );
        assert_eq!(
            parse("attachment; filename=\"..\\..\\evil.bat\"").as_deref(),
            Some("evil.bat")
        );
        assert_eq!(
            parse("attachment; filename=\"C:evil.bat\"").as_deref(),
            Some("Cevil.bat")
        );
        assert_eq!(parse("attachment; filename=\"..\""), None);
        assert_eq!(parse("attachment; filename=\"a/..\""), None);
        assert_eq!(parse("attachment; filename=\"\""), None);
        assert_eq!(parse("attachment"), None);
        assert_eq!(parse("inline; name=x"), None);
    }

    #[tokio::test]
    async fn test_read_to_string_capped() {
        use crate::util::fs::read_to_string_capped;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file.txt");
        std::fs::write(&path, "0123456789").unwrap();

        assert_eq!(
            read_to_string_capped(&path, 10).await.unwrap(),
            "0123456789"
        );
        assert!(read_to_string_capped(&path, 9).await.is_err());
        assert!(read_to_string_capped(temp.path().join("missing"), 10)
            .await
            .is_err());
        #[cfg(unix)]
        assert!(read_to_string_capped("/dev/zero", 1024).await.is_err());
    }

    /// A program started without a console (as servers are) still reads its piped stdin,
    /// writes its piped stdout, and sees the end of stdin when the core closes it.
    /// `findstr` stands in for Java: it echoes every line it reads and exits at the end.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_program_without_a_console_talks_through_its_pipes() {
        use crate::util::dont_spawn_terminal;
        use std::process::Stdio;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut command = tokio::process::Command::new("findstr");
        command.arg("^");
        let mut child = dont_spawn_terminal(&mut command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"stop\r\n").await.unwrap();
        drop(stdin);
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .await
            .unwrap();
        let status = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
            .await
            .expect("findstr did not exit when its stdin closed")
            .unwrap();
        assert!(status.success(), "{status}");
        assert_eq!(stdout.trim(), "stop");
    }
}
