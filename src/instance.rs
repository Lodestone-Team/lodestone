use std::char::UNICODE_VERSION;
use std::process::{Command, Stdio, ChildStdout, Child};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;
use bus::Bus;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct InstanceConfig {
    pub name: String,
    pub version: String,
    pub flavour: String,
    pub url: String,
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

pub struct ServerInstance {
    pub name : String,
    jvm_args: Vec<String>,
    path : String,
    pub uuid : String,
    pub stdin: Option<Sender<String>>,
    running: bool,
    stdout: Option<Receiver<String>>,
    process: Option<Child>,
    broadcaster: Option<Bus<bool>>,
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
            running: false,
            name: config.name.clone(),
            stdin: None,
            stdout: None,
            jvm_args,
            process: None,
            broadcaster: None,
            path,
            uuid: config.uuid.as_ref().unwrap().clone(),
        }
    }

    pub fn start(&mut self, mongoDBClient: Client) -> Result<(), String> {
        env::set_current_dir(self.path.as_str()).unwrap(); // purely for debug
        if self.running {
            return Err("already running".to_string());
        }
        let _ = match 
            Command::new("java")
            .args(&self.jvm_args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn() {
                Ok(proc) => {
                    let (stdin_sender, stdin_receiver) : (Sender<String>, Receiver<String>) = mpsc::channel();
                    let mut stdin_writer = proc.stdin.ok_or("failed to open stdin of child process")?;
                    let stdout = proc.stdout.ok_or("failed to open stdin of child process")?;
                    let mut broadcaster : Bus<bool> = Bus::new(10);
                    let mut rx = broadcaster.add_rx();
                    let reader = BufReader::new(stdout);
                    thread::spawn(move || {
                        let stdin_receiver = stdin_receiver;
                        loop {
                            if rx.try_recv().is_ok() { break; }
                            let rec = stdin_receiver.recv().unwrap();
                            println!("writing to stdin: {}", rec);
                            stdin_writer.write_all(rec.as_bytes()).unwrap();
                            stdin_writer.flush().unwrap();
                        }
                        
                        println!("writer thread terminating");

                    });
                    let uuid = self.uuid.clone();
                    thread::spawn(move || {
                        for line_result in reader.lines() {
                            let line = line_result.unwrap();

                            let time128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                            let time = i64::try_from(time128).unwrap();
                            println!("Server said: {}", line);
                            mongoDBClient
                                .database(uuid.as_str())
                                .collection("logs")
                                .insert_one(doc! {
                                    "time": time, 
                                    "log": line
                                }, None).unwrap();
                        }
                        println!("reader thread terminating");

                    });
                    self.running = true;
                    self.stdin = Some(stdin_sender);
                    self.stdout = None;
                    self.broadcaster = Some(broadcaster);
                    return Ok(())
                }
                Err(_) => return Err("failed to open child process".to_string())
            };
    }
    pub fn stop(&mut self) -> Result<(), String> {
        if !self.running {
            return Err("server already stopped".to_string());
        }
        self.stdin.clone().unwrap().send("stop\n".to_string()).unwrap();
        self.broadcaster.as_mut().unwrap().broadcast(true);
        self.running = false;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn get_path(&self) -> String {
        self.path.clone()
    }

}
