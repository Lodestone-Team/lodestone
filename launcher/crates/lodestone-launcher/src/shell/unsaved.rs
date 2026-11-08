//! Asking before unsaved settings are left behind: closing the Settings dialog with
//! changes to Instance defaults, or selecting another instance with changes in the
//! Settings tab. Save, Discard, or keep editing.

use gpui::{AnyElement, Context, Entity, FocusHandle, IntoElement, Window, div, prelude::*};

use super::{LauncherShell, Modal};
use crate::{
    components::{Button, ButtonVariant, Dialog},
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    instances::settings::InstanceSettings,
    theme::colors,
};

/// Whose changes the question is about.
#[derive(Clone)]
pub enum UnsavedTarget {
    /// Settings > Instance defaults, as the Settings dialog closes.
    LauncherDefaults,
    /// The Settings tab of the instance that was selected before.
    Instance(Entity<InstanceSettings>),
}

impl LauncherShell {
    /// Closes the Settings dialog, or first asks about unsaved changes.
    pub fn request_close_settings(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.launcher_settings.read(cx).has_changes(cx) {
            self.ask_unsaved(UnsavedTarget::LauncherDefaults, window, cx);
        } else {
            self.close_settings(cx);
        }
    }

    pub(super) fn ask_unsaved(
        &mut self,
        target: UnsavedTarget,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.modal.current(), Some(Modal::Unsaved { .. })) {
            return;
        }
        let focus = cx.focus_handle();
        if let Some(window) = window {
            window.focus(&focus, cx);
        }
        self.modal.open(Modal::Unsaved { focus, target }, cx);
    }

    /// Asks about unsaved changes in the Settings dialog, for the offscreen screenshots.
    pub fn preview_unsaved_prompt(&mut self, cx: &mut Context<Self>) {
        self.ask_unsaved(UnsavedTarget::LauncherDefaults, None, cx);
    }

    /// Saves (or discards) the changes the question is about, then goes on with what
    /// asked: closing the Settings dialog, or showing the newly selected instance.
    fn finish_unsaved(&mut self, save: bool, cx: &mut Context<Self>) {
        let Some(Modal::Unsaved { target, .. }) = self.modal.current() else {
            return;
        };
        let target = target.clone();
        match target {
            UnsavedTarget::LauncherDefaults => {
                let done = self.launcher_settings.update(cx, |settings, cx| {
                    if save {
                        settings.save(cx)
                    } else {
                        settings.discard(cx);
                        true
                    }
                });
                self.modal.close(cx);
                // A value that isn't valid keeps the dialog open, showing why.
                if done {
                    self.close_settings(cx);
                }
            }
            UnsavedTarget::Instance(view) => {
                let done = view.update(cx, |settings, cx| {
                    if save {
                        settings.save(cx)
                    } else {
                        settings.discard(cx);
                        true
                    }
                });
                self.modal.close(cx);
                if done {
                    self.settings = None;
                    self.sync_settings(cx);
                } else {
                    // Back to the instance whose settings need fixing first.
                    let id = view.read(cx).instance_id().to_string();
                    self.select_instance(&id, cx);
                    self.select_tab(super::InstanceTab::Settings, cx);
                }
            }
        }
        cx.notify();
    }

    /// "Keep editing": back to the changes, and to their instance if another one was
    /// selected.
    fn keep_editing(&mut self, cx: &mut Context<Self>) {
        if let Some(Modal::Unsaved {
            target: UnsavedTarget::Instance(view),
            ..
        }) = self.modal.current()
        {
            let id = view.read(cx).instance_id().to_string();
            self.select_instance(&id, cx);
            self.select_tab(super::InstanceTab::Settings, cx);
        }
        self.close_modal(cx);
    }

    pub(super) fn render_unsaved(
        &self,
        focus: FocusHandle,
        target: &UnsavedTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = match target {
            UnsavedTarget::LauncherDefaults => t(M::UnsavedDefaultsBody).to_string(),
            UnsavedTarget::Instance(view) => {
                let id = view.read(cx).instance_id().to_string();
                match self.instances.read(cx).instance_name(&id) {
                    Some(name) => tr!(M::UnsavedInstanceBody, name = name),
                    None => t(M::UnsavedThisInstanceBody).to_string(),
                }
            }
        };
        let entity = cx.entity();
        Dialog::new("unsaved-settings", t(M::SaveChangesTitle), focus)
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.keep_editing(cx)))
            .child(div().text_color(colors::GRAY_400).child(body))
            .footer(
                Button::new("unsaved-save", t(CommonMsg::Save))
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, _, _, cx| this.finish_unsaved(true, cx))),
            )
            .footer(
                Button::new("unsaved-discard", t(CommonMsg::Discard))
                    .variant(ButtonVariant::Danger)
                    .on_click(cx.listener(|this, _, _, cx| this.finish_unsaved(false, cx))),
            )
            .footer(
                Button::new("unsaved-cancel", t(CommonMsg::KeepEditing))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.keep_editing(cx))),
            )
            .into_any_element()
    }
}
