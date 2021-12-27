use std::process::{Command, Stdio, ChildStdout, Child};
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::env;
use bus::Bus;
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
    pub running: bool,
    pub stdin: Option<Sender<String>>,
    stdout: Option<Receiver<String>>,
    jvm_args: Vec<String>,
    process: Option<Child>,
    broadcaster: Option<Bus<bool>>
}



impl ServerInstance {
    pub fn new(config : Option<InstanceConfig>) -> ServerInstance {
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
            stdin: None,
            stdout: None,
            jvm_args,
            process: None,
            broadcaster: None,
            }
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
                    let mut stdin_writer = proc.stdin.unwrap();
                    let mut broadcaster : Bus<bool> = Bus::new(10);
                    let mut rx = broadcaster.add_rx();
                    let reader = BufReader::new(proc.stdout
                        .ok_or_else(|| Error::new(ErrorKind::Other,"bruh")).unwrap());
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
                    thread::spawn(move || {
                        for line_result in reader.lines() {
                            let line = line_result.unwrap();
                            println!("Server said: {}", line);
                        }
                        println!("reader thread terminating");

                    });
                    self.running = true;
                    self.stdin = Some(stdin_sender);
                    self.stdout = None;
                    self.broadcaster = Some(broadcaster);
                    return Ok(())
                }
                Err(e) => return Err(e),
            };
    }
    pub fn stop(&mut self) -> Result<(), std::io::Error> {
        self.stdin.clone().unwrap().send("stop\n".to_string()).unwrap();
        self.broadcaster.as_mut().unwrap().broadcast(true);
        self.running = false;
        Ok(())
    }

}
