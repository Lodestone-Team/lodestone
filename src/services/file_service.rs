use rocket::fs::TempFile;
use std::{
    fs::{create_dir_all, read_dir, File},
    io::{Error, Read, Seek, Write},
    path::PathBuf,
};
use zip::{ZipWriter, write::FileOptions};

pub async fn save_temp_file(path: &PathBuf, mut data: TempFile<'_>) -> Result<(), String> {
    // FIXME this is not recommended in the document
    // the documentation prefer use adding temp_dir to config and use persis_to()
    // I am too lazy to figure out a dynamic temp_dir, so this will do for now
    match data.copy_to(path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!("{}", path.to_str().unwrap());
            warn!("{}", err);
            Err("Error saving file".to_string())
        }
    }
}

/// This will overwrite the destination if file exists
pub fn zipping(src: PathBuf, dst_folder: PathBuf, file_name: &str) -> Result<File, Error> {
    if !dst_folder.is_dir() {
        create_dir_all(dst_folder.clone());
    }
    let mut file_path = dst_folder.clone();
    file_path.set_file_name(file_name);
    file_path.set_extension("zip");
    let out = File::create(file_path.clone())?;

    let mut zip = ZipWriter::new(out);

    build_zip(src, PathBuf::new(), &mut zip)?;

    zip.finish().unwrap();
    Ok(File::open(file_path)?)
}

fn build_zip(
    cur_src_dir: PathBuf,
    cur_zip_dir: PathBuf,
    zip: &mut ZipWriter<File>,
) -> Result<(), Error> {
    let paths = read_dir(cur_src_dir.as_path())?;
    for path_result in paths {
        match path_result {
            Ok(path) => match path.file_type() {
                Ok(file_type) => {
                    if file_type.is_file() {
                        let mut file = File::open(path.path()).unwrap();
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer);
                        zip.start_file(
                            cur_zip_dir.join(path.file_name().to_str().unwrap()).to_str().unwrap(),
                            FileOptions::default(),
                        );
                        zip.write_all(&mut buffer);
                    } else if file_type.is_dir() {
                        let file_name = path.file_name(); // Rust stuff...
                        let dir_name = file_name.to_str().unwrap();
                        let new_zip_dir = cur_zip_dir.join(dir_name);
                        zip.add_directory(new_zip_dir.to_str().unwrap(), FileOptions::default());
                        build_zip(cur_src_dir.join(dir_name), new_zip_dir, zip);
                    }
                }
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
    Ok(())
}
