//! "Find it for me": finds the mod that crashes an instance by playing it with halves
//! of its mods turned off ([`launcher_core::content::mod_finder`]), one try after
//! another, until one mod is left.
//!
//! When the game crashed while it was starting, the tries run by themselves: a try
//! that crashes counts as a crash, and one that reaches the main menu and stays up for
//! [`SETTLE`] counts as working (the game is then stopped). When it crashed later, in
//! play, the player plays each try and says when it didn't crash; a crash is still
//! noticed by itself.
//!
//! The search is saved before any mod is renamed, so if the launcher quits half way,
//! the next start offers to finish it or put the mods back ([`ModFinder::resume`]).

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::content::{
    InstanceDirs,
    mod_finder::{self, Mods, Search, Step},
};

use super::{CrashInfo, Instances, InstancesEvent};
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots, RichText},
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, t, tr, trn},
    theme::{self, colors, px, radius, text, weights},
};

/// How long a game that reached the main menu has to stay up for a try to count as
/// working.
const SETTLE: Duration = Duration::from_secs(20);

/// Emitted when the dialog is done with: the shell closes it.
pub struct ModFinderClosed;

enum State {
    /// Reading the mods' jars.
    Reading,
    /// Before the first try (or after an interrupted search): waiting for Start.
    Ready,
    /// Renaming mods for the next try.
    Applying,
    /// A try is being played.
    Playing { loaded: bool },
    /// The game is being stopped; the try counts as `crashed` once it has exited.
    Stopping { crashed: bool },
    /// The game closed by itself in a try the player plays: did it crash?
    Asking,
    Found {
        mods: Vec<String>,
        /// The mods to keep off: the cause, and the mods that need it.
        keep_off: BTreeSet<String>,
    },
    /// Putting the mods back.
    Restoring,
    /// The search ended without a cause, with the mods put back.
    Over {
        message: SharedString,
        error: Option<SharedString>,
    },
}

/// What happens once the game of the try being stopped has exited.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AfterStop {
    /// Count the try.
    Record,
    /// The loader refused the mods: put them back and say so.
    Refused,
    /// The player stopped the search: put the mods back.
    Cancel,
}

pub struct ModFinder {
    instances: Entity<Instances>,
    id: String,
    name: String,
    dirs: InstanceDirs,
    search: Option<Search>,
    mods: Option<Arc<Mods>>,
    state: State,
    after_stop: AfterStop,
    /// Whether this finishes a search an earlier launcher started.
    resumed: bool,
    /// Started from Settings without a crash ([`ModFinder::by_hand`]).
    by_hand: bool,
    focus: FocusHandle,
    _events: Subscription,
    /// Redraws when the try's game moves on from preparing to running, which turns
    /// the answers on.
    _observe: Subscription,
    task: Option<Task<()>>,
    settle: Option<Task<()>>,
}

impl EventEmitter<ModFinderClosed> for ModFinder {}

impl Focusable for ModFinder {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ModFinder {
    /// A search for the mod behind `crash`, starting with the mods it points at.
    pub fn new(instances: Entity<Instances>, crash: &CrashInfo, cx: &mut Context<Self>) -> Self {
        let suspects: Vec<String> = crash.suspects.iter().map(|s| s.file_name.clone()).collect();
        let at_startup = crash.at_startup;
        let dirs = instances.read(cx).content_dirs(&crash.instance_id);
        let mut this = Self::empty(instances, &crash.instance_id, cx);
        this.read(
            move |dirs| {
                // A search left over from before is finished, not started over: it
                // knows which mods it turned off.
                mod_finder::load(dirs)
                    .map(|search| (search, true))
                    .unwrap_or_else(|| (Search::start(dirs, &suspects, at_startup), false))
            },
            dirs,
            cx,
        );
        this
    }

    /// A search started from the instance's Settings (Maintenance) without a crash:
    /// every mod is a suspect, and the person plays each try (a crash at startup ends
    /// a try too).
    pub fn by_hand(instances: Entity<Instances>, id: &str, cx: &mut Context<Self>) -> Self {
        let dirs = instances.read(cx).content_dirs(id);
        let mut this = Self::empty(instances, id, cx);
        this.by_hand = true;
        this.read(
            |dirs| {
                mod_finder::load(dirs)
                    .map(|search| (search, true))
                    .unwrap_or_else(|| (Search::start(dirs, &[], false), false))
            },
            dirs,
            cx,
        );
        this
    }

    /// Finishes (or undoes) a search that an earlier launcher left half way.
    pub fn resume(instances: Entity<Instances>, id: &str, cx: &mut Context<Self>) -> Self {
        let dirs = instances.read(cx).content_dirs(id);
        let mut this = Self::empty(instances, id, cx);
        this.resumed = true;
        this.read(
            |dirs| match mod_finder::load(dirs) {
                Some(search) => (search, true),
                None => (Search::new(Vec::new(), &[], true), true),
            },
            dirs,
            cx,
        );
        this
    }

    fn empty(instances: Entity<Instances>, id: &str, cx: &mut Context<Self>) -> Self {
        let name = instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let dirs = instances.read(cx).content_dirs(id);
        let events = cx.subscribe(&instances, |this: &mut Self, _, event, cx| {
            this.on_event(event, cx)
        });
        let observe = cx.observe(&instances, |_, _, cx| cx.notify());
        Self {
            instances,
            id: id.to_string(),
            name,
            dirs,
            search: None,
            mods: None,
            state: State::Reading,
            after_stop: AfterStop::Record,
            resumed: false,
            by_hand: false,
            focus: cx.focus_handle(),
            _events: events,
            _observe: observe,
            task: None,
            settle: None,
        }
    }

    /// The instance whose mods this is searching.
    pub fn instance_id(&self) -> &str {
        &self.id
    }

    /// A made-up search for the offscreen screenshots: before the first try, or
    /// (`found`) with Zenith found and its add-on kept off with it.
    pub fn preview(
        instances: Entity<Instances>,
        id: &str,
        found: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        use launcher_core::content::mod_finder::ModNode;
        let mut this = Self::empty(instances, id, cx);
        let names = [
            ("zenith.jar", "Zenith", vec!["zenith"], vec![]),
            (
                "zenith-addon.jar",
                "Zenith Attributes",
                vec![],
                vec!["zenith"],
            ),
            ("fancymenu.jar", "FancyMenu", vec![], vec![]),
            ("sodium.jar", "Sodium", vec![], vec![]),
        ];
        let mut files: Vec<String> = names.iter().map(|n| n.0.to_string()).collect();
        files.extend((0..208).map(|i| format!("mod{i:03}.jar")));
        let mut nodes: Vec<ModNode> = names
            .iter()
            .map(|(file, name, provides, requires)| ModNode {
                file_name: file.to_string(),
                name: name.to_string(),
                provides: provides.iter().map(|s| s.to_string()).collect(),
                requires: requires.iter().map(|s| s.to_string()).collect(),
            })
            .collect();
        nodes.extend(files[4..].iter().map(|f| ModNode {
            file_name: f.clone(),
            name: f.clone(),
            ..ModNode::default()
        }));
        let mods = Mods::new(nodes);
        let mut search = Search::new(files, &["fancymenu.jar".into()], true);
        if found {
            search.candidates = vec!["zenith.jar".into()];
            search.rounds = (0..8)
                .map(|i| mod_finder::Round {
                    off: Vec::new(),
                    crashed: i % 2 == 0,
                })
                .collect();
            let found = vec!["zenith.jar".to_string()];
            this.state = State::Found {
                keep_off: mods.closure(&found.iter().cloned().collect()),
                mods: found,
            };
        } else {
            this.state = State::Ready;
        }
        this.search = Some(search);
        this.mods = Some(Arc::new(mods));
        this
    }

    fn read(
        &mut self,
        start: impl FnOnce(&InstanceDirs) -> (Search, bool) + Send + 'static,
        dirs: InstanceDirs,
        cx: &mut Context<Self>,
    ) {
        let work = cx.background_spawn(async move {
            let (search, resumed) = start(&dirs);
            let mods = Mods::read(&dirs, &search.mods);
            (search, resumed, mods)
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let (search, resumed, mods) = work.await;
            this.update(cx, |this, cx| {
                this.resumed |= resumed;
                this.search = Some(search);
                this.mods = Some(Arc::new(mods));
                this.state = State::Ready;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Plans the next try and starts it, or ends the search.
    fn next(&mut self, cx: &mut Context<Self>) {
        let (Some(search), Some(mods)) = (self.search.as_mut(), self.mods.clone()) else {
            return;
        };
        if self.instances.read(cx).is_active(&self.id) {
            return;
        }
        match search.next(&mods) {
            Step::Test(off) => {
                launcher_core::trace::emit("mod_finder.try", || {
                    serde_json::json!({
                        "instance": self.id,
                        "try": search.rounds.len() + 1,
                        "off": off.len(),
                        "candidates": search.candidates.len(),
                    })
                });
                self.state = State::Applying;
                let mut search = search.clone();
                let dirs = self.dirs.clone();
                let work = cx.background_spawn(async move {
                    let result = mod_finder::apply(&dirs, &mut search, &off);
                    (search, result)
                });
                self.task = Some(cx.spawn(async move |this, cx| {
                    let (search, result) = work.await;
                    this.update(cx, |this, cx| {
                        this.search = Some(search);
                        match result {
                            Ok(()) => this.play(cx),
                            Err(err) => this.fail(err.to_string().into(), cx),
                        }
                    })
                    .ok();
                }));
            }
            Step::Found(found) => {
                launcher_core::trace::emit(
                    "mod_finder.found",
                    || serde_json::json!({ "instance": self.id, "mods": found, "tries": search.rounds.len() }),
                );
                let keep_off = mods.closure(&found.iter().cloned().collect());
                self.state = State::Found {
                    mods: found,
                    keep_off,
                };
            }
            Step::NotFound => {
                launcher_core::trace::emit(
                    "mod_finder.not_found",
                    || serde_json::json!({ "instance": self.id, "tries": search.rounds.len() }),
                );
                self.finish(BTreeSet::new(), Some(t(M::FinderNotFound).into()), cx);
            }
        }
        cx.notify();
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        self.state = State::Playing { loaded: false };
        self.after_stop = AfterStop::Record;
        let id = self.id.clone();
        self.instances
            .update(cx, |instances, cx| instances.play(&id, cx));
        if !self.instances.read(cx).is_active(&self.id) {
            // It refused to start (an update of the modpack, say); it says why once
            // this dialog has closed.
            self.fail(t(M::FinderFailed).into(), cx);
        }
        cx.notify();
    }

    /// Counts the try and goes on.
    fn record(&mut self, crashed: bool, cx: &mut Context<Self>) {
        self.settle = None;
        let Some(search) = self.search.as_mut() else {
            return;
        };
        search.record(crashed);
        if let Err(err) = mod_finder::save(&self.dirs, search) {
            self.fail(err.to_string().into(), cx);
            return;
        }
        self.next(cx);
    }

    /// Stops the game, then does `after`.
    fn stop_game(&mut self, crashed: bool, after: AfterStop, cx: &mut Context<Self>) {
        self.settle = None;
        self.after_stop = after;
        if self.instances.read(cx).is_active(&self.id) {
            self.state = State::Stopping { crashed };
            let id = self.id.clone();
            self.instances
                .update(cx, |instances, cx| instances.stop(&id, cx));
        } else {
            self.stopped(crashed, cx);
        }
        cx.notify();
    }

    /// The game of the try has exited.
    fn stopped(&mut self, crashed: bool, cx: &mut Context<Self>) {
        match self.after_stop {
            AfterStop::Record => self.record(crashed, cx),
            AfterStop::Refused => {
                self.finish(BTreeSet::new(), Some(t(M::FinderRefused).into()), cx)
            }
            AfterStop::Cancel => self.finish(BTreeSet::new(), Some(t(M::FinderStopped).into()), cx),
        }
    }

    fn on_event(&mut self, event: &InstancesEvent, cx: &mut Context<Self>) {
        match event {
            InstancesEvent::Crashed(crash) if crash.instance_id == self.id => match self.state {
                State::Playing { .. } if crash.mods_refused => {
                    self.after_stop = AfterStop::Refused;
                    self.stopped(true, cx);
                }
                State::Playing { .. } => self.record(true, cx),
                State::Stopping { crashed } => self.stopped(crashed || crash.mods_refused, cx),
                _ => {}
            },
            InstancesEvent::Exited { id, .. } if *id == self.id => match self.state {
                State::Stopping { crashed } => self.stopped(crashed, cx),
                State::Playing { loaded } => {
                    let automatic = self.search.as_ref().is_some_and(|s| s.at_startup);
                    if automatic && loaded {
                        // Closed after it loaded: it didn't crash at startup.
                        self.record(false, cx);
                    } else {
                        self.state = State::Asking;
                        cx.notify();
                    }
                }
                _ => {}
            },
            InstancesEvent::ReachedMenu(id) if *id == self.id => {
                if let State::Playing { loaded } = &mut self.state {
                    *loaded = true;
                    let automatic = self.search.as_ref().is_some_and(|s| s.at_startup);
                    if automatic {
                        self.settle = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(SETTLE).await;
                            this.update(cx, |this, cx| {
                                if matches!(this.state, State::Playing { loaded: true }) {
                                    this.stop_game(false, AfterStop::Record, cx);
                                }
                            })
                            .ok();
                        }));
                    }
                    cx.notify();
                }
            }
            InstancesEvent::ModsRefused(id) if *id == self.id => {
                if matches!(self.state, State::Playing { .. }) {
                    // Forge and NeoForge wait on an error screen: close it.
                    self.stop_game(true, AfterStop::Refused, cx);
                }
            }
            InstancesEvent::PlayFailed { id, message, .. } if *id == self.id => {
                match self.state {
                    // Stopped anyway: the try's start failing changes nothing.
                    State::Stopping { crashed } if self.after_stop == AfterStop::Cancel => {
                        self.stopped(crashed, cx)
                    }
                    State::Playing { .. } | State::Stopping { .. } => {
                        self.fail(message.clone(), cx)
                    }
                    _ => {}
                }
            }
            // The try was stopped while its game was still preparing: it never ran, and
            // no Exited or Crashed will come.
            InstancesEvent::PlayCancelled(id) if *id == self.id => match self.state {
                // Only the person stops a try that hasn't started (the answers wait
                // for the game): put the mods back.
                State::Stopping { crashed } if self.after_stop != AfterStop::Record => {
                    self.stopped(crashed, cx)
                }
                // Nothing ran to count: play the same try again.
                State::Stopping { .. } => self.play(cx),
                // Cancelled from outside the dialog (the control API): that stops the
                // search as Stop does.
                State::Playing { .. } => self.stop_game(false, AfterStop::Cancel, cx),
                _ => {}
            },
            _ => {}
        }
    }

    /// Ends the search with an error, putting the mods back.
    fn fail(&mut self, error: SharedString, cx: &mut Context<Self>) {
        launcher_core::trace::emit(
            "mod_finder.failed",
            || serde_json::json!({ "instance": self.id, "error": error.to_string() }),
        );
        self.finish_with(BTreeSet::new(), t(M::FinderFailed).into(), Some(error), cx);
    }

    /// Puts the mods back (keeping `keep_off` off), then says `message`, or closes.
    fn finish(
        &mut self,
        keep_off: BTreeSet<String>,
        message: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        match message {
            Some(message) => self.finish_with(keep_off, message, None, cx),
            None => self.restore(keep_off, None, cx),
        }
    }

    fn finish_with(
        &mut self,
        keep_off: BTreeSet<String>,
        message: SharedString,
        error: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.restore(keep_off, Some((message, error)), cx);
    }

    fn restore(
        &mut self,
        keep_off: BTreeSet<String>,
        then: Option<(SharedString, Option<SharedString>)>,
        cx: &mut Context<Self>,
    ) {
        self.settle = None;
        self.state = State::Restoring;
        cx.notify();
        let search = self.search.clone();
        let dirs = self.dirs.clone();
        let work = cx.background_spawn(async move {
            match &search {
                Some(search) => mod_finder::restore(&dirs, search, &keep_off),
                None => mod_finder::forget(&dirs),
            }
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match (result, then) {
                    (Err(err), then) => {
                        let message = then.map_or_else(|| t(M::FinderFailed).into(), |(m, _)| m);
                        this.state = State::Over {
                            message,
                            error: Some(tr!(M::FinderPutBackFailed, error = err).into()),
                        };
                    }
                    (Ok(()), Some((message, error))) => {
                        this.state = State::Over { message, error };
                    }
                    (Ok(()), None) => cx.emit(ModFinderClosed),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// The Stop button, or closing the dialog.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        match &self.state {
            State::Over { .. } => cx.emit(ModFinderClosed),
            State::Restoring => {}
            // Closing the dialog while the game stops ends the search once the game
            // has exited, and asks again in case the stop didn't take. A game that is
            // already gone ends it now: nothing else may say so.
            State::Stopping { crashed } => {
                let crashed = *crashed;
                if self.after_stop == AfterStop::Record {
                    self.after_stop = AfterStop::Cancel;
                }
                if self.instances.read(cx).is_active(&self.id) {
                    let id = self.id.clone();
                    self.instances
                        .update(cx, |instances, cx| instances.stop(&id, cx));
                } else {
                    self.stopped(crashed, cx);
                }
                cx.notify();
            }
            State::Playing { .. } => self.stop_game(false, AfterStop::Cancel, cx),
            // Nothing is off yet: there is nothing to put back.
            State::Reading => cx.emit(ModFinderClosed),
            State::Ready if !self.resumed => {
                let dirs = self.dirs.clone();
                cx.background_spawn(async move { mod_finder::forget(&dirs) })
                    .detach();
                cx.emit(ModFinderClosed);
            }
            _ => self.finish(BTreeSet::new(), None, cx),
        }
    }

    /// Whether "It crashed" and "It didn't crash" mean anything yet: not while the try's
    /// game is still being prepared, before it has run at all.
    fn can_answer(&self, cx: &App) -> bool {
        match self.state {
            State::Playing { .. } => self.instances.read(cx).is_running(&self.id),
            State::Asking => true,
            _ => false,
        }
    }

    fn answer(&mut self, crashed: bool, cx: &mut Context<Self>) {
        if !self.can_answer(cx) {
            return;
        }
        match self.state {
            State::Playing { .. } => self.stop_game(crashed, AfterStop::Record, cx),
            State::Asking => self.record(crashed, cx),
            _ => {}
        }
    }
}

/// A muted line under a heading.
fn note(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .text_color(crate::theme::ink::secondary())
        .child(RichText::new(text.into()))
}

fn waiting(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_color(crate::theme::ink::muted())
        .child(LoadingDots)
        .child(text.into())
}

impl Render for ModFinder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let search = self.search.as_ref();
        let mods = self.mods.clone();
        let name_of = |file: &str| {
            mods.as_ref()
                .map_or_else(|| file.to_string(), |mods| mods.name(file))
        };
        let progress = search.map(|search| {
            let tries = search.rounds.len() + usize::from(search.testing.is_some());
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_x_3()
                .text_size(theme::text_size(text::SMALL))
                .text_color(crate::theme::ink::muted())
                .when(tries > 0, |this| {
                    this.child(tr!(M::FinderTry, number = tries))
                })
                .child(trn!(P::FinderModsLeft, search.candidates.len()))
                .when_some(search.testing.as_ref(), |this, off| {
                    this.child(trn!(P::FinderModsOff, off.len()))
                })
                .when(search.tries_left() > 0, |this| {
                    this.child(trn!(P::FinderTriesLeft, search.tries_left()))
                })
        });
        let automatic = search.is_some_and(|s| s.at_startup);

        let body = match &self.state {
            State::Reading => waiting(t(M::FinderReading)).into_any_element(),
            State::Ready => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(if self.resumed {
                    tr!(M::FinderInterrupted, name = self.name)
                } else {
                    t(M::FinderIntro).to_string()
                }))
                .child(note(t(if automatic {
                    M::FinderAutomatic
                } else if self.by_hand {
                    M::FinderByHand
                } else {
                    M::FinderManual
                })))
                .children(progress)
                .into_any_element(),
            State::Applying => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(waiting(t(M::FinderStarting)))
                .children(progress)
                .into_any_element(),
            State::Playing { loaded } => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(if automatic {
                    waiting(t(if *loaded {
                        M::FinderSettling
                    } else {
                        M::FinderWaiting
                    }))
                    .into_any_element()
                } else {
                    note(t(M::FinderPlay)).into_any_element()
                })
                .children(progress)
                .into_any_element(),
            State::Stopping { .. } => waiting(t(M::FinderStopping)).into_any_element(),
            State::Asking => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(note(t(M::FinderAsk)))
                .children(progress)
                .into_any_element(),
            State::Found {
                mods: found,
                keep_off,
            } => {
                let names: Vec<String> = found.iter().map(|f| name_of(f)).collect();
                let others: Vec<String> = keep_off
                    .iter()
                    .filter(|f| !found.contains(*f))
                    .map(|f| name_of(f))
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .rounded(radius::LG)
                            .border_1()
                            .border_color(colors::FADE_700)
                            .bg(colors::GRAY_800)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(crate::theme::ink::muted())
                                    .child(t(M::FinderFound).to_uppercase()),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_size(text::H2))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(names.join(", ")),
                            ),
                    )
                    .child(note(if names.len() == 1 {
                        tr!(M::FinderFoundOne, name = names[0])
                    } else {
                        tr!(M::FinderFoundGroup, names = names.join(", "))
                    }))
                    .when(!others.is_empty(), |this| {
                        this.child(note(tr!(M::FinderAlsoOff, names = others.join(", "))))
                    })
                    .into_any_element()
            }
            State::Restoring => waiting(t(M::FinderStopping)).into_any_element(),
            State::Over { message, error } => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(note(message.clone()))
                .when_some(error.clone(), |this, error| {
                    this.child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::RED_200)
                            .child(RichText::new(error)),
                    )
                })
                .into_any_element(),
        };

        let mut dialog = Dialog::new(
            "mod-finder",
            tr!(M::FinderTitle, name = self.name),
            self.focus.clone(),
        )
        .width(px(560.))
        .on_close({
            let entity = entity.clone();
            move |_, cx| entity.update(cx, |this, cx| this.cancel(cx))
        })
        .child(body);

        dialog =
            match &self.state {
                State::Ready => dialog
                    .footer(
                        Button::new(
                            "finder-start",
                            if self.resumed {
                                t(CommonMsg::Continue)
                            } else {
                                t(M::FinderStart)
                            },
                        )
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
                    )
                    .footer(
                        Button::new(
                            "finder-cancel",
                            if self.resumed {
                                t(M::FinderPutBack)
                            } else {
                                t(CommonMsg::Cancel)
                            },
                        )
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    ),
                State::Playing { .. } | State::Asking => dialog
                    .footer(
                        Button::new("finder-no-crash", t(M::FinderDidntCrash))
                            .variant(ButtonVariant::Primary)
                            .disabled(!self.can_answer(cx))
                            .on_click(cx.listener(|this, _, _, cx| this.answer(false, cx))),
                    )
                    .footer(
                        Button::new("finder-crashed", t(M::FinderItCrashed))
                            .variant(ButtonVariant::Secondary)
                            .disabled(!self.can_answer(cx))
                            .on_click(cx.listener(|this, _, _, cx| this.answer(true, cx))),
                    )
                    .footer_start(
                        Button::new("finder-stop", t(M::FinderStop))
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    ),
                State::Found { keep_off, .. } => {
                    let keep_off = keep_off.clone();
                    dialog
                        .footer(
                            Button::new("finder-keep-off", t(M::FinderKeepOff))
                                .variant(ButtonVariant::Primary)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.finish(keep_off.clone(), None, cx)
                                })),
                        )
                        .footer(
                            Button::new("finder-put-back", t(M::FinderPutBack))
                                .variant(ButtonVariant::Secondary)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.finish(BTreeSet::new(), None, cx)
                                })),
                        )
                }
                State::Over { .. } => dialog.footer(
                    Button::new("finder-close", t(CommonMsg::Close))
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ModFinderClosed))),
                ),
                State::Applying | State::Reading => dialog.footer(
                    Button::new("finder-stop", t(M::FinderStop))
                        .variant(ButtonVariant::Ghost)
                        .disabled(matches!(self.state, State::Applying))
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                ),
                State::Stopping { .. } | State::Restoring => dialog,
            };
        dialog
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::{Entity, TestAppContext};

    use super::*;

    /// A search half way through a try of a sample instance (signed out, in a
    /// temporary data folder, so nothing is downloaded or started).
    fn searching(cx: &mut TestAppContext) -> (Entity<Instances>, Entity<ModFinder>, String) {
        cx.update(crate::components::init);
        let (instances, finder, id) = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let instances = crate::preview::instances("sidebar-groups", &accounts, cx);
            let id = instances.read(cx).list()[0].id.clone();
            let finder = cx.new(|cx| {
                let mut finder = ModFinder::preview(instances.clone(), &id, false, cx);
                // Nothing was renamed, so putting the mods back only forgets the search.
                if let Some(search) = finder.search.as_mut() {
                    search.turned_off.clear();
                }
                finder
            });
            (instances, finder, id)
        });
        cx.run_until_parked();
        (instances, finder, id)
    }

    fn set_state(
        finder: &Entity<ModFinder>,
        state: State,
        after: AfterStop,
        cx: &mut TestAppContext,
    ) {
        finder.update(cx, |finder, _| {
            finder.state = state;
            finder.after_stop = after;
        });
    }

    fn stopping(finder: &Entity<ModFinder>, cx: &mut TestAppContext) -> bool {
        finder.read_with(cx, |finder, _| {
            matches!(finder.state, State::Stopping { .. })
        })
    }

    fn over(finder: &Entity<ModFinder>, cx: &mut TestAppContext) -> Option<String> {
        finder.read_with(cx, |finder, _| match &finder.state {
            State::Over { message, .. } => Some(message.to_string()),
            _ => None,
        })
    }

    /// Stop pressed while the try's game was still preparing: the preparation ends
    /// cancelled, with no Exited or Crashed, and the search ends instead of waiting
    /// on "Stopping the game" for good (LS-56).
    #[test]
    fn a_try_stopped_while_preparing_ends_the_search() {
        let mut cx = TestAppContext::single();
        let (instances, finder, id) = searching(&mut cx);
        set_state(
            &finder,
            State::Stopping { crashed: false },
            AfterStop::Cancel,
            &mut cx,
        );
        instances.update(&mut cx, |_, cx| {
            cx.emit(InstancesEvent::PlayCancelled(id.clone()))
        });
        cx.run_until_parked();
        assert!(!stopping(&finder, &mut cx));
        assert_eq!(over(&finder, &mut cx).as_deref(), Some(t(M::FinderStopped)));
    }

    /// Signing in failing after Stop ends the search too (LS-56).
    #[test]
    fn a_failed_start_after_stop_ends_the_search() {
        let mut cx = TestAppContext::single();
        let (instances, finder, id) = searching(&mut cx);
        set_state(
            &finder,
            State::Stopping { crashed: false },
            AfterStop::Record,
            &mut cx,
        );
        // Another job's failure isn't the try's.
        instances.update(&mut cx, |_, cx| {
            cx.emit(InstancesEvent::PlayFailed {
                id: "another".into(),
                title: "Couldn't sign in".into(),
                message: "offline".into(),
            })
        });
        cx.run_until_parked();
        assert!(stopping(&finder, &mut cx));
        instances.update(&mut cx, |_, cx| {
            cx.emit(InstancesEvent::PlayFailed {
                id: id.clone(),
                title: "Couldn't sign in".into(),
                message: "offline".into(),
            })
        });
        cx.run_until_parked();
        assert_eq!(over(&finder, &mut cx).as_deref(), Some(t(M::FinderFailed)));
    }

    /// Closing the dialog while it says "Stopping the game" with no game left ends the
    /// search: nothing else would (LS-56).
    #[test]
    fn closing_while_stopping_ends_the_search() {
        let mut cx = TestAppContext::single();
        let (_instances, finder, _id) = searching(&mut cx);
        set_state(
            &finder,
            State::Stopping { crashed: true },
            AfterStop::Record,
            &mut cx,
        );
        finder.update(&mut cx, |finder, cx| finder.cancel(cx));
        cx.run_until_parked();
        assert_eq!(over(&finder, &mut cx).as_deref(), Some(t(M::FinderStopped)));
    }

    /// The answers wait for the try's game to run: an answer while it is still being
    /// prepared would count a try that never ran (LS-56).
    #[test]
    fn answers_wait_for_the_game() {
        let mut cx = TestAppContext::single();
        let (_instances, finder, _id) = searching(&mut cx);
        set_state(
            &finder,
            State::Playing { loaded: false },
            AfterStop::Record,
            &mut cx,
        );
        let answerable = finder.read_with(&cx, |finder, cx| finder.can_answer(cx));
        assert!(!answerable, "the game isn't running");
        finder.update(&mut cx, |finder, cx| finder.answer(true, cx));
        assert!(!stopping(&finder, &mut cx), "ignored");
        set_state(&finder, State::Asking, AfterStop::Record, &mut cx);
        let answerable = finder.read_with(&cx, |finder, cx| finder.can_answer(cx));
        assert!(answerable, "the game closed by itself");
    }
}
