//! A loopback file server for the download and install tests, on a port the OS picks.
//! Each test starts its own. Every connection is handled on its own thread and closed
//! after one response, so the tests can see how many downloads run at once.
//!
//! It is a small HTTP/1.1 server on `std::net` rather than `tiny_http`: tiny_http's
//! worker pool can leave a new connection queued behind idle keep-alive connections
//! until the client drops them (30 seconds for reqwest), which made tests slow at
//! random.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use url::Url;

/// How a path answers.
#[derive(Clone, Debug)]
pub struct Entry {
    pub body: Vec<u8>,
    /// Answer this many requests with HTTP 500 first.
    pub fail_first: u32,
    /// Answer this many requests with the wrong bytes first.
    pub corrupt_first: u32,
    pub status: u16,
}

impl Entry {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: body.into(),
            fail_first: 0,
            corrupt_first: 0,
            status: 200,
        }
    }
}

#[derive(Default)]
pub struct State {
    pub entries: HashMap<String, Entry>,
    /// Requests per path.
    pub hits: HashMap<String, u32>,
    pub user_agents: Vec<String>,
    pub in_flight: usize,
    pub max_in_flight: usize,
    /// When set, a request waits until this many are in flight at once (or the
    /// timeout passes) before it is answered.
    pub hold_until: Option<usize>,
    pub released: bool,
}

pub struct FileServer {
    pub base: Url,
    state: Arc<(Mutex<State>, Condvar)>,
    stop: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

/// How long a held request waits for the others before it gives up; only reached
/// when the code under test is wrong.
const HOLD_TIMEOUT: Duration = Duration::from_secs(10);

impl FileServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = state.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let state = state.clone();
                    std::thread::spawn(move || answer(&state, stream));
                }
            })
        };
        Self {
            base: Url::parse(&format!("http://{addr}/")).unwrap(),
            state,
            stop,
            addr,
            thread: Some(thread),
        }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.0.lock().unwrap()
    }

    /// Serves `body` at the mirrored path of a Mojang URL, such as
    /// `https://libraries.minecraft.net/a.jar` at `/libraries.minecraft.net/a.jar`.
    pub fn serve(&self, mojang_url: &str, entry: Entry) {
        let path = mirror_path(mojang_url);
        self.state().entries.insert(path, entry);
    }

    pub fn hits(&self, mojang_url: &str) -> u32 {
        self.state()
            .hits
            .get(&mirror_path(mojang_url))
            .copied()
            .unwrap_or(0)
    }

    pub fn total_hits(&self) -> u32 {
        self.state().hits.values().sum()
    }
}

fn mirror_path(mojang_url: &str) -> String {
    let url = Url::parse(mojang_url).unwrap();
    format!("/{}{}", url.host_str().unwrap(), url.path())
}

/// Reads one request's line and headers. GET requests have no body.
fn read_request(stream: &TcpStream) -> Option<(String, Option<String>)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?.to_string();
    let mut user_agent = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("user-agent")
        {
            user_agent = Some(value.trim().to_string());
        }
    }
    Some((target, user_agent))
}

fn answer(state: &(Mutex<State>, Condvar), mut stream: TcpStream) {
    let Some((target, user_agent)) = read_request(&stream) else {
        return;
    };
    let (lock, condvar) = state;
    let path = target.split('?').next().unwrap_or_default().to_string();
    let (status, body) = {
        let mut state = lock.lock().unwrap();
        *state.hits.entry(path.clone()).or_default() += 1;
        if let Some(agent) = user_agent {
            state.user_agents.push(agent);
        }
        state.in_flight += 1;
        state.max_in_flight = state.max_in_flight.max(state.in_flight);
        if let Some(wanted) = state.hold_until {
            if state.in_flight >= wanted {
                state.released = true;
                condvar.notify_all();
            }
            let (guard, _) = condvar
                .wait_timeout_while(state, HOLD_TIMEOUT, |state| !state.released)
                .unwrap();
            state = guard;
        }
        let answer = match state.entries.get_mut(&path) {
            None => (404, b"not found".to_vec()),
            Some(entry) if entry.fail_first > 0 => {
                entry.fail_first -= 1;
                (500, b"try again".to_vec())
            }
            Some(entry) if entry.corrupt_first > 0 => {
                entry.corrupt_first -= 1;
                let mut body = entry.body.clone();
                match body.first_mut() {
                    Some(byte) => *byte ^= 0xff,
                    None => body.push(0),
                }
                (200, body)
            }
            Some(entry) => (entry.status, entry.body.clone()),
        };
        state.in_flight -= 1;
        answer
    };
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

impl Drop for FileServer {
    fn drop(&mut self) {
        // Wake the accept loop with one last connection so it sees the flag.
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
