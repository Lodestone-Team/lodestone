//! "Accept the Minecraft EULA": when starting a Minecraft server fails because the
//! user hasn't agreed to Mojang's EULA (PR #626), as the dashboard's instance card
//! asks. Only the user's click on "I agree, start the server" accepts it, beside a
//! link to the EULA; then the server starts.
//!
//! The shell makes one and draws it over the window; it follows the state's
//! [`HostEvent::StartFailed`].

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement, Render,
    Styled, Subscription, Task, Window, div,
};
use launcher_core::cores::admin::{
    instance_settings::MINECRAFT_EULA_URL,
    instances::Power,
    models::{Game, InstancePermission},
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Toast, a11y::Accessible as _, settings::link,
        window_layer,
    },
    theme::{ink, leading, line_height, px, text},
};

use crate::{
    i18n::{InstanceMsg as M, t, tr},
    state::{HostEvent, HostState},
};

struct Prompt {
    focus: FocusHandle,
    uuid: String,
    name: String,
}

pub struct EulaPrompt {
    state: Entity<HostState>,
    dialog: DialogSlot<Self, Prompt>,
    busy: bool,
    _tasks: Vec<Task<()>>,
    _subscription: Subscription,
}

impl EulaPrompt {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe(&state, |this, _, event, cx| {
            if let HostEvent::StartFailed(uuid) = event {
                this.start_failed(uuid.clone(), cx);
            }
        });
        Self {
            state,
            dialog: DialogSlot::new(|this: &mut Self| &mut this.dialog),
            busy: false,
            _tasks: Vec::new(),
            _subscription: subscription,
        }
    }

    /// A Minecraft server didn't start: asks the user if its EULA is why.
    fn start_failed(&mut self, uuid: String, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let Some(instance) = state.instance(&uuid) else {
            return;
        };
        if !matches!(instance.game_type, Game::MinecraftJava { .. }) || self.dialog.is_open() {
            return;
        }
        let name = instance.name.clone();
        let admin = state.admin.clone();
        let query = uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let status = cx
                .background_spawn(async move { admin.eula(&query).await })
                .await;
            this.update(cx, |this, cx| {
                if status.is_ok_and(|status| !status.accepted) {
                    // the dialog says why, instead of the core's error
                    this.state.update(cx, |state, cx| {
                        state.error = None;
                        cx.notify();
                    });
                    let focus = cx.focus_handle();
                    this.dialog.open(Prompt { focus, uuid, name }, cx);
                }
            })
            .ok();
        });
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    /// The user agreed: accept the EULA, then start the server.
    fn accept(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.dialog.current() else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        cx.notify();
        let uuid = prompt.uuid.clone();
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let accepted = {
                let uuid = uuid.clone();
                cx.background_spawn(async move { admin.accept_eula(&uuid).await })
                    .await
            };
            this.update(cx, |this, cx| {
                this.busy = false;
                this.dialog.close(cx);
                match accepted {
                    Ok(_) => this
                        .state
                        .update(cx, |state, cx| state.power(&uuid, Power::Start, cx)),
                    Err(err) => this.state.update(cx, |state, cx| {
                        state.toast(Toast::error(tr!(M::EulaAcceptFailed, error = err)), cx)
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    fn render_dialog(&self, prompt: &Prompt, cx: &mut Context<Self>) -> AnyElement {
        let may_accept = self
            .state
            .read(cx)
            .me
            .may(InstancePermission::Start, &prompt.uuid);
        Dialog::new("eula-prompt", t(M::EulaDialogTitle), prompt.focus.clone())
            .on_close({
                let this = cx.entity().downgrade();
                move |_, cx| {
                    this.update(cx, |this, cx| {
                        if !this.busy {
                            this.dialog.close(cx);
                        }
                    })
                    .ok();
                }
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .text_size(text::MEDIUM)
                    .line_height(line_height(text::MEDIUM, leading::NORMAL))
                    .text_color(ink::secondary())
                    .child(tr!(M::EulaDialogBody, name = prompt.name))
                    .child(
                        div()
                            .flex()
                            .child(link("eula-prompt-read", t(M::EulaRead)).on_press(
                                |_, _, cx| {
                                    cx.open_url(MINECRAFT_EULA_URL);
                                },
                            )),
                    ),
            )
            .footer(
                Button::new("eula-prompt-accept", t(M::EulaAcceptAndStart))
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .disabled(self.busy || !may_accept)
                    .on_click(cx.listener(|this, _, _, cx| this.accept(cx))),
            )
            .into_any_element()
    }
}

impl Render for EulaPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialogs = self
            .dialog
            .render(window, |prompt| Some(self.render_dialog(prompt, cx)));
        div().children(window_layer(window, dialogs))
    }
}
