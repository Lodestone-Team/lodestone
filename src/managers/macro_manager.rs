use std::{
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::ChildStdin,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use regex::Regex;
use rlua::{Lua, MultiValue};

use crate::event_processor::EventProcessor;

pub struct MacroManager {
    pub path: PathBuf,
    stdin_sender: Option<Arc<Mutex<ChildStdin>>>,
    event_processor: Option<Arc<Mutex<EventProcessor>>>,
}

impl MacroManager {
    pub fn new(
        path: PathBuf,
        stdin_sender: Option<Arc<Mutex<ChildStdin>>>,
        event_processor: Option<Arc<Mutex<EventProcessor>>>,
    ) -> MacroManager {
        fs::create_dir_all(path.as_path()).map_err(|e| e.to_string());
        MacroManager {
            path,
            stdin_sender,
            event_processor,
        }
    }
    pub fn run(&self, name: &str, args: Vec<&str>) -> Result<(), String> {
        let macro_file = fs::File::open(self.path.join(name).with_extension("lua")).map_err(|e| e.to_string())?;
        let mut program: String = String::new();

        for line_result in io::BufReader::new(macro_file).lines() {
            program.push_str(format!("{}\n", line_result.unwrap()).as_str());
        }

        Lua::new().context(move |lua_ctx| {
            for (pos, arg) in args.iter().enumerate() {
                println!("setting {} to {}", format!("arg{}", pos + 1), arg);
                lua_ctx
                    .globals()
                    .set(format!("arg{}", pos + 1), arg.clone());
            }
            let delay_sec = lua_ctx
                .create_function(|_, time: u64| {
                    thread::sleep(std::time::Duration::from_secs(time));
                    Ok(())
                })
                .unwrap();
            lua_ctx.globals().set("delay_sec", delay_sec);

            let event_processor = self.event_processor.clone();
            let await_msg = lua_ctx
                .create_function(move |lua_ctx, ()| {
                    if let Some(event_processor_clone) = &event_processor {
                        let (tx, rx) = mpsc::channel();
                        let tx = Arc::new(Mutex::new(tx));
                        let index = event_processor_clone.lock().unwrap().on_chat.len();
                        event_processor_clone.lock().unwrap().on_chat.push(Arc::new(
                            move |player, player_msg| {
                                tx.lock().unwrap().send((player, player_msg)).unwrap();
                            },
                        ));
                        println!("awaiting message");
                        let (player, player_msg) = rx.recv().unwrap();
                        println!("got message from {}: {}", player, player_msg);
                        // remove the callback
                        event_processor_clone.lock().unwrap().on_chat.remove(index);
                        Ok((player, player_msg))
                    } else {
                        Ok((String::from(""), String::from("")))
                    }
                })
                .unwrap();
            lua_ctx.globals().set("await_msg", await_msg);
            let delay_milli = lua_ctx
                .create_function(|_, time: u64| {
                    thread::sleep(std::time::Duration::from_millis(time));
                    Ok(())
                })
                .unwrap();
            lua_ctx.globals().set("delay_milli", delay_milli);
            let stdin_sender_closure = self.stdin_sender.clone();
            let send_stdin = lua_ctx
                .create_function(move |ctx, line: String| {
                    if let Some(stdin_sender) = &stdin_sender_closure {
                        let reg = Regex::new(r"\$\{(\w*)\}").unwrap();
                        let globals = ctx.globals();
                        let mut after = line.clone();
                        if reg.is_match(&line) {
                            for cap in reg.captures_iter(&line) {
                                println!("cap1: {}", cap.get(1).unwrap().as_str());
                                after = after.replace(
                                    format!("${{{}}}", &cap[1]).as_str(),
                                    &globals.get::<_, String>(cap[1].to_string()).unwrap(),
                                );
                                println!("after: {}", after);
                            }

                            stdin_sender
                                .lock()
                                .as_mut()
                                .unwrap()
                                .write_all(format!("{}\n", after).as_bytes());
                        } else {
                            stdin_sender
                                .lock()
                                .unwrap()
                                .write_all(format!("{}\n", line).as_bytes());
                        }
                    }
                    Ok(())
                })
                .unwrap();
            lua_ctx.globals().set("send_stdin", send_stdin);

            lua_ctx.globals().set(
                "isBadWord",
                lua_ctx
                    .create_function(|_, word: String| {
                        use censor::*;
                        let censor = Standard + "lambda";
                        Ok((censor.check(word.as_str()),))
                    })
                    .unwrap(),
            );

            match lua_ctx.load(&program).eval::<MultiValue>() {
                Ok(value) => {
                    println!(
                        "{}",
                        value
                            .iter()
                            .map(|value| format!("{:?}", value))
                            .collect::<Vec<_>>()
                            .join("\t")
                    );
                }
                // Err(Error::SyntaxError {
                //     incomplete_input: true,
                //     ..
                // }) => {}
                Err(e) => {
                    eprintln!("error: {}", e);
                }
            }
        });
        Ok(())
    }

    /// Set the macro manager's stdin sender.
    pub fn set_stdin_sender(&mut self, stdin_sender: Option<Arc<Mutex<ChildStdin>>>) {
        self.stdin_sender = stdin_sender;
    }

    /// Set the macro manager's event processor.
    pub fn set_event_processor(&mut self, event_processor: Option<Arc<Mutex<EventProcessor>>>) {
        self.event_processor = event_processor;
    }
}
