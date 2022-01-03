#[macro_use]
extern crate rocket;
extern crate sanitize_filename;

use chashmap::CHashMap;
use futures_util::lock::Mutex;
use std::env;
use std::sync::Arc;
mod handlers;
mod util;
mod managers;
use managers::*;
use handlers::*;
use users::*;
use instance_manager::InstanceManager;
use mongodb::{options::ClientOptions, sync::Client};
pub struct MyManagedState {
    instance_manager: Arc<Mutex<InstanceManager>>,
    download_status: CHashMap<String, (u64, u64)>,
    mongodb_client: Client,
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
            instance_manager: Arc::new(Mutex::new(InstanceManager::new(
                format!("{}/InstanceTest/", env::current_dir().unwrap().display()),
                client.clone(),
            ).unwrap())),
            download_status: CHashMap::new(),
            mongodb_client: client,
        })
}
