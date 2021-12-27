use std::{cmp::min, sync::atomic::AtomicU64};
use std::fs::File;
use std::io::Write;

use reqwest::Client;
use indicatif::{ProgressBar, ProgressStyle};
use futures_util::StreamExt;
use rocket::State;

use crate::MyManagedState;

// copied from https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
pub async fn download_file(url: &str, path: &str, state: &State<MyManagedState>, name: String) -> Result<(), String> {
    let client = Client::new();
    // Reqwest setup
    let res = client
        .get(url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))?;
    let total_size = res
        .content_length()
        .ok_or(format!("Failed to get content length from '{}'", &url))?;
    state.download_status.insert(name.clone(), (0, total_size));
    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));
    pb.set_message(&format!("Downloading {}", url));

    // download chunks
    let mut file = File::create(path).or(Err(format!("Failed to create file '{}'", path)))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("Error while downloading file")))?;
        file.write(&chunk)
            .or(Err(format!("Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        state.download_status.insert(name.clone(), (new, total_size));
        pb.set_position(new);
    }

    pb.finish_with_message(&format!("Downloaded {} to {}", url, path));
    return Ok(());
}