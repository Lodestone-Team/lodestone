
use rocket::http::Status;
use rocket::State;
use rocket::serde::json::Json;
use sys_info::{os_type, os_release, cpu_num, cpu_speed, disk_info, mem_info};
extern crate uptime_lib;
extern crate cpuid;
#[get("/api/sys/mem")]
pub async fn get_ram() -> (Status, String) {
    match mem_info() {
        Ok(mem) => return (Status::Ok, format!("{}/{}", mem.free, mem.total)),
        Err(_) => return (Status::InternalServerError, "failed to get ram".to_string())
    }
}

#[get("/api/sys/disk")]
pub async fn get_disk() -> (Status, String) {
    match disk_info() {
        Ok(disk) => return (Status::Ok, format!("{}/{}", disk.free, disk.total)),
        Err(_) => return (Status::InternalServerError, "failed to get disk".to_string())
    }
}

#[get("/api/sys/cpuspeed")]
pub async fn get_cpu_speed() -> (Status, String) {
    match cpu_speed() {
        Ok(cpuspeed) => return (Status::Ok, cpuspeed.to_string()),
        Err(_) => return (Status::InternalServerError, "failed to get cpu speed".to_string())
    }
}
/// DOES NOT WORK IN WSL
#[get("/api/sys/cpuinfo")]
pub async fn get_cpu_info() -> (Status, String) {
    match cpuid::identify() {
        Ok(cpuinfo) => return (Status::Ok, format!("{} {}", cpuinfo.vendor, cpuinfo.codename)),
        Err(_) => return (Status::InternalServerError, "failed to get cpu info".to_string())
    }
}

#[get("/api/sys/osinfo")]
pub async fn get_os_info() -> (Status, String) {
    match os_release() {
        Ok(release) => {
            match os_type() {
                Ok(ostype) => (Status::Ok, format!("{} {}", ostype, release)),
                Err(_) => return (Status::InternalServerError, "failed to get os info".to_string())
            }
        }
        Err(_) => return (Status::InternalServerError, "failed to get os info".to_string())
    }
}

#[get("/api/sys/uptime")]
pub async fn get_uptime() -> (Status, String) {
    match uptime_lib::get() {
        Ok(uptime) => return (Status::Ok, format!("{}", uptime.as_secs_f64().to_string())),
        Err(_) => return (Status::InternalServerError, "failed to get cpu info".to_string())
    }
}