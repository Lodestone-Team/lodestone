use std::process::{Command, Stdio, ChildStdout, Child};
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::Duration;
use std::env;

pub struct InstanceConfig {
    min_ram: u32,
    max_ram: u32,    
}

pub struct ServerInstance {
    pub running: bool,
    pub stdin: Option<Sender<String>>,
    pub stdout: Option<Receiver<String>>,
    jvm_args: Vec<String>,
    process: Option<Child>
}

impl ServerInstance {
    pub fn new(config : Option<InstanceConfig>) -> Arc<Mutex<ServerInstance>> {
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
        Arc::new(Mutex::new( 
            ServerInstance {
            running: false,
            stdin: None,
            stdout: None,
            jvm_args,
            process: None
        }))
    }

    pub fn start(&mut self) -> Result<(), std::io::Error> {
        env::set_current_dir("/home/peter/Lodestone/backend/mcserver").unwrap(); // purely for debug
        let _ = match 
            Command::new("java")
            .args(&self.jvm_args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn() {
                Ok(proc) => {
                    let (stdin_sender, stdin_receiver) : (Sender<String>, Receiver<String>) = mpsc::channel();
                    let (stdout_sender, stdout_receiver) : (Sender<String>, Receiver<String>) = mpsc::channel();
                    let mut stdin_writer = proc.stdin.unwrap();
                    let reader = BufReader::new(proc.stdout
                        .ok_or_else(|| Error::new(ErrorKind::Other,"bruh")).unwrap());
                    thread::spawn(move || {
                        for rec in stdin_receiver {
                            stdin_writer.write_all(rec.as_bytes()).unwrap();
                        }
                    });
                    thread::spawn(move || {
                        reader.lines()
                        .filter_map(|line| line.ok())
                        .for_each(|line| stdout_sender.send(line).unwrap());                    
                    });
                    self.running = true;
                    self.stdin = Some(stdin_sender);
                    self.stdout = Some(stdout_receiver);
                    return Ok(())
                }
                Err(e) => return Err(e),
            };
    }

}
