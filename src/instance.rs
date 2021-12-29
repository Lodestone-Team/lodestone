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
use uuid::Uuid; 



pub struct InstanceConfig {
    min_ram: u32,
    max_ram: u32,    
}
#[derive(Clone)]
#[derive(PartialEq)]
enum BroadcastCommand {
    Terminate,
    Continue,       
}

pub struct ServerInstance {
    pub stdin: Option<Sender<String>>,
    pub name : String,
    running: bool,
    stdout: Option<Receiver<String>>,
    jvm_args: Vec<String>,
    process: Option<Child>,
    broadcaster: Option<Bus<bool>>,
    path : String,
    pub uuid : String
}



impl ServerInstance {
    pub fn new(config : Option<InstanceConfig>, path : String, name : String) -> ServerInstance {
        let mut jvm_args : Vec<String> = vec![];
        match config {
            None => {
                jvm_args.push("-jar".to_string());
                jvm_args.push("server.jar".to_string());
                jvm_args.push("nogui".to_string());
            }
            Some(instance_config) => {
                jvm_args.push(format!("-Xms{}M", instance_config.min_ram));
                jvm_args.push(format!("-Xmx{}M", instance_config.max_ram));
                jvm_args.push("-jar".to_string());
                jvm_args.push("server.jar".to_string());
                jvm_args.push("nogui".to_string());
            }
        }
            ServerInstance {
                running: false,
                name,
                stdin: None,
                stdout: None,
                jvm_args,
                process: None,
                broadcaster: None,
                path,
                uuid: format!("{}", Uuid::new_v4()),
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
