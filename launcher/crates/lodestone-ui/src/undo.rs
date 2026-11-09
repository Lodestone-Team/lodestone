//! Undo and redo for settings: Ctrl+Z (⌘Z) and Shift+Ctrl+Z (⇧⌘Z), and the Edit menu.
//!
//! The model:
//! - The launcher keeps one history of the last [`LIMIT`] setting changes, across
//!   every page, in memory only.
//! - A setting that saves as soon as it changes (Language, Zoom, a switch, a shared
//!   game setting, an instance's icon, …) is one step. Undoing it puts the old value
//!   back and saves it.
//! - In a form that is saved with Save (Instance defaults, an instance's Settings tab),
//!   each field's edit is one step while it is unsaved: typing in one field, then in
//!   another, makes two steps. Save turns the form's unsaved steps into a single step
//!   for what it saved; Discard drops them.
//! - While a text field has the focus, Undo and Redo change its text instead (see
//!   `TextInput`), so a setting never changes under the cursor.
//! - A step whose setting is gone (a deleted instance) is skipped. A step that can't
//!   be undone now (the instance's game is running) stays, and the toast says why.
//!
//! A step holds two closures that put the setting back to how it was before or after
//! the change, and two that describe it then ("Language → Français").

use std::{collections::VecDeque, path::PathBuf, rc::Rc};

use gpui::{App, EntityId, Global};

/// How many steps are kept.
pub const LIMIT: usize = 15;

/// What a step changed, to merge a field's consecutive edits into one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKey {
    /// A change that is saved.
    Saved,
    /// Unsaved edits of `fields` (a bit per field) in the form `view`.
    Form { view: EntityId, fields: u64 },
}

/// Why a step wasn't undone or redone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoError {
    /// What it changed no longer exists; the step is dropped.
    Gone,
    /// It can't be changed now, for the reason given; the step stays.
    Refused(String),
}

type Apply = Rc<dyn Fn(&mut App) -> Result<(), UndoError>>;
type Describe = Rc<dyn Fn() -> String>;

#[derive(Clone)]
pub struct Step {
    pub key: StepKey,
    before: Describe,
    after: Describe,
    undo: Apply,
    redo: Apply,
}

impl Step {
    /// A step: `undo` and `redo` put the setting as it was before and after the change,
    /// which `before` and `after` describe.
    pub fn new(
        key: StepKey,
        before: impl Fn() -> String + 'static,
        after: impl Fn() -> String + 'static,
        undo: impl Fn(&mut App) -> Result<(), UndoError> + 'static,
        redo: impl Fn(&mut App) -> Result<(), UndoError> + 'static,
    ) -> Self {
        Self {
            key,
            before: Rc::new(before),
            after: Rc::new(after),
            undo: Rc::new(undo),
            redo: Rc::new(redo),
        }
    }
}

/// The steps that can be undone, oldest first, and those that were undone.
#[derive(Default)]
pub struct History {
    done: VecDeque<Step>,
    undone: Vec<Step>,
    /// A step is being undone or redone: what it changes isn't a new step.
    replaying: bool,
}

impl Global for History {}

impl History {
    /// Adds a step: a form's next edit of the same fields is merged into its last
    /// step. A new step forgets what was undone.
    pub fn push(&mut self, step: Step) {
        if self.replaying {
            return;
        }
        self.undone.clear();
        if let StepKey::Form { .. } = step.key
            && let Some(last) = self.done.back_mut()
            && last.key == step.key
        {
            last.after = step.after;
            last.redo = step.redo;
            return;
        }
        self.done.push_back(step);
        while self.done.len() > LIMIT {
            self.done.pop_front();
        }
    }

    /// The key of the step Undo would undo next.
    pub fn last_key(&self) -> Option<StepKey> {
        self.done.back().map(|step| step.key)
    }

    /// Removes the last step if it has `key`: a form's edit undone by hand.
    pub fn pop_if(&mut self, key: StepKey) {
        if self.last_key() == Some(key) {
            self.done.pop_back();
        }
    }

    /// Drops the unsaved steps of the form `view`: it was saved or discarded.
    pub fn forget_form(&mut self, view: EntityId) {
        let other = |step: &Step| !matches!(step.key, StepKey::Form { view: v, .. } if v == view);
        self.done.retain(other);
        self.undone.retain(other);
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn is_replaying(&self) -> bool {
        self.replaying
    }
}

/// What Undo or Redo did, for the toast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Undid a step; the setting is now as described.
    Undid(String),
    Redid(String),
    Refused(String),
    /// Nothing to undo or redo.
    Nothing,
}

/// Records a step (unless a step is being undone or redone).
pub fn record(step: Step, cx: &mut App) {
    cx.default_global::<History>().push(step);
}

/// Whether a step is being undone or redone, so a change isn't a new step.
pub fn replaying(cx: &App) -> bool {
    cx.try_global::<History>()
        .is_some_and(History::is_replaying)
}

pub fn last_key(cx: &App) -> Option<StepKey> {
    cx.try_global::<History>().and_then(History::last_key)
}

pub fn pop_if(key: StepKey, cx: &mut App) {
    cx.default_global::<History>().pop_if(key);
}

pub fn forget_form(view: EntityId, cx: &mut App) {
    cx.default_global::<History>().forget_form(view);
}

/// Undoes the last step that can be.
pub fn undo(cx: &mut App) -> Outcome {
    step(cx, true)
}

/// Redoes the last undone step.
pub fn redo(cx: &mut App) -> Outcome {
    step(cx, false)
}

fn step(cx: &mut App, undo: bool) -> Outcome {
    loop {
        let history = cx.default_global::<History>();
        let next = if undo {
            history.done.pop_back()
        } else {
            history.undone.pop()
        };
        let Some(step) = next else {
            return Outcome::Nothing;
        };
        history.replaying = true;
        let result = if undo {
            (step.undo)(cx)
        } else {
            (step.redo)(cx)
        };
        let history = cx.default_global::<History>();
        history.replaying = false;
        match result {
            Ok(()) => {
                return if undo {
                    let text = (step.before)();
                    history.undone.push(step);
                    Outcome::Undid(text)
                } else {
                    let text = (step.after)();
                    history.done.push_back(step);
                    Outcome::Redid(text)
                };
            }
            // what it changed is gone: try the step before
            Err(UndoError::Gone) => continue,
            Err(UndoError::Refused(reason)) => {
                if undo {
                    history.done.push_back(step);
                } else {
                    history.undone.push(step);
                }
                return Outcome::Refused(reason);
            }
        }
    }
}

/// Files as they are now: their bytes, or `None` for a file that doesn't exist. A
/// setting kept in files (the shared game settings) is undone by writing them back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files(Vec<(PathBuf, Option<Vec<u8>>)>);

impl Files {
    pub fn read(paths: &[PathBuf]) -> Self {
        Self(
            paths
                .iter()
                .map(|path| (path.clone(), std::fs::read(path).ok()))
                .collect(),
        )
    }

    /// Puts the files back as they were read.
    pub fn write(&self) -> std::io::Result<()> {
        for (path, bytes) in &self.0 {
            match bytes {
                Some(bytes) => launcher_core::storage::write_atomic(path, bytes)?,
                None => match std::fs::remove_file(path) {
                    Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
                    _ => {}
                },
            }
        }
        Ok(())
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.0.iter().map(|(path, _)| path.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn step(key: StepKey, value: Rc<Cell<i32>>, before: i32, after: i32) -> Step {
        let (undo_value, redo_value) = (value.clone(), value);
        Step::new(
            key,
            move || before.to_string(),
            move || after.to_string(),
            move |_| {
                undo_value.set(before);
                Ok(())
            },
            move |_| {
                redo_value.set(after);
                Ok(())
            },
        )
    }

    #[test]
    fn keeps_the_last_fifteen_steps() {
        let mut history = History::default();
        let value = Rc::new(Cell::new(0));
        for i in 0..20 {
            history.push(step(StepKey::Saved, value.clone(), i, i + 1));
        }
        assert_eq!(history.done.len(), LIMIT);
        assert_eq!((history.done.front().unwrap().before)(), "5");
    }

    #[cfg(feature = "screenshot")]
    #[gpui::test]
    fn undo_and_redo_walk_the_history(cx: &mut gpui::TestAppContext) {
        let value = Rc::new(Cell::new(2));
        cx.update(|cx| {
            record(step(StepKey::Saved, value.clone(), 0, 1), cx);
            record(step(StepKey::Saved, value.clone(), 1, 2), cx);
            assert_eq!(undo(cx), Outcome::Undid("1".into()));
            assert_eq!(value.get(), 1);
            assert_eq!(undo(cx), Outcome::Undid("0".into()));
            assert_eq!(undo(cx), Outcome::Nothing);
            assert_eq!(redo(cx), Outcome::Redid("1".into()));
            assert_eq!(value.get(), 1);
            // a new change forgets what was undone
            record(step(StepKey::Saved, value.clone(), 1, 5), cx);
            assert_eq!(redo(cx), Outcome::Nothing);
            assert_eq!(undo(cx), Outcome::Undid("1".into()));
        });
    }

    #[test]
    fn a_fields_edits_merge_and_save_forgets_them() {
        let mut history = History::default();
        let value = Rc::new(Cell::new(0));
        let view = EntityId::from(7u64);
        let name = StepKey::Form { view, fields: 1 };
        history.push(step(StepKey::Saved, value.clone(), 0, 1));
        history.push(step(name, value.clone(), 1, 2));
        history.push(step(name, value.clone(), 2, 3));
        assert_eq!(history.done.len(), 2);
        let last = history.done.back().unwrap();
        assert_eq!(((last.before)(), (last.after)()), ("1".into(), "3".into()));
        history.push(step(StepKey::Form { view, fields: 2 }, value.clone(), 3, 4));
        assert_eq!(history.done.len(), 3);
        history.forget_form(view);
        assert_eq!(history.done.len(), 1);
    }

    #[cfg(feature = "screenshot")]
    #[gpui::test]
    fn gone_steps_are_skipped_and_refused_ones_stay(cx: &mut gpui::TestAppContext) {
        let value = Rc::new(Cell::new(1));
        cx.update(|cx| {
            record(step(StepKey::Saved, value.clone(), 0, 1), cx);
            record(
                Step::new(
                    StepKey::Saved,
                    || "gone".into(),
                    || "gone".into(),
                    |_| Err(UndoError::Gone),
                    |_| Ok(()),
                ),
                cx,
            );
            assert_eq!(undo(cx), Outcome::Undid("0".into()));
            record(
                Step::new(
                    StepKey::Saved,
                    || "x".into(),
                    || "y".into(),
                    |_| Err(UndoError::Refused("running".into())),
                    |_| Ok(()),
                ),
                cx,
            );
            assert_eq!(undo(cx), Outcome::Refused("running".into()));
            assert!(cx.global::<History>().can_undo());
        });
    }

    #[test]
    fn files_are_put_back() {
        let dir = std::env::temp_dir().join(format!("lodestone-undo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::write(&a, b"one").unwrap();
        let before = Files::read(&[a.clone(), b.clone()]);
        std::fs::write(&a, b"two").unwrap();
        std::fs::write(&b, b"new").unwrap();
        before.write().unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), b"one");
        assert!(!b.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
