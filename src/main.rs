#![allow(unused_must_use)]

#[macro_use]
extern crate rocket;
extern crate sanitize_filename;

use chashmap::CHashMap;
use crossbeam_channel::unbounded;
use futures_util::lock::Mutex;
use managers::types::{ResourceType};
use regex::Regex;
use std::env;
use std::fs::create_dir_all;
use std::io::{stdin, BufRead, BufReader};
use std::sync::{Arc};
use websocket::sync::Server;
use websocket::OwnedMessage;
mod event_processor;
mod handlers;
mod managers;
mod util;
mod services;
mod traits;
use handlers::*;
use instance_manager::InstanceManager;
use managers::*;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::FileServer;
use rocket::http::{Header, Status};
use rocket::{routes, Request, Response};
use std::path::PathBuf;
use std::{thread};
use sys_info::{cpu_speed, disk_info, mem_info, os_release, os_type};
use systemstat::{Duration, Platform, System};
use log::{info, LevelFilter};

static mut WS_SENDER: Option<crossbeam_channel::Sender<String>> = None;

pub struct MyManagedState {
    instance_manager: Arc<Mutex<InstanceManager>>,
    download_status: CHashMap<String, (u64, u64)>,
    ws_sender: std::sync::Mutex<crossbeam_channel::Sender<String>>,
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

#[catch(500)]
fn internal_server_error() -> &'static str {
    "Unknown Internal Error"
}

#[rocket::main]
async fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).format_module_path(false).format_timestamp(None).format_target(false).init();
    let lodestone_path = match env::var("LODESTONE_PATH") {
        Ok(val) => PathBuf::from(val),
        Err(_) => env::current_dir().unwrap(),
    };
    env::set_current_dir(&lodestone_path).unwrap();

    let static_path = lodestone_path.join("web");

    //create the web direcotry if it doesn't exist
    create_dir_all(&static_path).unwrap();

    //print file locations to console
    info!("Lodestone directory: {}", lodestone_path.display());

    let instance_manager = Arc::new(Mutex::new(InstanceManager::new(lodestone_path).unwrap()));
    let instance_manager_closure = instance_manager.clone();
    rocket::tokio::spawn(async move {
        let reader = BufReader::new(stdin());
        for line_result in reader.lines() {
            let mut instance_manager = instance_manager_closure.lock().await;
            let line = line_result.unwrap_or("failed to read stdin command".to_string());
            let line_vec: Vec<&str> = line.split_whitespace().collect();
            if line_vec.len() < 2 {
                eprint!("Invalid command: {}\n", line);
                continue;
            }
            let identifier = instance_manager
                .name_to_uuid(&line_vec[1].to_string())
                .unwrap_or(line_vec[1].to_string());
            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+send[[:space:]]+(.+)")
                .unwrap();
            if regex.is_match(&line) {
                println!("sending command");
                instance_manager
                    .send_command(
                        &identifier,
                        regex
                            .captures(&line)
                            .unwrap()
                            .get(2)
                            .unwrap()
                            .as_str()
                            .to_string(),
                    )
                    .map_err(|err| println!("{}", err));
            }
            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+start").unwrap();
            if regex.is_match(&line) {
                instance_manager
                    .start_instance(&identifier)
                    .map_err(|err| println!("{}", err));
            }
            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+stop").unwrap();
            if regex.is_match(&line) {
                instance_manager
                    .stop_instance(&identifier)
                    .map_err(|err| println!("{}", err));
            }

            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+playercount").unwrap();
            if regex.is_match(&line) {
                match instance_manager.player_num(&identifier) {
                    Ok(size) => println!("{}", size.to_string()),
                    Err(reason) => println!("{}", reason),
                }
            }
            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+playerlist").unwrap();
            if regex.is_match(&line) {
                match instance_manager.player_list(&identifier) {
                    Ok(list) => println!("{:?}", list),
                    Err(reason) => println!("{}", reason),
                }
            }
            let regex = Regex::new(
                r"instance[[:space:]]+([\w-]+)[[:space:]]+log[[:space:]]+(\d+)[[:space:]]+(\d+)",
            )
            .unwrap();
            // TODO implement mongodb get logs
            // match regex.capture(&line) {
            //     Some(cap) => match instance_manager.player_list(line_vec[1].to_string()) {
            //         Ok(list) => println!("{:?}", list),
            //         Err(reason) => println!("{}", reason),
            //     }
            //     None() => ()
            // }
            // TODO turn string into enum
            let regex = Regex::new(r"instance[[:space:]]+([\w-]+)[[:space:]]+resources[[:space:]]+((?:Mod)|(?:World))[[:space:]]+list").unwrap();
            match regex.captures(&line) {
                Some(cap) => {
                    match instance_manager.list_resource(
                        &identifier,
                        if cap.get(2).unwrap().as_str().eq("Mod") {
                            ResourceType::Mod
                        } else {
                            ResourceType::World
                        },
                    ) {
                        Ok(list) => {
                            println!("loaded: {:?}", list.0);
                            println!("unloaded: {:?}", list.1);
                        }
                        Err(reason) => println!("{}", reason),
                    }
                }
                _ => (), // Not a match, do nothing
            }

            if Regex::new(r"sys[[:space:]]+mem").unwrap().is_match(&line) {
                match mem_info() {
                    Ok(mem) => println!("{}/{}", mem.free, mem.total),
                    Err(_) => println!("failed to get ram"),
                }
            }
            if Regex::new(r"sys[[:space:]]+disk").unwrap().is_match(&line) {
                match disk_info() {
                    Ok(disk) => println!("{}/{}", disk.free, disk.total),
                    Err(_) => println!("failed to get disk"),
                }
            }
            if Regex::new(r"sys[[:space:]]+cpuspeed")
                .unwrap()
                .is_match(&line)
            {
                match cpu_speed() {
                    Ok(cpuspeed) => println!("{}", cpuspeed.to_string()),
                    Err(_) => println!("failed to get cpu speed"),
                }
            }
            if Regex::new(r"sys[[:space:]]+cpuutil")
                .unwrap()
                .is_match(&line)
            {
                let sys = System::new();
                match sys.cpu_load_aggregate() {
                    Ok(load) => {
                        thread::sleep(Duration::from_secs(1));
                        println!("{}", load.done().unwrap().user.to_string())
                    }
                    Err(_) => println!("failed to get cpu info"),
                }
            }
            if Regex::new(r"sys[[:space:]]+cpuutil")
                .unwrap()
                .is_match(&line)
            {
                match os_release() {
                    Ok(release) => match os_type() {
                        Ok(ostype) => println!("{} {}", ostype, release),
                        Err(_) => println!("failed to get os info"),
                    },
                    Err(_) => println!("failed to get os info"),
                }
            }
            // TODO #[get("/sys/osinfo")]
            if Regex::new(r"sys[[:space:]]+uptime")
                .unwrap()
                .is_match(&line)
            {
                let sys = System::new();
                match sys.uptime() {
                    Ok(uptime) => println!("{}", uptime.as_secs_f64().to_string()),
                    Err(_) => println!("failed to get cpu info"),
                }
            }
        }
    });
    let (tx, rx) = unbounded();

    unsafe {
        WS_SENDER = Some(tx.clone());
    }

    let server = Server::bind("0.0.0.0:8006").unwrap();
    thread::spawn(move || {
        for request in server.filter_map(Result::ok) {
            let rx = rx.clone();
            thread::spawn(move || {
                if !request.protocols().contains(&"rust-websocket".to_string()) {
                    println!("refused");
                    request.reject().unwrap();
                    return;
                }

                let client = request.use_protocol("rust-websocket").accept().unwrap();
                client.set_nonblocking(true).unwrap();
                let ip = client.peer_addr().unwrap();

                println!("Connection from {}", ip);

                let (mut receiver, mut sender) = client.split().unwrap();
                // shutdown the connection if the client closes
                loop {
                    if let Ok(s) = rx.try_recv() {
                        sender.send_message(&OwnedMessage::Text(s)).unwrap();
                    }
                    if let Ok(OwnedMessage::Close(_)) = receiver.recv_message() {
                        println!("Client {} disconnected", ip);
                        break;
                    }
                }
            });
        }
    });
    rocket::build()
        .mount(
            "/api/v1/",
            routes![
                // users::create,
                // users::test,
                instance::start,
                instance::stop,
                instance::send,
                instance::setup,
                instance::delete,
                instance::download_status,
                instance::status,
                instance::get_list,
                instance::player_count,
                instance::player_list,
                instance::list_resource,
                instance::load_resource,
                instance::unload_resource,
                instance::upload,
                instance::download_mod,
                instance::download_world,
                // instance::events,
                jar::vanilla_versions,
                jar::vanilla_jar,
                jar::vanilla_filters,
                jar::fabric_versions,
                jar::fabric_jar,
                jar::fabric_filters,
                jar::paper_versions,
                jar::paper_jar,
                jar::paper_filters,
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
        .register("/", catchers![internal_server_error])
        .manage(MyManagedState {
            instance_manager,
            download_status: CHashMap::new(),
            ws_sender: std::sync::Mutex::new(tx),
        })
        .attach(CORS)
        .launch()
        .await;
    println!("shutting down");
}
