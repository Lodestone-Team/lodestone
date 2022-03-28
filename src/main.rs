#![allow(unused_must_use)]

#[macro_use]
extern crate rocket;
extern crate sanitize_filename;

use chashmap::CHashMap;
use futures_util::lock::Mutex;
use regex::Regex;
use std::env;
use std::fs::create_dir_all;
use std::io::{stdin, BufRead, BufReader};
use std::sync::Arc;
mod handlers;
mod managers;
mod util;
use handlers::*;
use instance_manager::InstanceManager;
use managers::*;
use mongodb::{options::ClientOptions, sync::Client};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::FileServer;
use rocket::http::{Header, Status};
use rocket::{routes, Request, Response};
use std::path::PathBuf;
use std::{thread, time};

pub struct MyManagedState {
    instance_manager: Arc<Mutex<InstanceManager>>,
    download_status: CHashMap<String, (u64, u64)>,
    mongodb_client: Client,
}

pub struct CORS;

#[rocket::async_trait]
impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        res.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, GET, PATCH, OPTIONS, DELETE",
        ));
        res.set_header(Header::new(
            "Access-Control-Allow-Headers",
            "Origin, Content-Type, X-Auth-Token",
        ));
        res.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}

#[options("/<path..>")]
fn options_handler<'a>(path: PathBuf) -> Status {
    Status::Ok
}

#[rocket::main]
async fn main() {
    let mut client_options = ClientOptions::parse("mongodb://localhost:27017/?tls=false").unwrap();
    client_options.app_name = Some("MongoDB Client".to_string());
    let client = Client::with_options(client_options).unwrap();

    let mut lodestone_path = match env::var("LODESTONE_PATH") {
        Ok(val) => PathBuf::from(val),
        Err(_) => env::current_dir().unwrap(),
    };
    // lodestone_path = PathBuf::from("/home/peter/Lodestone/backend/lodestone/");
    env::set_current_dir(&lodestone_path).unwrap();

    let static_path = lodestone_path.join("web");

    //create the web direcotry if it doesn't exist
    create_dir_all(&static_path).unwrap();

    //print file locations to console
    println!("Lodestone directory: {}", lodestone_path.display());

    let instance_manager = Arc::new(Mutex::new(
        InstanceManager::new(lodestone_path, client.clone()).unwrap(),
    ));
    let instance_manager_closure = instance_manager.clone();
    rocket::tokio::spawn(async move {
        let reader = BufReader::new(stdin());
        for line_result in reader.lines() {
            let mut instance_manager = instance_manager_closure.lock().await;
            let line = line_result.unwrap_or("failed to read stdin command".to_string());
            let line_vec: Vec<&str> = line.split_whitespace().collect();
            let regex = Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+start").unwrap();
            if regex.is_match(&line) {
                instance_manager
                    .start_instance(line_vec[1].to_string())
                    .map_err(|err| eprintln!("{}", err));
            }
            let regex = Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+stop").unwrap();
            if regex.is_match(&line) {
                instance_manager
                    .stop_instance(line_vec[1].to_string())
                    .map_err(|err| eprintln!("{}", err));
            }
            let regex =
                Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+send[[:space:]]+(\w+)").unwrap();
            if regex.is_match(&line) {
                instance_manager
                    .send_command(line_vec[1].to_string(), line_vec[3].to_string())
                    .map_err(|err| eprintln!("{}", err));
            }
            let regex = Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+playercount").unwrap();
            if regex.is_match(&line) {
                match instance_manager.player_num(line_vec[1].to_string()) {
                    Ok(size) => println!("{}", size.to_string()),
                    Err(reason) => eprintln!("{}", reason),
                }
            }
            let regex = Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+playerlist").unwrap();
            if regex.is_match(&line) {
                match instance_manager.player_list(line_vec[1].to_string()) {
                    Ok(list) => println!("{:?}", list),
                    Err(reason) => eprintln!("{}", reason),
                }
            }
            let regex = Regex::new(r"instance[[:space:]]+(\w+)[[:space:]]+log[[:space:]]+(\d+)[[:space:]]+(\d+)").unwrap();
            // TODO implement mongodb get logs
            // match regex.capture(&line) {
            //     Some(cap) => match instance_manager.player_list(line_vec[1].to_string()) {
            //         Ok(list) => println!("{:?}", list),
            //         Err(reason) => eprintln!("{}", reason),
            //     }
            //     None() => ()
            // }
            // TODO turn string into enum

            
        }
    });
    rocket::build()
        .mount(
            "/api/v1/",
            routes![
                users::create,
                users::test,
                instance::start,
                instance::stop,
                instance::send,
                instance::setup,
                instance::delete,
                instance::download_status,
                instance::status,
                instance::get_list,
                instance::get_logs,
                instance::player_count,
                instance::player_list,
                instance::list_resource,
                instance::load_resource,
                instance::unload_resource,
                jar::vanilla_versions,
                jar::vanilla_jar,
                jar::vanilla_filters,
                jar::fabric_versions,
                jar::fabric_jar,
                jar::fabric_filters,
                jar::flavours,
                system::get_ram,
                system::get_disk,
                system::get_cpu_speed,
                system::get_cpu_info,
                system::get_os_info,
                system::get_utilization,
                system::get_uptime
            ],
        )
        .mount("/", FileServer::from(static_path))
        .mount("/", routes![options_handler])
        .manage(MyManagedState {
            instance_manager,
            download_status: CHashMap::new(),
            mongodb_client: client,
        })
        .attach(CORS)
        .launch()
        .await;
    println!("shutting down");
}
