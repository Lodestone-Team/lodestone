use color_eyre::eyre::{eyre, Context, ContextCompat};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{Read, Write};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use futures_util::StreamExt;
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
use crate::prelude::path_to_tmp;
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
    let lodestone_tmp = path_to_tmp().clone();
    tokio::fs::create_dir_all(&lodestone_tmp)
        .await
        .context("Failed to create tmp dir")?;
    let temp_file_path = tempfile::NamedTempFile::new_in(lodestone_tmp)
        .context("Failed to create temporary file")?
        .path()
        .to_owned();
    let mut temp_file = tokio::fs::File::create(&temp_file_path)
        .await
        .context("Failed to create temporary file")?;
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
    }
    tokio::fs::rename(temp_file_path, path.join(&file_name))
        .await
        .context(format!("Failed to rename file {}", &file_name))?;
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

pub fn unzip_file(
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

    let mut dest = match unzip_option {
        UnzipOption::Normal => parent.to_path_buf(),
        // resolve the dest after we unzip the file to a temp dir
        UnzipOption::Smart => Default::default(),
        UnzipOption::ToDirectoryWithFileName => resolve_path_conflict(parent.join(file_stem), None),
        UnzipOption::ToDir(ref d) => d.to_owned(),
    };
    let lodestone_tmp = path_to_tmp().clone();
    std::fs::create_dir_all(&lodestone_tmp).context(format!(
        "Failed to create temporary directory {}",
        lodestone_tmp.display()
    ))?;

    let temp_dest_dir = tempfile::tempdir_in(lodestone_tmp).context(
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
        archive
            .extract(temp_dest)
            .context(format!("Failed to decompress file {}", file.display()))?;
    }

    let mut ret: HashSet<PathBuf> = HashSet::new();

    let temp_dir_content = std::fs::read_dir(temp_dest)
        .context(format!("Failed to read directory {}", temp_dest.display()))?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .collect::<Vec<_>>();

    if let UnzipOption::Smart = unzip_option {
        dest = if temp_dir_content.len() > 1 {
            resolve_path_conflict(parent.join(file_stem), None)
        } else {
            parent.to_path_buf()
        }
    };

    // let dest = resolve_path_conflict(dest, None);

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
    let _file = file.as_ref().to_owned();
    tokio::task::spawn_blocking(move || unzip_file(_file, unzip_option))
        .await
        .context(format!(
            "Failed to unzip file {} in a blocking task",
            file.as_ref().display()
        ))?
}

pub fn zip_files(files: &[impl AsRef<Path>], dest: impl AsRef<Path>) -> Result<PathBuf, Error> {
    let dest = dest.as_ref();
    std::fs::create_dir_all(dest.parent().context("Failed to get destination parent")?)
        .context(format!("Failed to create directory {}", dest.display()))?;
    let lodestone_tmp = path_to_tmp().clone();
    std::fs::create_dir_all(&lodestone_tmp).context(format!(
        "Failed to create temporary directory {}",
        lodestone_tmp.display()
    ))?;
    let tmp_archive = tempfile::NamedTempFile::new_in(lodestone_tmp)
        .context("Failed to create temporary file for zipping")?;

    let mut buffer = Vec::new();
    let mut writer = zip::ZipWriter::new(&tmp_archive);
    let options = zip::write::FileOptions::default().unix_permissions(0o775);
    for entry_path in files.iter().map(|f| f.as_ref()) {
        if entry_path.is_dir() {
            writer
                .add_directory(
                    entry_path
                        .file_name()
                        .ok_or_else(|| eyre!("Entry has abnormal name"))?
                        .to_str()
                        .ok_or_else(|| eyre!("Entry has abnormal name"))?,
                    options,
                )
                .context(format!(
                    "Failed to create {} in archive",
                    entry_path.display()
                ))?;

            for child_entry in walkdir::WalkDir::new(entry_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let child_entry_path = child_entry.path();
                let child_entry_dest =
                    child_entry_path
                        .strip_prefix(entry_path.parent().context(format!(
                            "Failed to get parent for {}",
                            entry_path.display()
                        ))?)
                        .context(format!(
                            "Failed to strip prefix for {}",
                            child_entry_path.display()
                        ))?;

                if child_entry_path.is_dir() {
                    writer
                        .add_directory(child_entry_dest.to_string_lossy(), options)
                        .context(format!(
                            "Failed to create {} in archive",
                            child_entry_path.display()
                        ))?;
                }

                if child_entry_path.is_file() {
                    let child_entry_name = child_entry_dest.to_string_lossy();

                    writer
                        .start_file(child_entry_name, options)
                        .context(format!(
                            "Failed to create {} in archive",
                            child_entry_path.display()
                        ))?;

                    let mut child_entry_file = std::fs::File::open(child_entry_path)
                        .context(format!("Failed to open {}", child_entry_path.display()))?;
                    child_entry_file
                        .read_to_end(&mut buffer)
                        .context(format!("Failed to read {}", child_entry_path.display()))?;
                    writer.write_all(&buffer).context(format!(
                        "Failed to write {} to archive",
                        child_entry_path.display()
                    ))?;
                    buffer.clear();
                }
            }
        }

        if entry_path.is_file() {
            let entry_name = entry_path
                .file_name()
                .ok_or_else(|| eyre!("File to zip has no name"))?
                .to_str()
                .ok_or_else(|| eyre!("File to zip has abnormal name"))?;

            writer.start_file(entry_name, options).context(format!(
                "Failed to create {} in archive",
                entry_path.display()
            ))?;

            let mut entry_file = std::fs::File::open(entry_path)
                .context(format!("Failed to open {}", entry_path.display()))?;
            entry_file
                .read_to_end(&mut buffer)
                .context(format!("Failed to read {}", entry_path.display()))?;
            writer.write_all(&buffer).context(format!(
                "Failed to write {} to archive",
                entry_path.display()
            ))?;
            buffer.clear();
        }
    }

    writer.finish().context("Zip failed")?;
    let dest = resolve_path_conflict(dest.into(), None);
    std::fs::rename(tmp_archive.path(), &dest).context(format!(
        "Failed to move {} to {}",
        tmp_archive.path().display(),
        dest.display()
    ))?;
    Ok(dest)
}

pub async fn zip_files_async(
    files: &[impl AsRef<Path>],
    dest: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
    let _files = files
        .iter()
        .map(|f| f.as_ref().to_owned())
        .collect::<Vec<_>>();
    let _dest = dest.as_ref().to_owned();
    tokio::task::spawn_blocking(move || zip_files(&_files, &_dest))
        .await
        .context("Failed to spawn blocking task")?
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

pub fn format_byte_download(mut bytes: u64, mut total: u64) -> String {
    let mut unit = "B";
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "KB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "MB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "GB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "TB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "PB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "EB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "ZB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        total /= 1024;
        unit = "YB";
    }
    format!("{:.1} / {:.1} {}", bytes, total, unit)
}

pub fn format_byte(mut bytes: u64) -> String {
    let mut unit = "B";
    if bytes > 1024 {
        bytes /= 1024;
        unit = "KB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "MB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "GB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "TB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "PB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "EB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "ZB";
    }
    if bytes > 1024 {
        bytes /= 1024;
        unit = "YB";
    }
    format!("{:.1} {}", bytes, unit)
}

#[cfg(test)]
mod tests {
    use crate::prelude::init_paths;
    use crate::util::{resolve_path_conflict, unzip_file, zip_files, UnzipOption};
    use std::collections::HashSet;
    use std::io::Read;
    use std::path::PathBuf;
    use tokio;

    #[tokio::test]
    async fn test_unzip_file() {
        let temp_lodestone_path = tempfile::tempdir().unwrap();
        let temp_lodestone_path = temp_lodestone_path.path();
        init_paths(temp_lodestone_path.to_path_buf());
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let temp_path = temp.path();
        let zip = PathBuf::from("testdata/sample.zip");

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg.txt"));
        test.insert(temp_path.join("amendments.txt"));
        test.insert(temp_path.join("constitution.txt"));

        assert_eq!(
            unzip_file(&zip, UnzipOption::ToDir(temp_path.to_owned())).unwrap(),
            test
        );

        let mut test: HashSet<PathBuf> = HashSet::new();
        test.insert(temp_path.join("gettysburg_1.txt"));
        test.insert(temp_path.join("amendments_1.txt"));
        test.insert(temp_path.join("constitution_1.txt"));

        assert_eq!(
            unzip_file(&zip, UnzipOption::ToDir(temp_path.to_owned())).unwrap(),
            test
        );
    }

    #[tokio::test]
    async fn test_unzip_file_3() {
        let temp_lodestone_path = tempfile::tempdir().unwrap();
        let temp_lodestone_path = temp_lodestone_path.path();
        init_paths(temp_lodestone_path.to_path_buf());
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let dest_path = temp.path().to_path_buf();
        let tar_gz = PathBuf::from("testdata/sample.gz");

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample"));

        assert_eq!(
            unzip_file(&tar_gz, UnzipOption::ToDir(dest_path.clone())).unwrap(),
            expected
        );
        assert!(dest_path.join("sample").join("sample.exe").is_file());
        assert!(dest_path.join("sample").join("sample.c").is_file());
        assert!(dest_path.join("sample").join("sample.obj").is_file());

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("sample_1"));

        assert_eq!(
            unzip_file(&tar_gz, UnzipOption::ToDir(dest_path.to_owned())).unwrap(),
            expected
        );
        assert!(dest_path.join("sample_1").join("sample.exe").is_file());
        assert!(dest_path.join("sample_1").join("sample.c").is_file());
        assert!(dest_path.join("sample_1").join("sample.obj").is_file(),);
    }

    #[test]
    fn test_resolve_path_conflict() {
        let temp_lodestone_path = tempfile::tempdir().unwrap();
        let temp_lodestone_path = temp_lodestone_path.path();
        init_paths(temp_lodestone_path.to_path_buf());
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

    #[tokio::test]
    async fn test_zip_files() {
        let temp = tempdir::TempDir::new("test_unzip_file").unwrap();
        let dest_path = temp.path().to_path_buf();

        assert_eq!(
            zip_files(
                &["testdata/zip_test/test1.txt", "testdata/zip_test/test2"],
                dest_path.join("test_dest.zip")
            )
            .unwrap(),
            dest_path.join("test_dest.zip")
        );
        assert_eq!(
            zip_files(
                &["testdata/zip_test/test1.txt", "testdata/zip_test/test2"],
                dest_path.join("test_dest.zip")
            )
            .unwrap(),
            dest_path.join("test_dest_1.zip")
        );
        assert_eq!(
            zip_files(
                &["testdata/zip_test/test1.txt", "testdata/zip_test/test2"],
                dest_path.join("test_dest.zip")
            )
            .unwrap(),
            dest_path.join("test_dest_2.zip")
        );

        let mut expected: HashSet<PathBuf> = HashSet::new();
        expected.insert(dest_path.join("unzipped").join("test1.txt"));
        expected.insert(dest_path.join("unzipped").join("test2"));

        assert_eq!(
            unzip_file(
                &dest_path.join("test_dest_2.zip"),
                UnzipOption::ToDir(dest_path.join("unzipped"))
            )
            .unwrap(),
            expected
        );

        assert!(dest_path.join("unzipped").join("test1.txt").is_file());
        assert!(dest_path.join("unzipped").join("test2").is_dir());
        assert!(dest_path
            .join("unzipped")
            .join("test2")
            .join("test1.txt")
            .is_file());
        assert!(dest_path
            .join("unzipped")
            .join("test2")
            .join("test2")
            .is_dir());
        assert!(dest_path
            .join("unzipped")
            .join("test2")
            .join("test2")
            .join("test1.txt")
            .is_file());

        let file = std::fs::File::open(dest_path.join("unzipped").join("test1.txt")).unwrap();
        let mut buf_reader = std::io::BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).unwrap();
        assert_eq!(contents.trim(), "test1");

        let file = std::fs::File::open(dest_path.join("unzipped").join("test2").join("test1.txt"))
            .unwrap();
        let mut buf_reader = std::io::BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).unwrap();
        assert_eq!(contents.trim(), "test2_test1");

        let file = std::fs::File::open(
            dest_path
                .join("unzipped")
                .join("test2")
                .join("test2")
                .join("test1.txt"),
        )
        .unwrap();
        let mut buf_reader = std::io::BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).unwrap();
        assert_eq!(contents.trim(), "test2_test2_test1");
    }
}
