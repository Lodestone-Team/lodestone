//! Settings › Updates › Developer › "Connect service (relay)": the Lodestone Connect
//! introduction service this computer's core shares through, for testing against
//! another deployment (`https://connect-dev.lodestone.cc`) or a local `wrangler dev`.
//!
//! It is saved in the Host's settings ([`Prefs`]) and reaches the core as
//! `LODESTONE_CONNECT_SERVICE` ([`crate::core_process::CoreConfig::env`]), which the
//! core reads when it starts. So saving a change restarts this computer's core, which
//! stops its servers: with servers running on it, the row asks first, as Restart to
//! update does ([`crate::local_core`], whichever core the shell shows). The
//! Host's own environment variable wins over the setting, which the row says.

use gpui::{
    AnyElement, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render,
    SharedString, Styled, Task, Window, div, prelude::*,
};
use launcher_core::cores::connect_service::{self, DEFAULT_SERVICE, Service, SettingError, Source};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, TextInput,
        a11y::Accessible as _,
        settings::{SettingRow, card, muted_link},
        window_layer,
    },
    theme::{Color, colors, ink, leading, px, text},
};

use crate::{
    i18n::{CommonMsg, CoreMsg as M, CorePlural, t, tr, trn},
    prefs::Prefs,
};

/// What a setting's error says, for people.
pub fn error_text(error: SettingError) -> &'static str {
    match error {
        SettingError::Scheme => t(M::ConnectServiceScheme),
        SettingError::Insecure => t(M::ConnectServiceInsecure),
        SettingError::Extra => t(M::ConnectServiceExtra),
        SettingError::Address => t(M::ConnectServiceAddress),
    }
}

/// "Dev relay: connect-dev.lodestone.cc" for a service that isn't the default.
pub fn relay_note(service: &Service) -> Option<String> {
    (*service != Service::default())
        .then(|| tr!(M::ConnectServiceInUse, service = service.display_name()))
}

/// Whether Settings › Updates shows its Developer section: in a developer build (or a
/// debug one), with developer builds on, or while the service isn't the default.
pub fn offered(cx: &gpui::App) -> bool {
    cfg!(debug_assertions)
        || Prefs::get(cx).dev_builds
        || Prefs::get(cx).connect_service.is_some()
        || crate::updater::this_build().is_some_and(|build| build.dev)
        || connect_service::active().source == Source::Env
}

pub struct ConnectServiceSetting {
    input: Entity<TextInput>,
    /// "Restart the core?", with the service to save, while servers run on this
    /// computer's core: with their count when it was asked for, `None` when it isn't
    /// known.
    confirm: DialogSlot<Self, Confirm>,
    /// Counting this computer's core's servers, before saving (replaced by the next).
    asking: Option<Task<()>>,
}

/// What "Restart the core?" holds.
struct Confirm {
    focus: FocusHandle,
    service: Option<Service>,
    running: Option<usize>,
}

impl ConnectServiceSetting {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let saved = connect_service::setting().map(|s| s.base_url());
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(DEFAULT_SERVICE)
                .with_label(M::ConnectService)
                .with_automation_id("connect-service-input")
                .with_text(saved.unwrap_or_default())
        });
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        Self {
            input,
            confirm: DialogSlot::new(|this: &mut Self| &mut this.confirm),
            asking: None,
        }
    }

    fn typed(&self, cx: &gpui::App) -> Result<Option<Service>, SettingError> {
        connect_service::parse(self.input.read(cx).text())
    }

    /// Saves the typed service, if valid and changed: asks first while servers run.
    fn request_save(&mut self, cx: &mut Context<Self>) {
        let Ok(service) = self.typed(cx) else {
            return;
        };
        self.request(service, cx);
    }

    /// Back to connect.lodestone.cc.
    fn request_reset(&mut self, cx: &mut Context<Self>) {
        self.request(None, cx);
    }

    fn request(&mut self, service: Option<Service>, cx: &mut Context<Self>) {
        if service == connect_service::setting() {
            return;
        }
        if !restarts_core() {
            return self.save(service, cx);
        }
        // the core restarted is this computer's, whichever the shell shows
        let running = crate::local_core::running_servers(cx);
        self.asking = Some(cx.spawn(async move |this, cx| {
            let running = running.await;
            this.update(cx, |this, cx| {
                if crate::local_core::asks(running) {
                    let focus = cx.focus_handle();
                    this.confirm.open(
                        Confirm {
                            focus,
                            service,
                            running,
                        },
                        cx,
                    );
                } else {
                    this.save(service, cx);
                }
            })
            .ok();
        }));
    }

    /// Saves `service` and restarts the core with it (unless the environment variable
    /// decides the core's service anyway).
    fn save(&mut self, service: Option<Service>, cx: &mut Context<Self>) {
        self.confirm.close(cx);
        let restart = restarts_core();
        let text = service.as_ref().map(|s| s.base_url());
        Prefs::update(cx, |prefs| prefs.connect_service = text.clone());
        connect_service::set_setting(service);
        self.input
            .update(cx, |input, cx| input.set_text(text.unwrap_or_default(), cx));
        if restart {
            // after this click or key is handled (LS-200)
            crate::dispatch_later(crate::RestartCore, cx);
        }
        cx.notify();
    }

    /// Types `text`, for the development screenshots.
    pub fn preview_type(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    fn confirm_dialog(&self, confirm: &Confirm, cx: &mut Context<Self>) -> AnyElement {
        // live while the shell shows this computer's core
        let running = crate::local_core::running_now(cx).or(confirm.running);
        let service = confirm.service.clone();
        Dialog::new(
            "restart-core-dialog",
            t(M::RestartCoreTitle),
            confirm.focus.clone(),
        )
        .on_close({
            let close = cx.listener(|this, _: &(), _, cx| {
                this.confirm.close(cx);
            });
            move |window, cx| close(&(), window, cx)
        })
        .child(
            div()
                .text_size(text::MEDIUM)
                .line_height(lodestone_ui::theme::line_height(
                    text::MEDIUM,
                    leading::NORMAL,
                ))
                .text_color(ink::secondary())
                // without the count (this computer's core didn't answer), the title
                // and "Stop servers and restart" say it
                .when_some(running, |this, running| {
                    this.child(trn!(CorePlural::ServersRunning, running))
                        .child(t(M::RestartCoreBody))
                }),
        )
        .footer(
            Button::new("restart-core-confirm", t(M::RestartUpdateConfirm))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(move |this, _, _, cx| this.save(service.clone(), cx))),
        )
        .footer(
            Button::new("restart-core-cancel", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.confirm.close(cx);
                })),
        )
        .into_any_element()
    }
}

/// Saving restarts the core: unless `LODESTONE_CONNECT_SERVICE` decides its service.
fn restarts_core() -> bool {
    !connect_service::env_overrides()
}

impl Render for ConnectServiceSetting {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = connect_service::setting();
        let typed = self.typed(cx);
        let blank = self.input.read(cx).text().trim().is_empty();
        let changed = matches!(&typed, Ok(service) if *service != saved);
        let active = connect_service::active();
        let note: Option<(SharedString, Color)> = match (&typed, blank) {
            (Err(error), false) => Some((error_text(*error).into(), colors::RED_200)),
            _ if active.source == Source::Env => Some((
                tr!(
                    M::ConnectServiceEnv,
                    var = connect_service::ENV,
                    service = active.url()
                )
                .into(),
                colors::YELLOW,
            )),
            _ => relay_note(&active.service).map(|note| (note.into(), colors::YELLOW)),
        };
        let save_label = if restarts_core() {
            t(M::SaveAndRestartCore)
        } else {
            t(CommonMsg::Save)
        };
        let row = SettingRow::new(t(M::ConnectService))
            .hint(t(M::ConnectServiceHint))
            .note_opt(note)
            .child(
                div()
                    .w(px(260.))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.request_save(cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.input.clone()),
            )
            .child(
                Button::new("connect-service-save", save_label)
                    .variant(ButtonVariant::Primary)
                    .disabled(!changed)
                    .on_click(cx.listener(|this, _, _, cx| this.request_save(cx))),
            )
            .when(saved.is_some(), |row| {
                row.child(
                    muted_link("connect-service-reset", t(CommonMsg::Reset))
                        .on_press(cx.listener(|this, _, _, cx| this.request_reset(cx))),
                )
            });
        let dialogs = self
            .confirm
            .render(window, |payload| Some(self.confirm_dialog(payload, cx)));
        div()
            .child(card([row.into_any_element()]))
            .children(window_layer(window, dialogs))
    }
}

/// With GPUI's test platform, which the `screenshot` feature turns on.
#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::local_core::{self, gpui_tests::state, instance};

    /// Saving a service while the shell shows another core (`shown`), with `local` as
    /// this computer's core: whether it asks first, and whether it saved.
    ///
    /// It saves connect.lodestone.cc itself, which differs from the setting (none) but
    /// leaves the process's setting as it was, for the other tests.
    fn save_asks(local: &str, shown: &str) -> Option<(bool, bool)> {
        if !restarts_core() {
            // LODESTONE_CONNECT_SERVICE decides the service; saving restarts nothing
            return None;
        }
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            lodestone_ui::components::init(cx);
            crate::prefs::Prefs::init_for_tests(cx);
        });
        let local = state(vec![instance("a", local)], &mut cx);
        // the shell's state, which the row doesn't take: it counts this computer's core
        let _shown = state(vec![instance("b", shown)], &mut cx);
        cx.update(|cx| local_core::set(&local, cx));
        let row = cx.new(ConnectServiceSetting::new);
        row.update(&mut cx, |row, cx| row.request(Some(Service::default()), cx));
        cx.run_until_parked();
        let asked = row.read_with(&cx, |row, _| row.confirm.is_open());
        let saved = cx.update(|cx| Prefs::get(cx).connect_service.is_some());
        Some((asked, saved))
    }

    /// LS-11: a server running on this computer's core asks before the core restarts,
    /// though the core shown runs none.
    #[test]
    fn saving_asks_about_this_computers_servers_while_another_core_is_shown() {
        if let Some(result) = save_asks("Running", "Stopped") {
            assert_eq!(result, (true, false), "asks, and saves nothing yet");
        }
    }

    /// LS-200: "Save and restart core", pressed in the window, restarts the core. The
    /// click is handled while the window is being updated, so the action is dispatched
    /// afterwards, and its handler (as `main.rs` registers it) updates the window
    /// afterwards too: called at once, either one fails and nothing restarts.
    #[test]
    fn saving_in_the_window_restarts_the_core() {
        use std::{cell::Cell, rc::Rc};

        if !restarts_core() {
            return;
        }
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            lodestone_ui::components::init(cx);
            crate::prefs::Prefs::init_for_tests(cx);
        });
        let window = cx.add_window(|_, cx| ConnectServiceSetting::new(cx));
        let restarts = Rc::new(Cell::new(0));
        cx.update(|cx| {
            let restarts = restarts.clone();
            cx.on_action(move |_: &crate::RestartCore, cx| {
                let restarts = restarts.clone();
                crate::update_later(window, move |_, _| restarts.set(restarts.get() + 1), cx);
            });
        });
        window
            .update(&mut cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        // as the button's click does: inside the window's update
        window
            .update(&mut cx, |row, _, cx| row.save(Some(Service::default()), cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(restarts.get(), 1, "the core restarted once");
    }

    /// LS-11: the shown core's running servers don't ask: the restart doesn't touch them.
    #[test]
    fn saving_doesnt_ask_about_the_shown_cores_servers() {
        if let Some(result) = save_asks("Stopped", "Running") {
            assert_eq!(result, (false, true), "saves at once");
        }
    }
}
