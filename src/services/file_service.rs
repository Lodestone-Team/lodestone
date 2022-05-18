use std::path::PathBuf;
use rocket::fs::{TempFile};

pub async fn save_temp_file(path: &PathBuf, mut data: TempFile<'_>) -> Result<(), String> {
    // FIXME this is not recommended in the document
    // the documentation prefer use adding temp_dir to config and use persis_to()
    // I am too lazy to figure out a dynamic temp_dir, so this will do for now
    match data.copy_to(path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            println!("{}", err);
            Err("Error saving file".to_string())
        },
    }
}
