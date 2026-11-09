//! Long operations: installs, imports, backups, downloads. Each runs as a job with an
//! id, so a client can wait for it (with progress lines), or come back to it later
//! with `jobs.get` / `jobs.wait`, or cancel it.

use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::{App, Task};
use launcher_control::{ErrorObject, Message, codes};
use launcher_core::{
    content::install::InstallProgress,
    game::download::{CancelToken, Progress as Downloads, ProgressSnapshot},
    import::ImportProgress,
    instances::Instance,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiResult, Call, Control, Reply, api_error, ready};
use crate::instances::Instances;

/// Finished jobs kept for `jobs.get`.
const KEEP_FINISHED: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    fn name(self) -> &'static str {
        match self {
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
}

/// Where a job's progress is read from.
#[derive(Clone)]
pub enum Progress {
    None,
    Install(Arc<InstallProgress>),
    Files(Arc<Downloads>),
    /// The import running now, of several in a row.
    Import(Arc<Mutex<Option<Arc<ImportProgress>>>>),
    /// The instance's run (installing the game, verifying).
    Run(String),
}

/// How a job is cancelled.
#[derive(Clone)]
pub enum Cancel {
    None,
    Token(CancelToken),
    /// Stopping the instance's run (`Instances::stop`).
    Instance(String),
    /// A modpack import of the window's (`Instances::cancel_import`).
    Import(u64),
}

/// What a new job is.
pub struct JobSpec {
    pub kind: &'static str,
    pub label: String,
    pub instance: Option<Instance>,
    pub progress: Progress,
    pub cancel: Cancel,
    /// Text the work updates as it goes ("Signing in…").
    pub stage: Option<Arc<Mutex<String>>>,
}

impl JobSpec {
    pub fn new(kind: &'static str, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            instance: None,
            progress: Progress::None,
            cancel: Cancel::None,
            stage: None,
        }
    }

    pub fn instance(mut self, instance: &Instance) -> Self {
        self.instance = Some(instance.clone());
        self
    }

    pub fn progress(mut self, progress: Progress) -> Self {
        self.progress = progress;
        self
    }

    pub fn cancel(mut self, cancel: Cancel) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn stage(mut self, stage: Arc<Mutex<String>>) -> Self {
        self.stage = Some(stage);
        self
    }
}

struct Job {
    id: u64,
    spec: JobSpec,
    state: JobState,
    result: Option<Value>,
    error: Option<ErrorObject>,
    started: u64,
    finished: Option<u64>,
    _task: Option<Task<()>>,
}

#[derive(Default)]
pub struct Jobs {
    next: u64,
    list: Vec<Job>,
}

fn now() -> u64 {
    launcher_core::instances::now()
}

fn snapshot_json(snapshot: ProgressSnapshot) -> Value {
    let fraction = if snapshot.bytes_total > 0 {
        Some(snapshot.bytes_done as f64 / snapshot.bytes_total as f64)
    } else if snapshot.files_total > 0 {
        Some(snapshot.files_done as f64 / snapshot.files_total as f64)
    } else {
        None
    };
    json!({
        "files_done": snapshot.files_done,
        "files_total": snapshot.files_total,
        "bytes_done": snapshot.bytes_done,
        "bytes_total": snapshot.bytes_total,
        "fraction": fraction.map(|f| (f.clamp(0.0, 1.0) * 1000.0).round() / 1000.0),
    })
}

/// The progress of a stage and its downloads, as the API reports it.
pub fn progress_json(label: &str, snapshot: ProgressSnapshot) -> Value {
    let mut value = snapshot_json(snapshot);
    value["label"] = json!(label);
    value
}

impl Jobs {
    fn job_json(&self, job: &Job, instances: &Instances) -> Value {
        let progress = match &job.spec.progress {
            Progress::None => Value::Null,
            Progress::Install(progress) => {
                progress_json(&progress.label(), progress.downloads.snapshot())
            }
            Progress::Files(progress) => snapshot_json(progress.snapshot()),
            Progress::Import(slot) => match slot.lock().ok().and_then(|slot| slot.clone()) {
                Some(progress) => json!({
                    "label": format!("{:?}", progress.stage()).to_lowercase(),
                    "fraction": progress.fraction(),
                }),
                None => Value::Null,
            },
            Progress::Run(id) => match instances.run_status(id).progress {
                Some((stage, snapshot)) => progress_json(stage.label(), snapshot),
                None => Value::Null,
            },
        };
        let stage = job
            .spec
            .stage
            .as_ref()
            .and_then(|stage| stage.lock().ok().map(|s| s.clone()))
            .filter(|s| !s.is_empty());
        json!({
            "id": job.id,
            "kind": job.spec.kind,
            "label": job.spec.label,
            "instance": job.spec.instance.as_ref().map(|i| json!({ "id": i.id, "name": i.name })),
            "state": job.state.name(),
            "stage": stage,
            "progress": if job.state == JobState::Running { progress } else { Value::Null },
            "started": job.started,
            "finished": job.finished,
            "result": job.result,
            "error": job.error,
        })
    }

    /// Every job, or only those still running.
    pub fn to_json_with(&self, running_only: bool, instances: &Instances) -> Value {
        Value::Array(
            self.list
                .iter()
                .filter(|j| !running_only || j.state == JobState::Running)
                .map(|j| self.job_json(j, instances))
                .collect(),
        )
    }

    /// Every job, or only those still running, without the run progress (which needs
    /// the app).
    pub fn to_json(&self, running_only: bool) -> Value {
        Value::Array(
            self.list
                .iter()
                .filter(|j| !running_only || j.state == JobState::Running)
                .map(|j| {
                    json!({
                        "id": j.id,
                        "kind": j.spec.kind,
                        "label": j.spec.label,
                        "state": j.state.name(),
                        "instance": j.spec.instance.as_ref().map(|i| &i.id),
                    })
                })
                .collect(),
        )
    }

    pub fn get_json(&self, id: u64, instances: &Instances) -> Option<Value> {
        self.list
            .iter()
            .find(|j| j.id == id)
            .map(|j| self.job_json(j, instances))
    }

    /// The job's outcome once it ended.
    fn outcome(&self, id: u64) -> Option<Option<ApiResult>> {
        let job = self.list.iter().find(|j| j.id == id)?;
        Some(match job.state {
            JobState::Running => None,
            JobState::Succeeded => {
                let mut result = job.result.clone().unwrap_or(json!({}));
                if let Value::Object(object) = &mut result {
                    object.insert("job".into(), json!(id));
                }
                Some(Ok(result))
            }
            JobState::Failed | JobState::Cancelled => {
                let mut error = job
                    .error
                    .clone()
                    .unwrap_or_else(|| api_error(codes::FAILED, "The job failed."));
                let mut data = error.data.take().unwrap_or(json!({}));
                if let Value::Object(object) = &mut data {
                    object.insert("job".into(), json!(id));
                }
                error.data = Some(data);
                Some(Err(error))
            }
        })
    }

    /// Whether a job of `kind` runs for `instance`.
    pub fn running_for(&self, kind: &str, instance: &str) -> Option<u64> {
        self.list
            .iter()
            .find(|j| {
                j.state == JobState::Running
                    && j.spec.kind == kind
                    && j.spec.instance.as_ref().is_some_and(|i| i.id == instance)
            })
            .map(|j| j.id)
    }

    /// Jobs whose state changed since `seen`, for subscriptions: `(id, state)`.
    pub fn states(&self) -> Vec<(u64, JobState)> {
        self.list.iter().map(|j| (j.id, j.state)).collect()
    }

    fn finish(&mut self, id: u64, result: ApiResult) {
        launcher_core::trace::emit("job.finished", || match &result {
            Ok(_) => json!({ "job": id, "state": "succeeded" }),
            Err(error) => {
                json!({ "job": id, "state": if error.code == codes::CANCELLED { "cancelled" } else { "failed" }, "error": { "code": error.code, "message": error.message } })
            }
        });
        if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
            job.finished = Some(now());
            job._task = None;
            match result {
                Ok(value) => {
                    job.state = JobState::Succeeded;
                    job.result = Some(value);
                }
                Err(error) => {
                    job.state = if error.code == codes::CANCELLED {
                        JobState::Cancelled
                    } else {
                        JobState::Failed
                    };
                    job.error = Some(error);
                }
            }
        }
        let finished = self
            .list
            .iter()
            .filter(|j| j.state != JobState::Running)
            .count();
        if finished > KEEP_FINISHED {
            let mut drop = finished - KEEP_FINISHED;
            self.list.retain(|j| {
                if drop > 0 && j.state != JobState::Running {
                    drop -= 1;
                    false
                } else {
                    true
                }
            });
        }
    }
}

impl Control {
    /// Runs `work` as a job and returns its id.
    pub fn start_job(self: &Rc<Self>, spec: JobSpec, work: Task<ApiResult>, cx: &mut App) -> u64 {
        let id = {
            let mut jobs = self.jobs.borrow_mut();
            jobs.next += 1;
            let id = jobs.next;
            launcher_core::trace::emit("job.started", || {
                json!({
                    "job": id,
                    "kind": spec.kind,
                    "label": spec.label,
                    "instance": spec.instance.as_ref().map(|i| i.id.clone()),
                })
            });
            jobs.list.push(Job {
                id,
                spec,
                state: JobState::Running,
                result: None,
                error: None,
                started: now(),
                finished: None,
                _task: None,
            });
            id
        };
        let control = Rc::downgrade(self);
        let task = cx.spawn(async move |_| {
            let result = work.await;
            if let Some(control) = control.upgrade() {
                control.jobs.borrow_mut().finish(id, result);
            }
        });
        if let Some(job) = self.jobs.borrow_mut().list.iter_mut().find(|j| j.id == id)
            && job.state == JobState::Running
        {
            job._task = Some(task);
        }
        id
    }

    /// Answers a call that started job `id`: at once with the job, or (`"wait": true`)
    /// when it ends, with progress lines until then.
    pub fn reply_job(self: &Rc<Self>, call: &Call, id: u64, cx: &mut App) -> Reply {
        if !call.wait() {
            let job = self.jobs.borrow().get_json(id, self.instances.read(cx));
            return ready(Ok(json!({ "job": job })));
        }
        self.wait_job(call, id, cx)
    }

    fn wait_job(self: &Rc<Self>, call: &Call, id: u64, cx: &mut App) -> Reply {
        let control = self.clone();
        let outbox = call.outbox.clone();
        let request = call.id.clone();
        cx.spawn(async move |cx| {
            let mut last = Value::Null;
            loop {
                let (outcome, json) = cx.update(|cx| {
                    let jobs = control.jobs.borrow();
                    (
                        jobs.outcome(id),
                        jobs.get_json(id, control.instances.read(cx)),
                    )
                });
                match outcome {
                    None => {
                        return Err(api_error(codes::NOT_FOUND, format!("No job {id}.")));
                    }
                    Some(Some(result)) => return result,
                    Some(None) => {}
                }
                let json = json.unwrap_or_default();
                if json != last {
                    outbox.send(&Message::Progress {
                        id: request.clone(),
                        progress: json.clone(),
                    });
                    last = json;
                }
                if outbox.is_closed() {
                    return Err(api_error(
                        codes::CANCELLED,
                        "The client went away; the job goes on.",
                    ));
                }
                cx.background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
            }
        })
    }

    fn cancel_job(&self, id: u64, cx: &mut App) -> Result<(), ErrorObject> {
        let cancel = {
            let jobs = self.jobs.borrow();
            let job = jobs
                .list
                .iter()
                .find(|j| j.id == id)
                .ok_or_else(|| api_error(codes::NOT_FOUND, format!("No job {id}.")))?;
            if job.state != JobState::Running {
                return Err(api_error(codes::CONFLICT, "The job has ended."));
            }
            job.spec.cancel.clone()
        };
        match cancel {
            Cancel::None => {
                return Err(api_error(
                    codes::UNAVAILABLE,
                    "This job can't be cancelled.",
                ));
            }
            Cancel::Token(token) => token.cancel(),
            Cancel::Instance(instance) => self
                .instances
                .update(cx, |instances, cx| instances.stop(&instance, cx)),
            Cancel::Import(import) => self
                .instances
                .update(cx, |instances, cx| instances.cancel_import(import, cx)),
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct JobParams {
    job: u64,
}

pub fn list(call: Call, cx: &mut App) -> Reply {
    let running = call.params.get("running") == Some(&Value::Bool(true));
    let jobs = call
        .control
        .jobs
        .borrow()
        .to_json_with(running, call.control.instances.read(cx));
    ready(Ok(json!({ "jobs": jobs })))
}

pub fn get(call: Call, cx: &mut App) -> Reply {
    let params: JobParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let job = call
        .control
        .jobs
        .borrow()
        .get_json(params.job, call.control.instances.read(cx));
    match job {
        Some(job) => ready(Ok(json!({ "job": job }))),
        None => ready(Err(api_error(
            codes::NOT_FOUND,
            format!(
                "No job {}. Finished jobs are kept for a while only.",
                params.job
            ),
        ))),
    }
}

pub fn wait(call: Call, cx: &mut App) -> Reply {
    let params: JobParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let control = call.control.clone();
    control.wait_job(&call, params.job, cx)
}

pub fn cancel(call: Call, cx: &mut App) -> Reply {
    let params: JobParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    ready(
        call.control
            .cancel_job(params.job, cx)
            .map(|()| json!({ "job": params.job, "cancelling": true })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u64, state: JobState) -> Job {
        Job {
            id,
            spec: JobSpec::new("test", "Test"),
            state,
            result: None,
            error: None,
            started: 0,
            finished: None,
            _task: None,
        }
    }

    #[test]
    fn outcomes_carry_the_job_id() {
        let mut jobs = Jobs::default();
        jobs.list.push(job(1, JobState::Running));
        jobs.list.push(job(2, JobState::Running));
        assert_eq!(jobs.outcome(1), Some(None));
        jobs.finish(1, Ok(json!({ "installed": 3 })));
        assert_eq!(
            jobs.outcome(1),
            Some(Some(Ok(json!({ "installed": 3, "job": 1 }))))
        );
        jobs.finish(2, Err(api_error(codes::CANCELLED, "Cancelled")));
        let error = jobs.outcome(2).unwrap().unwrap().unwrap_err();
        assert_eq!(error.code, codes::CANCELLED);
        assert_eq!(error.data.unwrap()["job"], 2);
        assert_eq!(jobs.list[1].state, JobState::Cancelled);
        assert_eq!(jobs.outcome(3), None);
    }

    #[test]
    fn old_finished_jobs_are_dropped() {
        let mut jobs = Jobs::default();
        jobs.list.push(job(0, JobState::Running));
        for id in 1..=(KEEP_FINISHED as u64 + 10) {
            jobs.list.push(job(id, JobState::Running));
            jobs.finish(id, Ok(json!({})));
        }
        assert_eq!(jobs.list.len(), KEEP_FINISHED + 1);
        assert_eq!(jobs.list[0].id, 0, "running jobs stay");
        assert_eq!(jobs.list[1].id, 11);
    }

    #[test]
    fn progress_has_a_fraction() {
        let value = progress_json(
            "Downloading",
            ProgressSnapshot {
                files_total: 4,
                files_done: 1,
                bytes_total: 0,
                bytes_done: 0,
            },
        );
        assert_eq!(value["fraction"], json!(0.25));
        assert_eq!(value["label"], "Downloading");
    }
}
