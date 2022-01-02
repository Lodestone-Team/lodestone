extern crate crypto;

use std::{cmp::min};
use std::fs::File;
use std::io::Write;

use crypto::digest::Digest;
use crypto::sha3::Sha3;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use mongodb::{bson::doc, options::ClientOptions, sync::Client as mongoDBClient};
use reqwest::Client;
use rocket::State;
use rocket::response::status::NoContent;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Authentication {
    username: String,
    hashed_password: String
}

use crate::MyManagedState;
// copied from https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
pub async fn download_file(url: &str, path: &str, state: &State<MyManagedState>, uuid: &str) -> Result<(), String> {
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
    state.download_status.insert(uuid.to_string(), (0, total_size));
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
        state.download_status.insert(uuid.to_string(), (new, total_size));
        pb.set_position(new);
    }

    pb.finish_with_message(&format!("Downloaded {} to {}", url, path));
    return Ok(());
}

pub fn mongodb_create_user(password: &String ) {
    let mut client_options = ClientOptions::parse("MongoDB Connection String").unwrap();
    client_options.app_name = Some("Initial Add User".to_string());
    let client = mongoDBClient::with_options(client_options).unwrap();

    client
        .database("admin")
        .run_command( doc! {
            "createUser": "server",
            "pwd": password,
            "roles": [
                { "role": "userAdminAnyDatabase", "db": "admin" },
                { "role": "readWriteAnyDatabase", "db": "admin" },
                { "role": "hostManager", "db": "admin" } 
            ]
        }, None).unwrap();
}

pub fn hash_password(password: &String) -> String{
    let mut hasher = Sha3::sha3_256();
    hasher.input_str(format!("{}pepega", password).as_str());
    hasher.result_str()
}

pub fn authenticate(state: &State<MyManagedState>, username: String, password: String) -> Result<bool, String>{
    let mongodb_client = &state.mongodb_client; 
    let password = hash_password(&password);

    let pairing_option = mongodb_client
        .database("users")
        .collection::<Authentication>("authentication")
        .find_one( doc! {
            "username": &username,
            "password": &password
        }, None)
        .unwrap();
    match pairing_option {
        Some(_) => return Ok(true),
        None => return Ok(false)
    }
}

pub fn create_user(state: &State<MyManagedState>, username: String, password: String) -> Result<(), String>{
    let mongodb_client = &state.mongodb_client; 
    let password = hash_password(&password);
    //check if username is duplicate
    let exists_option = mongodb_client
        .database("users")
        .collection::<Authentication>("authentication")
        .find_one( doc! {
            "username": &username
        }, None)
        .unwrap();
    match exists_option {
        Some(_) => return Err("User already exists".to_string()),
        None => {
            mongodb_client
                .database("users")
                .collection("authentication")
                .insert_one( doc! {
                    "username": &username,
                    "password": &password
                }, None)
                .unwrap();
            return Ok(());
        }
    }

}

//TODO: permission stuff with user

mod fs_helper {
    
}
