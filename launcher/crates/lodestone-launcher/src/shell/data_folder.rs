//! The dialogs for moving the data folder (Settings › Storage) and for a moved data
//! folder that isn't there at start (a drive that isn't connected, say), and the
//! restart into the new folder.

use std::path::{Path, PathBuf};

use gpui::{AnyElement, Context, FocusHandle, IntoElement, div, prelude::*, px, relative};
use launcher_control::paths::DataDirSource;

use crate::{
    components::{Button, ButtonVariant, Dialog},
    i18n::{CommonMsg, ShellMsg as M, format, t, tr},
    theme::{self, colors, text},
};

use super::{LauncherShell, Modal};

impl LauncherShell {
    /// At start: when the data folder was moved and isn't there, says so before
    /// anything else.
    pub fn check_data_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = missing_data_folder() {
            let focus = cx.focus_handle();
            self.modal
                .open(Modal::DataFolderMissing { path, focus }, cx);
        }
    }

    /// Shows Settings > Storage's data folder as moved or not, and "Move your data?",
    /// for the offscreen screenshots.
    pub fn preview_data_folder(&mut self, moved: bool, confirm: bool, cx: &mut Context<Self>) {
        self.storage
            .update(cx, |storage, cx| storage.preview_location(moved, cx));
        if confirm {
            let focus = cx.focus_handle();
            let to = PathBuf::from("D:\\Games\\Lodestone Launcher");
            self.modal.open(
                Modal::DataMove {
                    to,
                    back: false,
                    focus,
                },
                cx,
            );
        }
    }

    /// Starts the launcher again and quits this one. Returns `false` when it couldn't.
    pub(super) fn restart_launcher(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(exe) = launcher_core::update::exe() else {
            return false;
        };
        match launcher_core::update::restart(exe) {
            Ok(()) => {
                cx.quit();
                true
            }
            Err(err) => {
                eprintln!("lodestone-launcher: couldn't restart: {err}");
                false
            }
        }
    }

    /// The data folder moved: restart into it.
    pub(super) fn data_folder_moved(&mut self, cx: &mut Context<Self>) {
        if !self.restart_launcher(cx) {
            self.close_modal(cx);
            self.storage.update(cx, |storage, cx| {
                storage.show_toast(t(M::StorageMoveRestartFailed).into(), true, cx)
            });
        }
    }

    /// "Move your data?", its progress, or why it failed.
    pub(super) fn render_data_move(
        &self,
        to: &Path,
        back: bool,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let storage = self.storage.read(cx);
        let from = storage.data_root(cx);
        let entity = cx.entity();

        if let Some(moving) = storage.moving() {
            use std::sync::atomic::Ordering;
            let total = moving.progress.total.load(Ordering::Relaxed);
            let done = moving.progress.done.load(Ordering::Relaxed);
            let fraction = moving.progress.fraction().unwrap_or(0.0);
            let status = if total > 1 {
                tr!(
                    M::StorageMovingProgress,
                    done = format::bytes(done),
                    total = format::bytes(total)
                )
            } else {
                t(M::StorageMovingPreparing).to_string()
            };
            // No ×: it closes when the move ends.
            return Dialog::new("data-move", t(M::StorageMoving), focus)
                .child(
                    div()
                        .text_color(crate::theme::ink::secondary())
                        .child(tr!(M::StorageMovingTo, to = to.display())),
                )
                .child(
                    div()
                        .mt_3()
                        .w_full()
                        .h(px(8.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(colors::GRAY_750)
                        .child(div().h_full().w(relative(fraction)).bg(colors::BLUE_200)),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::muted())
                        .child(status)
                        .child(t(M::StorageMovingNote)),
                )
                .footer(
                    Button::new("cancel-data-move", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.storage.update(cx, |storage, _| storage.cancel_move());
                        })),
                )
                .into_any_element();
        }

        if let Some(error) = storage.move_failed() {
            let close = entity.clone();
            return Dialog::new("data-move", t(M::StorageMoveFailedTitle), focus)
                .on_close(move |_, cx| {
                    close.update(cx, |this, cx| {
                        this.storage
                            .update(cx, |storage, cx| storage.clear_move_failed(cx));
                        this.close_modal(cx);
                    })
                })
                .child(div().text_color(crate::theme::ink::secondary()).child(tr!(
                    M::StorageMoveFailedBody,
                    from = from.display(),
                    error = error
                )))
                .into_any_element();
        }

        let (title, body) = if back {
            (
                M::StorageMoveBackTitle,
                tr!(M::StorageMoveBackBody, to = to.display()),
            )
        } else {
            (
                M::StorageMoveTitle,
                tr!(M::StorageMoveBody, to = to.display()),
            )
        };
        let size = storage.counted_total();
        let to_owned: PathBuf = to.to_path_buf();
        Dialog::new("data-move", t(title), focus)
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
            .child(div().text_color(crate::theme::ink::secondary()).child(body))
            .when_some(size, |dialog, size| {
                dialog.child(
                    div()
                        .mt_2()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::muted())
                        .child(tr!(M::StorageMoveSize, size = format::bytes(size))),
                )
            })
            .footer(
                Button::new("confirm-data-move", t(M::StorageMoveConfirm))
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let to = to_owned.clone();
                        this.storage
                            .update(cx, |storage, cx| storage.start_move(to, cx));
                    })),
            )
            .footer(
                Button::new("cancel-data-move", t(CommonMsg::Cancel))
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
            )
            .into_any_element()
    }

    /// "Can't find your data folder": try again, use the default folder, or quit.
    pub(super) fn render_data_folder_missing(
        &self,
        path: &Path,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // No ×: the launcher can't go on without its data.
        Dialog::new("data-folder-missing", t(M::DataFolderMissingTitle), focus)
            .child(
                div()
                    .text_color(crate::theme::ink::secondary())
                    .child(tr!(M::DataFolderMissingBody, path = path.display())),
            )
            .footer(
                Button::new("data-folder-try-again", t(CommonMsg::TryAgain))
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if missing_data_folder().is_none() {
                            this.restart_launcher(cx);
                        }
                    })),
            )
            .footer(
                Button::new("data-folder-use-default", t(M::DataFolderUseDefault))
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let default = launcher_control::paths::locate().map(|l| l.default);
                        if let Some(default) = default
                            && launcher_control::paths::write_location(&default, None).is_ok()
                        {
                            this.restart_launcher(cx);
                        }
                    })),
            )
            .footer(
                Button::new("data-folder-quit", t(M::DataFolderQuit))
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|_, _, _, cx| cx.quit())),
            )
            .into_any_element()
    }
}

/// The moved data folder, when it isn't there.
pub fn missing_data_folder() -> Option<PathBuf> {
    let location = launcher_control::paths::locate()?;
    (location.source == DataDirSource::Moved && !location.dir.is_dir()).then_some(location.dir)
}

/// At start, in the background: removes the folder a move left behind, once the
/// launcher runs from the new one.
pub fn finish_pending_move() {
    let Some(location) = launcher_control::paths::locate() else {
        return;
    };
    match launcher_core::data_move::finish_pending(&location.default, &location.dir) {
        launcher_core::data_move::Finished::Removed(path) => {
            eprintln!(
                "lodestone-launcher: removed the old data folder {}",
                path.display()
            );
        }
        launcher_core::data_move::Finished::Failed { path, error } => {
            eprintln!(
                "lodestone-launcher: couldn't remove all of the old data folder {} (trying again next start): {error}",
                path.display()
            );
        }
        _ => {}
    }
}
