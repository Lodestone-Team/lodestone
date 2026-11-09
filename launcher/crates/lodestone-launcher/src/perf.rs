//! The performance monitor's numbers: the launcher's memory, CPU, threads and open
//! files, how often the window redraws, a running game's memory and CPU, and what the
//! launcher holds (decoded images, the Modrinth cache, jobs, downloads, CLI clients,
//! the trace bus).
//!
//! Nothing here runs unless asked: the monitor window and `perf` on the control
//! socket take a [`Sampler`] about once a second, and frames are only counted while
//! one exists.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use gpui::{App, Entity, RenderImage};
use launcher_core::process_stats::{self, ProcessStats};
use serde_json::{Value, json};

use crate::{
    accounts::Accounts,
    instances::{Instances, content::ContentView, worlds::WorldsView},
};

/// Where the numbers come from: the app's entities.
#[derive(Clone)]
pub struct Sources {
    pub instances: Entity<Instances>,
    pub accounts: Entity<Accounts>,
    pub content: Option<Entity<ContentView>>,
    pub worlds: Option<Entity<WorldsView>>,
}

/// Frames are kept for this long.
const FRAME_HISTORY: Duration = Duration::from_secs(10);
/// Frame stats cover this much of the latest history.
const FRAME_WINDOW: Duration = Duration::from_secs(5);
/// No frame for this long: the window is idle.
const IDLE_AFTER: Duration = Duration::from_secs(2);

static RECORDERS: AtomicUsize = AtomicUsize::new(0);
static FRAMES: Mutex<Frames> = Mutex::new(Frames {
    since: None,
    at: VecDeque::new(),
    active: None,
});

struct Frames {
    /// When recording started.
    since: Option<Instant>,
    at: VecDeque<Frame>,
    /// Whether the main window had focus at its last frame.
    active: Option<bool>,
}

/// One frame of the main window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// When it started: the window's root view rendering.
    pub start: Instant,
    /// How long it took to render, lay out and paint, until its last element
    /// painted. `None` until then.
    pub took: Option<Duration>,
}

fn frames() -> std::sync::MutexGuard<'static, Frames> {
    FRAMES.lock().unwrap_or_else(|p| p.into_inner())
}

/// Whether frames are being counted: one atomic load.
pub fn recording() -> bool {
    RECORDERS.load(Ordering::Relaxed) > 0
}

/// The main window started a frame, with focus or not. One atomic load while
/// nothing records.
pub fn frame(at: Instant, active: bool) {
    if !recording() {
        return;
    }
    let mut frames = frames();
    frames.active = Some(active);
    frames.at.push_back(Frame {
        start: at,
        took: None,
    });
    while frames
        .at
        .front()
        .is_some_and(|first| at.duration_since(first.start) > FRAME_HISTORY)
    {
        frames.at.pop_front();
    }
}

/// The main window's frame finished painting.
pub fn frame_painted(at: Instant) {
    if !recording() {
        return;
    }
    if let Some(last) = frames().at.back_mut()
        && last.took.is_none()
    {
        last.took = Some(at.saturating_duration_since(last.start));
    }
}

/// Counts frames while it lives.
struct Recording;

impl Recording {
    fn start() -> Self {
        if RECORDERS.fetch_add(1, Ordering::Relaxed) == 0 {
            let mut frames = frames();
            frames.since = Some(Instant::now());
            frames.at.clear();
            frames.active = None;
        }
        Recording
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if RECORDERS.fetch_sub(1, Ordering::Relaxed) == 1 {
            let mut frames = frames();
            frames.since = None;
            frames.at.clear();
            frames.active = None;
        }
    }
}

/// How the main window drew over the last few seconds.
///
/// The launcher draws only when something on screen changes, so `fps` is how
/// often it redrew, not how fast it can: a still window draws nothing, a spinner
/// draws at the display's rate. The frame times are what each of those frames cost,
/// from rendering to the last element painted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Frames drawn per second.
    pub fps: f64,
    /// Time each frame took to render, lay out and paint, in milliseconds.
    pub avg_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub max_ms: Option<f64>,
    /// No frame for a while: nothing on screen changed.
    pub idle: bool,
    /// The main window was in the background at its last frame. Background windows
    /// animate at no more than 30 frames per second.
    pub background: bool,
}

/// Frame stats from `frames`, recorded since `since`, at `now`.
pub fn frame_stats(frames: &[Frame], since: Instant, now: Instant) -> FrameStats {
    let span = now.duration_since(since).min(FRAME_WINDOW);
    let start = now - span;
    let recent: Vec<&Frame> = frames.iter().filter(|f| f.start >= start).collect();
    let mut took: Vec<f64> = recent
        .iter()
        .filter_map(|f| f.took)
        .map(|took| took.as_secs_f64() * 1000.0)
        .collect();
    took.sort_by(|a, b| a.total_cmp(b));
    let avg = (!took.is_empty()).then(|| took.iter().sum::<f64>() / took.len() as f64);
    let p95 = (!took.is_empty()).then(|| {
        let index = ((took.len() as f64 * 0.95).ceil() as usize).clamp(1, took.len()) - 1;
        took[index]
    });
    let fps = if span.is_zero() {
        0.0
    } else {
        recent.len() as f64 / span.as_secs_f64()
    };
    let last = frames.last().map_or(since, |f| f.start);
    FrameStats {
        fps,
        avg_ms: avg,
        p95_ms: p95,
        max_ms: took.last().copied(),
        idle: now.duration_since(last) >= IDLE_AFTER,
        background: false,
    }
}

/// A process's numbers at one reading.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcessReading {
    pub pid: u32,
    /// Bytes: the physical footprint on macOS, the working set on Windows, the
    /// resident size elsewhere.
    pub memory: Option<u64>,
    /// Percent of one core since the last reading.
    pub cpu_percent: Option<f64>,
    pub threads: Option<u32>,
    pub open_files: Option<u32>,
}

/// A running game.
#[derive(Clone, Debug, PartialEq)]
pub struct GameReading {
    pub instance: String,
    pub name: String,
    pub process: ProcessReading,
}

/// Decoded images held in memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageTally {
    pub count: usize,
    pub bytes: u64,
}

impl ImageTally {
    pub fn add(&mut self, image: &RenderImage) {
        self.count += 1;
        let mut frame = 0;
        while let Some(bytes) = image.as_bytes(frame) {
            self.bytes += bytes.len() as u64;
            frame += 1;
        }
    }
}

/// The trace bus while traces are on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceReading {
    pub subscribers: usize,
    pub events_per_sec: Option<f64>,
}

/// One reading of everything.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub at: SystemTime,
    pub launcher: ProcessReading,
    pub cores: usize,
    pub frames: FrameStats,
    pub games: Vec<GameReading>,
    pub images: ImageTally,
    /// Modrinth responses kept: (entries, bytes).
    pub content_cache: Option<(usize, u64)>,
    /// Control API jobs running.
    pub jobs: usize,
    /// Instances downloading game files, and their bytes per second together.
    pub downloads: usize,
    pub download_rate: Option<f64>,
    /// Programs connected to the control socket; `None` without one.
    pub clients: Option<usize>,
    /// `None` while traces are off.
    pub trace: Option<TraceReading>,
}

/// Takes readings, keeping what the next one compares with. Frames are counted
/// while one exists.
pub struct Sampler {
    last: Option<Last>,
    _recording: Recording,
}

struct Last {
    at: Instant,
    processes: HashMap<u32, ProcessStats>,
    download_bytes: HashMap<String, u64>,
    trace_seq: u64,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            last: None,
            _recording: Recording::start(),
        }
    }

    pub fn sample(&mut self, sources: &Sources, cx: &App) -> Sample {
        let now = Instant::now();
        let wall = self.last.as_ref().map(|last| now.duration_since(last.at));
        let mut processes = HashMap::new();
        let mut read = |pid: u32| {
            let stats = process_stats::sample(pid);
            let before = self.last.as_ref().and_then(|l| l.processes.get(&pid));
            let cpu_percent = match (before, &stats, wall) {
                (Some(before), Some(after), Some(wall)) => {
                    process_stats::cpu_percent(before, after, wall)
                }
                _ => None,
            };
            if let Some(stats) = stats {
                processes.insert(pid, stats);
            }
            ProcessReading {
                pid,
                memory: stats.and_then(|s| s.memory),
                cpu_percent,
                threads: stats.and_then(|s| s.threads),
                open_files: stats.and_then(|s| s.open_files),
            }
        };
        let launcher = read(std::process::id());

        let instances = sources.instances.read(cx);
        let mut games = Vec::new();
        let mut downloads = 0;
        let mut download_bytes = HashMap::new();
        let mut moved = 0u64;
        for instance in instances.list() {
            let status = instances.run_status(&instance.id);
            if let Some(pid) = status.pid {
                games.push(GameReading {
                    instance: instance.id.clone(),
                    name: instance.name.clone(),
                    process: read(pid),
                });
            }
            if let Some((_, progress)) = status.progress
                && progress.bytes_total > 0
            {
                downloads += 1;
                let before = self
                    .last
                    .as_ref()
                    .and_then(|l| l.download_bytes.get(&instance.id))
                    .copied()
                    .unwrap_or(progress.bytes_done);
                moved += progress.bytes_done.saturating_sub(before);
                download_bytes.insert(instance.id.clone(), progress.bytes_done);
            }
        }
        let download_rate = match wall {
            Some(wall) if downloads > 0 && !wall.is_zero() => {
                Some(moved as f64 / wall.as_secs_f64())
            }
            _ => None,
        };

        let mut images = ImageTally::default();
        instances.tally_images(&mut images);
        sources.accounts.read(cx).tally_images(&mut images);
        if let Some(content) = &sources.content {
            content.read(cx).tally_images(&mut images);
        }
        if let Some(worlds) = &sources.worlds {
            worlds.read(cx).tally_images(&mut images);
        }
        crate::images::tally(&mut images, cx);
        crate::skin::tally_images(&mut images, cx);
        let content_cache = instances.modrinth().map(|m| m.cache_stats());

        let bus = launcher_core::trace::bus();
        let trace_seq = bus.last_seq();
        let trace = bus.is_enabled().then(|| TraceReading {
            subscribers: bus.subscribers(),
            events_per_sec: self.last.as_ref().zip(wall).and_then(|(last, wall)| {
                (!wall.is_zero())
                    .then(|| trace_seq.saturating_sub(last.trace_seq) as f64 / wall.as_secs_f64())
            }),
        });

        let frames = {
            let frames = frames();
            let at: Vec<Frame> = frames.at.iter().copied().collect();
            FrameStats {
                background: frames.active == Some(false),
                ..frame_stats(&at, frames.since.unwrap_or(now), now)
            }
        };

        #[cfg(unix)]
        let (jobs, clients) = (
            crate::control::running_jobs(cx),
            crate::control::client_count(cx),
        );
        #[cfg(not(unix))]
        let (jobs, clients) = (0, None);

        self.last = Some(Last {
            at: now,
            processes,
            download_bytes,
            trace_seq,
        });
        Sample {
            at: SystemTime::now(),
            launcher,
            cores: process_stats::cores(),
            frames,
            games,
            images,
            content_cache,
            jobs,
            downloads,
            download_rate,
            clients,
            trace,
        }
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn process_json(process: &ProcessReading) -> Value {
    json!({
        "pid": process.pid,
        "memory_bytes": process.memory,
        "cpu_percent": process.cpu_percent.map(round1),
        "threads": process.threads,
        "open_files": process.open_files,
    })
}

impl Sample {
    /// The reading as `perf` on the control socket answers it.
    pub fn to_json(&self) -> Value {
        let secs = self
            .at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        json!({
            "at": secs,
            "launcher": process_json(&self.launcher),
            "cores": self.cores,
            "cpu_percent_is": "of one core",
            "cpu_percent_of_all_cores": self.launcher.cpu_percent.map(|p| round1(p / self.cores.max(1) as f64)),
            "memory_is": if cfg!(target_os = "macos") {
                "physical footprint"
            } else if cfg!(windows) {
                "working set"
            } else {
                "resident size"
            },
            "frames": {
                "fps": round1(self.frames.fps),
                "avg_ms": self.frames.avg_ms.map(round1),
                "p95_ms": self.frames.p95_ms.map(round1),
                "max_ms": self.frames.max_ms.map(round1),
                "frame_ms_is": "render to last paint",
                "idle": self.frames.idle,
                "background": self.frames.background,
            },
            "games": self.games.iter().map(|game| json!({
                "instance": game.instance,
                "name": game.name,
                "process": process_json(&game.process),
            })).collect::<Vec<_>>(),
            "images": { "count": self.images.count, "bytes": self.images.bytes },
            "content_cache": self.content_cache.map(|(entries, bytes)| json!({ "entries": entries, "bytes": bytes })),
            "jobs": self.jobs,
            "downloads": { "active": self.downloads, "bytes_per_sec": self.download_rate.map(|r| r.round()) },
            "cli_clients": self.clients,
            "trace": self.trace.map(|t| json!({
                "subscribers": t.subscribers,
                "events_per_sec": t.events_per_sec.map(round1),
            })),
        })
    }

    /// The reading as plain text, for a bug report.
    pub fn to_text(&self) -> String {
        let mb = |bytes: Option<u64>| {
            bytes.map_or("?".to_string(), |b| {
                format!("{:.1} MB", b as f64 / 1_048_576.0)
            })
        };
        let pct = |value: Option<f64>| value.map_or("?".to_string(), |v| format!("{v:.1}%"));
        let n = |value: Option<u32>| value.map_or("?".to_string(), |v| v.to_string());
        let ms = |value: Option<f64>| value.map_or("–".to_string(), |v| format!("{v:.1} ms"));
        let mut text = format!(
            "Lodestone Launcher {} ({} {}), {} cores\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.cores
        );
        text += &format!(
            "Launcher (pid {}): memory {}, CPU {} of one core ({} of all {} cores), {} threads, {} {}\n",
            self.launcher.pid,
            mb(self.launcher.memory),
            pct(self.launcher.cpu_percent),
            pct(self
                .launcher
                .cpu_percent
                .map(|p| p / self.cores.max(1) as f64)),
            self.cores,
            n(self.launcher.threads),
            n(self.launcher.open_files),
            if cfg!(windows) {
                "handles"
            } else {
                "open files"
            },
        );
        text += &format!(
            "Rendering: {:.1} fps, frame time avg {} p95 {} max {}{}{}\n",
            self.frames.fps,
            ms(self.frames.avg_ms),
            ms(self.frames.p95_ms),
            ms(self.frames.max_ms),
            if self.frames.idle { " (idle)" } else { "" },
            if self.frames.background {
                " (window in background)"
            } else {
                ""
            },
        );
        for game in &self.games {
            text += &format!(
                "Game {} (pid {}): memory {}, CPU {}\n",
                game.name,
                game.process.pid,
                mb(game.process.memory),
                pct(game.process.cpu_percent),
            );
        }
        if self.games.is_empty() {
            text += "Game: none running\n";
        }
        text += &format!(
            "Decoded images: {} ({})\n",
            self.images.count,
            mb(Some(self.images.bytes))
        );
        if let Some((entries, bytes)) = self.content_cache {
            text += &format!("Content cache: {entries} entries ({})\n", mb(Some(bytes)));
        }
        text += &format!(
            "Jobs: {}, downloads: {}{}\n",
            self.jobs,
            self.downloads,
            self.download_rate
                .map_or(String::new(), |r| format!(" at {}/s", mb(Some(r as u64)))),
        );
        if let Some(clients) = self.clients {
            text += &format!("CLI clients: {clients}\n");
        }
        match self.trace {
            Some(trace) => {
                text += &format!(
                    "Trace bus: {} subscribers, {} events/s\n",
                    trace.subscribers,
                    trace
                        .events_per_sec
                        .map_or("?".to_string(), |r| format!("{r:.1}"))
                )
            }
            None => text += "Trace bus: off\n",
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_times_are_what_frames_took_not_the_gaps_between_them() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let frame = |ms: u64, took: Option<u64>| Frame {
            start: at(ms),
            took: took.map(Duration::from_millis),
        };
        // 30 quick frames at 60 fps, a pause, a slow one, and one still painting
        let mut frames: Vec<Frame> = (0..30).map(|i| frame(i * 16, Some(2))).collect();
        frames.push(frame(3000, Some(40)));
        frames.push(frame(3100, None));
        let stats = frame_stats(&frames, start, at(3200));
        assert!(
            (stats.avg_ms.unwrap() - 100.0 / 31.0).abs() < 0.01,
            "{stats:?}"
        );
        assert_eq!(stats.max_ms, Some(40.0));
        assert_eq!(stats.p95_ms, Some(2.0));
        assert!(!stats.idle);
        assert!((stats.fps - 32.0 / 3.2).abs() < 0.01, "{}", stats.fps);

        let idle = frame_stats(&frames, start, at(6000));
        assert!(idle.idle);
        let none = frame_stats(&[], start, at(1000));
        assert_eq!(none.fps, 0.0);
        assert_eq!(none.avg_ms, None);
    }

    #[test]
    fn frames_are_counted_only_while_recording() {
        frame(Instant::now(), true);
        assert!(frames().at.is_empty());
        {
            let _recording = Recording::start();
            let start = Instant::now();
            frame(start, false);
            assert_eq!(frames().at.len(), 1);
            assert_eq!(frames().active, Some(false));
            frame_painted(start + Duration::from_millis(5));
            frame_painted(start + Duration::from_millis(9));
            assert_eq!(
                frames().at.back().unwrap().took,
                Some(Duration::from_millis(5))
            );
        }
        assert!(frames().at.is_empty());
    }
}
