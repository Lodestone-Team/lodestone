#[macro_use]
extern crate rocket;
extern crate sanitize_filename;

use chashmap::CHashMap;
use futures_util::lock::Mutex;
use std::env;
use std::sync::Arc;
mod handlers;
mod managers;
mod util;
use handlers::*;
use instance_manager::InstanceManager;
use managers::*;
use mongodb::{options::ClientOptions, sync::Client};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

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
            kind: Kind::Response
        }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        res.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET, PATCH, OPTIONS, DELETE"));
        res.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        res.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}


#[launch]
async fn rocket() -> _ {
    let mut client_options = ClientOptions::parse("mongodb://localhost:27017/?tls=false").unwrap();
    client_options.app_name = Some("MongoDB Client".to_string());

    let client = Client::with_options(client_options).unwrap();

    rocket::build()
        .mount(
            "/",
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
                jar::vanilla_versions,
                jar::vanilla_jar,
                jar::vanilla_options,
                jar::flavours,
                system::get_ram,
                system::get_disk,
                system::get_cpu_speed,
                system::get_cpu_info,
                system::get_os_info,
                system::get_uptime
            ],
        )
        .manage(MyManagedState {
            instance_manager: Arc::new(Mutex::new(
                InstanceManager::new(
                    format!("{}/InstanceTest/", env::current_dir().unwrap().display()),
                    client.clone(),
                )
                .unwrap(),
            )),
            download_status: CHashMap::new(),
            mongodb_client: client,
        })
        .attach(CORS)
}
