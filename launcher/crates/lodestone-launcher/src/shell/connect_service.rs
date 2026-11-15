//! Settings > Developer > "Connect service (relay)": the Lodestone Connect introduction
//! service the friend list uses, for testing against another deployment
//! (`https://connect-dev.lodestone.cc`) or a local `wrangler dev`. Saved in
//! `settings.json` by the accounts (which own its developer section) and applied at
//! once: lookups read [`connect_service::service`] each time. The
//! `LODESTONE_CONNECT_SERVICE` environment variable wins over it, which the row says.

use gpui::{Context, Entity, IntoElement, KeyDownEvent, Render, SharedString, Window, div};
use gpui::{ParentElement as _, Styled as _, prelude::*};
use launcher_core::cores::connect_service::{self, DEFAULT_SERVICE, SettingError};

use crate::{
    accounts::Accounts,
    components::{
        Button, ButtonSize, ButtonVariant, TextInput,
        a11y::Accessible as _,
        settings::{SettingRow, card, muted_link},
    },
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    theme::{Color, colors, px},
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

/// "Dev relay: connect-dev.lodestone.cc" while the service in use isn't the default,
/// for Settings > Developer and the Friends' servers section.
pub fn custom_note() -> Option<SharedString> {
    let active = connect_service::active();
    active.is_custom().then(|| {
        tr!(
            M::ConnectServiceInUse,
            service = active.service.display_name()
        )
        .into()
    })
}

pub struct ConnectServiceSetting {
    accounts: Entity<Accounts>,
    input: Entity<TextInput>,
    /// Why the last save failed.
    error: Option<SharedString>,
    /// The setting the field last showed: when the command line changes it, the field
    /// follows.
    shown: Option<connect_service::Service>,
}

impl ConnectServiceSetting {
    pub fn new(accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        let shown = connect_service::setting();
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(DEFAULT_SERVICE)
                .with_label(M::ConnectService)
                .with_automation_id("connect-service-input")
                .with_text(shown.as_ref().map(|s| s.base_url()).unwrap_or_default())
        });
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.observe(&accounts, |this, _, cx| {
            let saved = connect_service::setting();
            if saved != this.shown {
                this.show_saved(cx);
            }
        })
        .detach();
        Self {
            accounts,
            input,
            error: None,
            shown,
        }
    }

    /// Puts the saved setting in the field.
    fn show_saved(&mut self, cx: &mut Context<Self>) {
        self.shown = connect_service::setting();
        let text = self
            .shown
            .as_ref()
            .map(|s| s.base_url())
            .unwrap_or_default();
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        cx.notify();
    }

    /// The typed text: the service it names (`None` for the default), or why it
    /// names none.
    fn typed(&self, cx: &gpui::App) -> Result<Option<connect_service::Service>, SettingError> {
        connect_service::parse(self.input.read(cx).text())
    }

    /// Saves the typed service, if it is valid and changed.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Ok(service) = self.typed(cx) else {
            return;
        };
        self.store(service, cx);
    }

    /// Back to connect.lodestone.cc.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text("", cx));
        self.store(None, cx);
    }

    fn store(&mut self, service: Option<connect_service::Service>, cx: &mut Context<Self>) {
        let saved = self
            .accounts
            .update(cx, |accounts, cx| accounts.set_connect_service(service, cx));
        self.error = saved.err();
        if self.error.is_none() {
            // shown as saved: `https://name`, without a trailing slash
            self.show_saved(cx);
        }
        cx.notify();
    }

    /// Types `text`, for the offscreen screenshots.
    pub fn preview_type(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }
}

impl Render for ConnectServiceSetting {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = connect_service::setting();
        let typed = self.typed(cx);
        let blank = self.input.read(cx).text().trim().is_empty();
        let changed = match &typed {
            Ok(service) => *service != saved,
            Err(_) => false,
        };
        let active = connect_service::active();
        let note: Option<(SharedString, Color)> = if let Some(error) = &self.error {
            Some((error.clone(), colors::RED_200))
        } else if let (Err(error), false) = (&typed, blank) {
            Some((error_text(*error).into(), colors::RED_200))
        } else if active.source == connect_service::Source::Env {
            Some((
                tr!(
                    M::ConnectServiceEnv,
                    var = connect_service::ENV,
                    service = active.url()
                )
                .into(),
                colors::YELLOW,
            ))
        } else {
            custom_note().map(|note| (note, colors::YELLOW))
        };
        let row = SettingRow::new(t(M::ConnectService))
            .hint(t(M::ConnectServiceHint))
            .note_opt(note)
            .child(
                div()
                    .w(px(260.))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.save(cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.input.clone()),
            )
            .child(
                Button::new("connect-service-save", t(CommonMsg::Save))
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Primary)
                    .disabled(!changed)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .when(saved.is_some(), |row| {
                row.child(
                    muted_link("connect-service-reset", t(CommonMsg::Reset))
                        .on_press(cx.listener(|this, _, _, cx| this.reset(cx))),
                )
            });
        card([row.into_any_element()])
    }
}
