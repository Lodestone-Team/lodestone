//! A loopback file server for the download and install tests, on a port the OS picks.
//! Each test starts its own. Every request is answered on its own thread, so the
//! tests can see how many downloads run at once.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    sync::{Arc, Condvar, Mutex, MutexGuard},
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
    server: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
}

/// How long a held request waits for the others before it gives up; only reached
/// when the code under test is wrong.
const HOLD_TIMEOUT: Duration = Duration::from_secs(10);

impl FileServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let thread = {
            let server = server.clone();
            let state = state.clone();
            std::thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    let state = state.clone();
                    std::thread::spawn(move || answer(&state, request));
                }
            })
        };
        Self {
            base: Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            state,
            server,
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

fn answer(state: &(Mutex<State>, Condvar), request: tiny_http::Request) {
    let (lock, condvar) = state;
    let path = request
        .url()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let (status, body) = {
        let mut state = lock.lock().unwrap();
        *state.hits.entry(path.clone()).or_default() += 1;
        if let Some(agent) = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("User-Agent"))
        {
            state.user_agents.push(agent.value.to_string());
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
    let _ = request.respond(tiny_http::Response::from_data(body).with_status_code(status));
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
