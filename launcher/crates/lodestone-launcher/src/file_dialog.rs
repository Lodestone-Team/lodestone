//! The system's file dialogs: what `cx.prompt_for_paths` and `cx.prompt_for_new_path`
//! give back, for every "Browse…", "Import…" and "Export…" button.
//!
//! GPUI answers with `Ok(None)` when the user cancels, and with an error when the
//! dialog couldn't open at all. On Linux that happens when no xdg-desktop-portal
//! (with a file chooser for the desktop) is running, which some window managers and
//! minimal installs don't have. Ignoring the error, as a cancel, made the button do
//! nothing, so [`chosen`] tells the user instead, in the view's own error message.

use futures::channel::oneshot;
use gpui::{AsyncApp, Context, SharedString, WeakEntity};

use crate::i18n::{CommonMsg, t};

/// What a file dialog's answer means: what the user chose, `Ok(None)` when they
/// cancelled (or the dialog went away without answering), or the message to show
/// when it couldn't open.
fn outcome<T>(
    answer: Result<anyhow::Result<Option<T>>, oneshot::Canceled>,
) -> Result<Option<T>, SharedString> {
    match answer {
        Ok(Ok(chosen)) => Ok(chosen),
        Ok(Err(err)) => {
            // The system's words, for the debug traces; the user gets ours.
            log::warn!("The file dialog couldn't open: {err:#}");
            Err(t(CommonMsg::FileDialogUnavailable).into())
        }
        Err(oneshot::Canceled) => Ok(None),
    }
}

/// A view with a button that opens a file dialog, and a place for an error message:
/// the inline message of a settings tab, or of a dialog (where a notice would close
/// the dialog it is about).
pub trait ShowsError: Sized {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>);
}

/// Waits for a file dialog and returns what the user chose, or `None` when they
/// cancelled. When the dialog couldn't open, `view` shows why, and this returns `None`.
pub async fn chosen<V: ShowsError + 'static, T>(
    dialog: oneshot::Receiver<anyhow::Result<Option<T>>>,
    view: &WeakEntity<V>,
    cx: &mut AsyncApp,
) -> Option<T> {
    match outcome(dialog.await) {
        Ok(chosen) => chosen,
        Err(message) => {
            view.update(cx, |view, cx| view.show_error(message, cx))
                .ok();
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_dialog_that_couldnt_open_is_an_error() {
        let path = || Some(vec![std::path::PathBuf::from("/tmp/pack.mrpack")]);
        assert_eq!(outcome(Ok(Ok(path()))), Ok(path()));
        // Cancelling stays silent.
        assert_eq!(outcome::<Vec<std::path::PathBuf>>(Ok(Ok(None))), Ok(None));
        assert_eq!(
            outcome::<Vec<std::path::PathBuf>>(Err(oneshot::Canceled)),
            Ok(None)
        );
        let missing = anyhow::anyhow!(
            "Couldn't open file picker due to missing xdg-desktop-portal implementation."
        );
        assert_eq!(
            outcome::<Vec<std::path::PathBuf>>(Ok(Err(missing))),
            Err(t(CommonMsg::FileDialogUnavailable).into())
        );
    }
}

/// The same through GPUI: the message reaches the view, and only on an error.
#[cfg(all(test, feature = "screenshot"))]
mod gpui_tests {
    use gpui::{AppContext as _, TestAppContext};

    use super::*;

    #[derive(Default)]
    struct View {
        error: Option<SharedString>,
        chosen: Option<Option<std::path::PathBuf>>,
    }

    impl ShowsError for View {
        fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
            self.error = Some(message);
            cx.notify();
        }
    }

    /// GPUI's test platform only answers with a choice or a cancel, so the test sends
    /// the answer itself, as a real platform would.
    fn pick(
        answer: Option<anyhow::Result<Option<std::path::PathBuf>>>,
    ) -> (Option<SharedString>, Option<Option<std::path::PathBuf>>) {
        let mut cx = TestAppContext::single();
        let view = cx.new(|_| View::default());
        let (tx, rx) = oneshot::channel();
        match answer {
            Some(answer) => {
                let _ = tx.send(answer);
            }
            // The dialog went away without answering.
            None => drop(tx),
        }
        let weak = view.downgrade();
        cx.spawn(|mut cx| async move {
            let picked = chosen(rx, &weak, &mut cx).await;
            weak.update(&mut cx, |view, _| view.chosen = Some(picked))
                .ok();
        })
        .detach();
        cx.run_until_parked();
        view.read_with(&cx, |view, _| (view.error.clone(), view.chosen.clone()))
    }

    #[test]
    fn shows_the_error_only_when_the_dialog_couldnt_open() {
        let file = std::path::PathBuf::from("/tmp/world.zip");
        assert_eq!(pick(Some(Ok(Some(file.clone())))), (None, Some(Some(file))));
        assert_eq!(pick(Some(Ok(None))), (None, Some(None)));
        assert_eq!(pick(None), (None, Some(None)));
        assert_eq!(
            pick(Some(Err(anyhow::anyhow!("no portal")))),
            (Some(t(CommonMsg::FileDialogUnavailable).into()), Some(None))
        );
    }
}
