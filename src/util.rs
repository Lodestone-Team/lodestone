use color_eyre::eyre::{eyre, Context, ContextCompat};
use std::collections::HashSet;
use std::ffi::OsStr;
use tokio::fs::File;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use flate2::read::GzDecoder;
use tar::Archive;

#[derive(Debug, Serialize, Deserialize)]
pub struct Authentication {
    username: String,
    password: String,
}

use crate::error::Error;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SetupProgress {
    pub current_step: (u8, String),
    pub total_steps: u8,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total: Option<u64>,
    pub downloaded: u64,
    pub step: u64,
    pub download_name: String,
}
pub async fn download_file(
    url: &str,
    path: &Path,
    name_override: Option<&str>,
    on_download: &(dyn Fn(DownloadProgress) + Send + Sync),
    overwrite_old: bool,
) -> Result<PathBuf, Error> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to send GET request")?;
    response.error_for_status_ref().context(
        "
        Failed to download file
    ",
    )?;
    tokio::fs::create_dir_all(path)
        .await
        .context(format!("Failed to create dir {}", &path.display()))?;

    let file_name;
    if let Some(name) = name_override {
        file_name = name.to_string();
    } else {
        file_name = response
            .headers()
            .get("Content-Disposition")
            .map_or_else(
                || "unknown".to_string(),
                |h| {
                    h.to_str()
                        .map_or_else(|_| "unknown".to_string(), |s| s.to_string())
                },
            )
            // parse filename's value from the header, remove the ""
            .split(';')
            .nth(1)
            .unwrap_or("unknown")
            .split('=')
            .nth(1)
            .unwrap_or("unknown")
            .replace('\"', "");
    }
    if !overwrite_old && path.join(&file_name).exists() {
        return Err(eyre!("File {} already exists", path.join(&file_name).display()).into());
    }
    fs::remove_file(path.join(&file_name)).await.ok();
    let total_size = response.content_length();
    let pb = ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));
    pb.set_message(&format!("Downloading {}", url));

    let mut downloaded_file = File::create(path.join(&file_name))
        .await
        .context(format!("Failed to create file {}", &path.display()))?;
    let mut downloaded: u64 = 0;
    let mut new_downloaded: u64 = 0;
    let threshold = total_size.unwrap_or(500000) / 100;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.expect("Error while downloading file");
        downloaded_file
            .write_all(&chunk)
            .await
            .context(format!("Failed to write to file {}", &file_name))?;
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

        pb.set_position(new_downloaded);
    }
    Ok(path.join(&file_name))
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

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
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

pub async fn unzip_file(
    file: impl AsRef<Path>,
    unzip_option: UnzipOption,
) -> Result<HashSet<PathBuf>, Error> {
    let file = file.as_ref();

    if !file.exists() {
        return Err(eyre!("File {} does not exist", file.display()).into());
    }

    let file_extension = file
        .extension()
        .ok_or_else(|| eyre!("Failed to get file extension for {}", file.display()))?;
    if file_extension != "gz"
        && file_extension != "tgz"
        && file_extension != "zip"
        && file_extension != "rar"
    {
        return Err(eyre!("Unsupported extension for {}", file.display()).into());
    }

    let parent = file.parent().context(format!(
        "Failed to get parent directory of {}",
        file.display()
    ))?;

    let file_stem = file
        .file_stem()
        .context(format!("Failed to get file stem of {}", file.display()))?;

    let mut dest = match unzip_option {
        UnzipOption::Normal => parent.to_path_buf(),
        // resolve the dest after we unzip the file to a temp dir
        UnzipOption::Smart => Default::default(),
        UnzipOption::ToDirectoryWithFileName => resolve_path_conflict(parent.join(file_stem), None),
        UnzipOption::ToDir(ref d) => resolve_path_conflict(d.to_owned(), None),
    };

    let temp_dest_dir =
        tempdir::TempDir::new("unzip_temp").context("Failed to create temporary directory for")?;
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
        archive
            .extract(temp_dest)
            .context(format!("Failed to decompress file {}", file.display()))?;
    } else if file_extension == "rar" {
        let archive = unrar::Archive::new(
            file.to_str()
                .ok_or_else(|| eyre!("Non-unicode character in file name {}", file.display()))?
                .to_string(),
        );
        archive
            .extract_to(
                temp_dest
                    .to_str()
                    .ok_or_else(|| {
                        eyre!("Non-unicode character in file name {}", temp_dest.display())
                    })?
                    .to_string(),
            )
            .map_err(|_| eyre!("Failed to decompress file {}", file.display()))?
            .process()
            .map_err(|_| eyre!("Failed to decompress file {}", file.display()))?;
    }

    let mut ret: HashSet<PathBuf> = HashSet::new();

    let temp_dir_content = list_dir(temp_dest, None).await?;

    if let UnzipOption::Smart = unzip_option {
        dest = if temp_dir_content.len() > 1 {
            resolve_path_conflict(parent.join(file_stem), None)
        } else {
            parent.to_path_buf()
        }
    };

    let dest = resolve_path_conflict(dest, None);

    tokio::fs::create_dir_all(&dest)
        .await
        .context(format!("Failed to create directory {}", dest.display()))?;

    // Only loop through direct children
    for temp_entry_path in temp_dir_content.iter() {
        let entry_path = match temp_entry_path.strip_prefix(temp_dest) {
            Ok(p) => dest.join(p),
            Err(_) => continue,
        };

        let entry_path = resolve_path_conflict(entry_path, None);

        if temp_entry_path.is_dir() {
            // Direct child is a directory
            tokio::fs::create_dir_all(&entry_path)
                .await
                .context(format!(
                    "Failed to create directory {}",
                    entry_path.display()
                ))?;

            // Copy all files from direct child directory. Guarentee no duplicate
            for temp_child in walkdir::WalkDir::new(temp_entry_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let temp_child_path = temp_child.path();
                let child_path = match temp_child_path.strip_prefix(temp_entry_path) {
                    Ok(p) => entry_path.join(p),
                    Err(_) => continue,
                };

                if temp_child_path.is_dir() {
                    tokio::fs::create_dir_all(&child_path)
                        .await
                        .context(format!(
                            "Failed to create directory {}",
                            child_path.display()
                        ))?;
                }

                if temp_child_path.is_file() {
                    tokio::fs::copy(&temp_child_path, &child_path)
                        .await
                        .context(format!(
                            "Failed to copy from {} to {}",
                            temp_child_path.display(),
                            child_path.display()
                        ))?;
                }
            }
        } else {
            // Direct child is a file

            // Copy direct child file
            tokio::fs::copy(&temp_entry_path, &entry_path)
                .await
                .context(format!(
                    "Failed to copy from {} to {}",
                    temp_entry_path.display(),
                    entry_path.display()
                ))?;
        }
        ret.insert(entry_path);
    }

    Ok(ret)
}

pub fn rand_alphanumeric(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).collect()
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

    use color_eyre::eyre::Context;
    use tokio::fs::File;

    use crate::error::Error;

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

    pub async fn create(file: impl AsRef<Path>) -> Result<File, Error> {
        let file = file.as_ref();
        let file = tokio::fs::File::create(file)
            .await
            .context(format!("Failed to create file at {}", file.display()))?;
        Ok(file)
    }
}
pub fn dont_spawn_terminal(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd
}

pub fn format_byte_download(bytes: u64, total: u64) -> String {
    let mut bytes = bytes as f64;
    let mut total = total as f64;
    let mut unit = "B";
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "KB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "MB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "GB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "TB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "PB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "EB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "ZB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "YB";
    }
    format!("{:.1} / {:.1} {}", bytes, total, unit)
}

pub fn format_byte(bytes: u64) -> String {
    let mut bytes = bytes as f64;
    let mut unit = "B";
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "KB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "MB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "GB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "TB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "PB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "EB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "ZB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "YB";
    }
    format!("{:.1} {}", bytes, unit)
}

#[cfg(test)]
mod tests {
    use crate::util::{resolve_path_conflict, unzip_file, UnzipOption};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use tokio;

    #[tokio::test]
    async fn test_unzip_file() {
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let temp_path = temp.path();
        let zip = PathBuf::from("testdata/sample.zip");

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg.txt"));
        test.insert(temp_path.join("amendments.txt"));
        test.insert(temp_path.join("constitution.txt"));

        assert_eq!(
            unzip_file(&zip, UnzipOption::ToDirectoryWithFileName)
                .await
                .unwrap(),
            test
        );

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg_1.txt"));
        test.insert(temp_path.join("amendments_1.txt"));
        test.insert(temp_path.join("constitution_1.txt"));

        assert_eq!(
            unzip_file(&zip, UnzipOption::ToDir(temp_path.to_owned()))
                .await
                .unwrap(),
            test
        );
    }

    #[tokio::test]
    async fn test_unzip_file_2() {
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let temp_path = temp.path();
        let rar = PathBuf::from("testdata/sample-1.rar");

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("hi").join("sample-1_1.webp"));

        assert_eq!(
            unzip_file(&rar, UnzipOption::ToDir(temp_path.join("hi")))
                .await
                .unwrap(),
            test
        );

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("hi").join("sample-1_1_1.webp"));

        assert_eq!(
            unzip_file(&rar, UnzipOption::ToDir(temp_path.join("hi")))
                .await
                .unwrap(),
            test
        );
    }

    #[tokio::test]
    async fn test_unzip_file_3() {
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let dest_path = temp.path().to_path_buf();
        let tar_gz = PathBuf::from("testdata/sample.gz");

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample"));

        assert_eq!(
            unzip_file(&tar_gz, UnzipOption::ToDir(dest_path.clone()))
                .await
                .unwrap(),
            expected
        );
        assert!(dest_path.join("sample").join("sample.exe").is_file());
        assert!(dest_path.join("sample").join("sample.c").is_file());
        assert!(dest_path.join("sample").join("sample.obj").is_file());

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample_1"));

        assert_eq!(
            unzip_file(&tar_gz, UnzipOption::ToDir(dest_path.to_owned()))
                .await
                .unwrap(),
            expected
        );
        assert!(dest_path.join("sample_1").join("sample.exe").is_file());
        assert!(dest_path.join("sample_1").join("sample.c").is_file());
        assert!(dest_path.join("sample_1").join("sample.obj").is_file(),);
    }

    #[test]
    fn test_resolve_path_conflict() {
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
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
}
