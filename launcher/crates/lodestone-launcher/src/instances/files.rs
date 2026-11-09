//! Instance changes whose file work is slow, above all on Windows (Defender scans each
//! file, and NTFS is slower with metadata): copying an instance, moving it to the
//! Trash, importing an image. The work runs off the window's thread with the instance
//! marked busy, and the list is changed in place when it is done. Also the list's
//! updates after one instance changed, and the custom icons, decoded off the window's
//! thread.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

use futures::channel::oneshot;
use gpui::{AppContext as _, Context, Task};
use launcher_core::instances::{CUSTOM_ICON, ICON_FILE, Instance, sort_listed};

use super::Instances;
use crate::i18n::{InstancesMsg as M, t};

/// The file work an instance is busy with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    /// Copying it to a new instance.
    Duplicate,
    /// Moving it to the Trash.
    Delete,
    /// Importing an image: its icon, its Play background or slideshow images.
    Image,
}

impl Instances {
    /// Goes up each time the list changes, for those that read something for each
    /// instance and want to know when to read again.
    pub fn list_generation(&self) -> u64 {
        self.list_generation
    }

    /// The file work instance `id` is busy with, if any.
    pub fn file_op(&self, id: &str) -> Option<FileOp> {
        self.file_ops.get(id).copied()
    }

    /// Runs `work` off the window's thread with instance `id` marked busy with `op`,
    /// then `done` on the window's thread with its result. Refused while the instance
    /// is busy with other file work. The returned task gives `done`'s result; dropping
    /// it stops neither the work nor `done`.
    pub(super) fn file_work<T: Send + 'static, R: 'static>(
        &mut self,
        id: &str,
        op: FileOp,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
        done: impl FnOnce(&mut Self, Result<T, String>, &mut Context<Self>) -> Result<R, String>
        + 'static,
        cx: &mut Context<Self>,
    ) -> Task<Result<R, String>> {
        if self.file_ops.contains_key(id) {
            return Task::ready(Err(t(M::FilesBusy).into()));
        }
        self.file_ops.insert(id.to_string(), op);
        cx.notify();
        let id = id.to_string();
        let work = cx.background_spawn(async move { work() });
        let (tx, rx) = oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let result = this.update(cx, |this, cx| {
                this.file_ops.remove(&id);
                cx.notify();
                done(this, result, cx)
            });
            if let Ok(result) = result {
                tx.send(result).ok();
            }
        })
        .detach();
        answer(rx, cx)
    }

    /// Runs `task` to its end and then `after` with its result, even if the caller
    /// drops the returned task, which gives the same result.
    pub(super) fn then_detached<R: 'static>(
        &mut self,
        task: Task<Result<R, String>>,
        after: impl FnOnce(&mut Self, &Result<R, String>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Task<Result<R, String>> {
        let (tx, rx) = oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| after(this, &result, cx)).ok();
            tx.send(result).ok();
        })
        .detach();
        answer(rx, cx)
    }

    /// Copies the instance in the background and selects the copy.
    pub fn duplicate(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<Instance, String>> {
        let store = self.store.clone();
        let source = id.to_string();
        self.file_work(
            id,
            FileOp::Duplicate,
            move || store.duplicate(&source).map_err(|err| err.to_string()),
            |this, result, cx| {
                let copy = result?;
                this.selected = Some(copy.id.clone());
                this.update_listed(copy.clone(), cx);
                Ok(copy)
            },
            cx,
        )
    }

    /// Moves the instance to the Trash, in the background.
    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        if self.is_active(id) {
            return Task::ready(Err(t(M::StopBeforeDeleting).into()));
        }
        let store = self.store.clone();
        let gone = id.to_string();
        let forget = id.to_string();
        self.file_work(
            id,
            FileOp::Delete,
            move || store.delete(&gone).map_err(|err| err.to_string()),
            move |this, result, cx| {
                result?;
                this.runs.remove(&forget);
                let dir = this.store.dir(&forget);
                this.undo_images.retain(|path, _| !path.starts_with(&dir));
                this.list.retain(|instance| instance.id != forget);
                this.list_changed(cx);
                Ok(())
            },
            cx,
        )
    }

    /// Makes the image at `image` the instance's icon. The image is read, scaled and
    /// written in the background.
    pub fn set_icon(
        &mut self,
        id: &str,
        image: &Path,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let store = self.store.clone();
        let (target, image) = (id.to_string(), image.to_path_buf());
        let record = id.to_string();
        self.file_work(
            id,
            FileOp::Image,
            move || {
                store
                    .write_icon(&target, &image)
                    .map_err(|err| err.to_string())
            },
            move |this, result, cx| {
                result?;
                // the record is changed here, so a change saved meanwhile stays
                let instance = this
                    .store
                    .use_custom_icon(&record)
                    .map_err(|err| err.to_string())?;
                this.update_listed(instance, cx);
                Ok(())
            },
            cx,
        )
    }

    /// Puts `instance`, as just saved, in the list in place of the one with its id,
    /// in the list's order: what [`Self::reload`] would read, without reading every
    /// instance's folder.
    pub(super) fn update_listed(&mut self, instance: Instance, cx: &mut Context<Self>) {
        let mut added = None;
        match self.list.iter_mut().find(|i| i.id == instance.id) {
            Some(listed) => *listed = instance,
            None => {
                added = Some(instance.id.clone());
                self.list.push(instance);
            }
        }
        sort_listed(&mut self.list);
        self.list_changed(cx);
        if let Some(id) = added {
            self.offer_main(&id, cx);
        }
    }

    /// Reads instance `id` again, alone; everything again if it can't be read.
    pub(super) fn refresh(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.store.get(id) {
            Ok(instance) => self.update_listed(instance, cx),
            Err(_) => self.reload(cx),
        }
    }

    /// After the list changed: a selection that is gone moves to the first instance,
    /// and the images follow.
    pub(super) fn list_changed(&mut self, cx: &mut Context<Self>) {
        self.list_generation += 1;
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.list.iter().any(|i| &i.id == id))
        {
            self.selected = self.list.first().map(|i| i.id.clone());
        }
        self.load_icons(cx);
        self.load_backgrounds(cx);
        cx.notify();
    }

    /// Decodes the custom icons that are new or changed, off the window's thread, and
    /// forgets those that are gone. An icon being replaced shows until its new image
    /// is decoded.
    pub(super) fn load_icons(&mut self, cx: &mut Context<Self>) {
        let wanted: Vec<(String, PathBuf)> = self
            .list
            .iter()
            .filter(|instance| instance.icon.as_deref() == Some(CUSTOM_ICON))
            .map(|instance| {
                let path = self.store.dir(&instance.id).join(ICON_FILE);
                (instance.id.clone(), path)
            })
            .collect();
        let mut gone = Vec::new();
        self.icons.retain(|id, (_, image)| {
            let keep = wanted.iter().any(|(w, _)| w == id);
            if !keep {
                gone.push(image.clone());
            }
            keep
        });
        crate::images::release(gone, cx);
        if wanted.is_empty() {
            self.icon_task = None;
            return;
        }
        let known: HashMap<String, Option<SystemTime>> = self
            .icons
            .iter()
            .map(|(id, (modified, _))| (id.clone(), *modified))
            .collect();
        let work = cx.background_spawn(async move {
            wanted
                .into_iter()
                .filter_map(|(id, path)| {
                    let modified = match std::fs::metadata(&path) {
                        Ok(meta) if meta.is_file() => meta.modified().ok(),
                        // a custom icon whose file is missing shows the default
                        _ => return Some((id, None)),
                    };
                    if known.get(&id) == Some(&modified) {
                        return None;
                    }
                    let image = image::open(&path).ok()?.to_rgba8();
                    Some((id, Some((modified, image))))
                })
                .collect::<Vec<_>>()
        });
        // A newer look replaces one still running: it knows less of what is decoded,
        // so it decodes whatever the old one would have.
        self.icon_task = Some(cx.spawn(async move |this, cx| {
            let decoded = work.await;
            this.update(cx, |this, cx| {
                this.icon_task = None;
                if decoded.is_empty() {
                    return;
                }
                let mut replaced = Vec::new();
                for (id, entry) in decoded {
                    let old = match entry {
                        Some((modified, image)) => this
                            .icons
                            .insert(id, (modified, crate::images::render_image(image))),
                        None => this.icons.remove(&id),
                    };
                    replaced.extend(old.map(|(_, image)| image));
                }
                crate::images::release(replaced, cx);
                cx.notify();
            })
            .ok();
        }));
    }
}

/// The task that waits for a result sent on `rx`.
fn answer<R: 'static>(
    rx: oneshot::Receiver<Result<R, String>>,
    cx: &mut Context<Instances>,
) -> Task<Result<R, String>> {
    cx.spawn(async move |_, _| {
        rx.await
            .unwrap_or_else(|_| Err(t(M::Cancelled).to_string()))
    })
}
