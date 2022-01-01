use std::char::UNICODE_VERSION;
use std::process::{Command, Stdio, ChildStdout, Child};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};
use serde::{Serialize, Deserialize};
use regex::Regex;
use crate::instance_manager::InstanceManager;


#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct InstanceConfig {
    pub name: String,
    pub version: String,
    pub flavour: String,
    /// url to download the server.jar file from upon setup
    pub url: Option<String>, 
    pub port : Option<u32>,
    pub uuid: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>
}

#[derive(Clone)]
#[derive(PartialEq)]
enum BroadcastCommand {
    Terminate,
    Continue,       
}
#[derive(Clone, Copy)]
pub enum Status {
    Starting,
    Stopping,
    Running,
    Stopped,
}
pub struct ServerInstance {
    pub name : String,
    jvm_args: Vec<String>,
    path : String,
    pub uuid : String,
    pub stdin: Option<Sender<String>>,
    status: Arc<Mutex<Status>>,
    process: Arc<Mutex<Option<Child>>>,
    kill_tx: Option<Sender<()>>,
}



impl ServerInstance {
    pub fn new(config : &InstanceConfig, path: String) -> ServerInstance {
        let mut jvm_args : Vec<String> = vec![];

        jvm_args.push("-jar".to_string());
        jvm_args.push("server.jar".to_string());
        jvm_args.push("nogui".to_string());

        match config.min_ram {
            Some(min_ram) => jvm_args.push(format!("-Xms{}M", min_ram)),
            None => ()
        }
        match config.max_ram {
            Some(max_ram) => jvm_args.push(format!("-Xmx{}M", max_ram)),
            None => ()
        }

        ServerInstance {
            status: Arc::new(Mutex::new(Status::Stopped)),
            name: config.name.clone(),
            stdin: None,
            jvm_args,
            process: Arc::new(Mutex::new(None)),
            kill_tx: None,
            path,
            uuid: config.uuid.as_ref().unwrap().clone(),
        }
    }

    pub fn start(&mut self, mongodb_client: Client) -> Result<(), String> {
        let mut status = self.status.lock().unwrap();
        env::set_current_dir(self.path.as_str()).unwrap(); 
        match *status {
            Status::Starting => return Err("cannot start, instance is already starting".to_string()),
            Status::Stopping => return Err("cannot start, instance is stopping".to_string()),
            Status::Running => return Err("cannot start, instance is already running".to_string()),
            Status::Stopped => (),
        }
        *status = Status::Starting;
        let mut command = Command::new("java");
        command
        .args(&self.jvm_args)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped());
        match command 
            .spawn() {
                Ok(proc) => {
                    let (stdin_sender, stdin_receiver) : (Sender<String>, Receiver<String>) = mpsc::channel();
                    let (kill_tx, kill_rx) : (Sender<()>, Receiver<()>) = mpsc::channel();

                    let mut stdin_writer = proc.stdin.ok_or("failed to open stdin of child process")?;
                    let stdout = proc.stdout.ok_or("failed to open stdin of child process")?;
                    let reader = BufReader::new(stdout);
                    thread::spawn(move || {
                        let stdin_receiver = stdin_receiver;
                        loop {
                            if kill_rx.try_recv().is_ok() { break; }
                            let rec = stdin_receiver.recv().unwrap();
                            println!("writing to stdin: {}", rec);
                            stdin_writer.write_all(rec.as_bytes()).unwrap();
                            stdin_writer.flush().unwrap();
                        }
                        
                        println!("writer thread terminating");

                    });
                    let uuid_closure = self.uuid.clone();
                    let status_closure = self.status.clone();
                    thread::spawn(move || {
                        let re = Regex::new(r"\[Server thread/INFO\]: Done").unwrap();
                        for line_result in reader.lines() {
                            let mut status = status_closure.lock().unwrap();
                            let line = line_result.unwrap();
                            let time128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                            let time = i64::try_from(time128).unwrap();
                            if re.is_match(line.as_str()) {
                                *status = Status::Running;
                            }
                            println!("Server said: {}", line);
                            mongodb_client
                                .database(uuid_closure.as_str())
                                .collection("logs")
                                .insert_one(doc! {
                                    "time": time, 
                                    "log": line
                                }, None).unwrap();
                        }
                        let mut status = status_closure.lock().unwrap();
                        println!("program exiting as reader thread is terminating...");
                        match *status {
                            Status::Starting => println!("instance failed to start"),
                            Status::Stopping => println!("instance is already stopping, this is not ok"),
                            Status::Running => println!("instance exited unexpectedly, restarting..."), //TODO: Restart thru localhost
                            Status::Stopped => println!("instance stopped properly, exiting..."),
                        }
                        *status = Status::Stopped;
                    });
                    self.stdin = Some(stdin_sender);
                    self.kill_tx = Some(kill_tx);
                    return Ok(())
                }
                Err(_) => return Err("failed to open child process".to_string())
            };
    }
    pub fn stop(&mut self) -> Result<(), String> {
        let mut status = self.status.lock().unwrap();
        match *status {
            Status::Starting => return Err("cannot stop, instance is starting".to_string()),
            Status::Stopping => return Err("cannot stop, instance is already stopping".to_string()),
            Status::Stopped => return Err("cannot stop, instance is already stopped".to_string()),
            Status::Running => println!("stopping instance"),
        }
        *status = Status::Stopping;
        self.stdin.clone().unwrap().send("stop\n".to_string()).unwrap();
        self.kill_tx.as_mut().unwrap().send(());
        *status = Status::Stopped;
        Ok(())
    }

    pub fn get_status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn get_process(&self) -> Arc<Mutex<Option<Child>>> {
        self.process.clone()
    }

    pub fn get_path(&self) -> String {
        self.path.clone()
    }

}
