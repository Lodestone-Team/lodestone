//! Other cores (`launcher/docs/host-parity.md`, §1.3): the Host runs the core on this
//! computer and shows it first, and can also manage another core, as the dashboard's
//! "Change core" does (`pages/login/CoreConnect.tsx`, `CoreSelectExisting.tsx`).
//!
//! The dialog lists this computer's core and the cores connected to before
//! ([`crate::prefs::SavedCore`]), and connects to a new one: its address (as people
//! type it), its HTTPS certificate when the system doesn't trust it (the user checks
//! the fingerprint and trusts it, as in Lodestone Launcher), then a username and
//! password. Sign-ins aren't saved: the Host signs in again each time. Switching keeps
//! this computer's core and its servers running.

use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        client::{display_address, parse_address},
        tls::{CertificatePin, PinnedHttp},
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, TextInput,
        settings::{self, SettingRow},
    },
    theme::{colors, ink, px, text},
};
use url::Url;

use super::parts;
use crate::{
    i18n::{CommonMsg, CoreMsg, t, tr},
    prefs::{Prefs, SavedCore},
    session,
    state::{HostEvent, HostState},
};

pub enum CoresEvent {
    Close,
}

/// Where connecting stands.
enum Step {
    /// Typing the address.
    Address,
    /// Asking the core who it is.
    Probing,
    /// The system doesn't trust its certificate: show the fingerprint.
    Trust { url: Url, fingerprint: String },
    /// Signing in to it.
    SignIn {
        client: CoreClient,
        name: String,
        /// How its version differs from this computer's core's.
        gap: VersionGap,
        version: String,
    },
}

pub struct CoresDialog {
    state: Entity<HostState>,
    /// Whether the core shown now is this computer's.
    on_local: bool,
    focus: FocusHandle,
    address: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    step: Step,
    busy: bool,
    error: Option<String>,
}

impl EventEmitter<CoresEvent> for CoresDialog {}

/// How another core's version differs from this computer's core's, as the dashboard's
/// version check sees it (`VersionMismatch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionGap {
    Same,
    /// Only the patch differs: everything should work.
    Patch,
    /// The major or minor version differs: some pages may not work.
    Incompatible,
}

/// Compares two core versions (`0.5.2`, `0.5.2-beta.1`). Versions that can't be read
/// count as the same: the core's requests say what fails.
pub fn version_gap(ours: &str, theirs: &str) -> VersionGap {
    fn parts(version: &str) -> Option<(u64, u64, u64)> {
        let core = version.trim().trim_start_matches('v');
        let core = core.split(['-', '+']).next()?;
        let mut numbers = core.split('.').map(|n| n.parse::<u64>().ok());
        Some((numbers.next()??, numbers.next()??, numbers.next()??))
    }
    match (parts(ours), parts(theirs)) {
        (Some(a), Some(b)) if (a.0, a.1) != (b.0, b.1) => VersionGap::Incompatible,
        (Some(a), Some(b)) if a.2 != b.2 => VersionGap::Patch,
        _ => VersionGap::Same,
    }
}

/// This computer's core's version, kept when the Cores dialog opens on it (the Host
/// always starts there), to compare other cores with.
static LOCAL_VERSION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The client for a core at `url`, accepting the certificate trusted for it.
fn client(url: Url, fingerprint: Option<&str>) -> Result<CoreClient, String> {
    let http = launcher_core::http::default_client().map_err(|e| e.to_string())?;
    let mut client = CoreClient::new(http, url.clone());
    if let Some(fingerprint) = fingerprint {
        let pin = CertificatePin::new(url.host_str().unwrap_or_default(), fingerprint);
        let pinned = PinnedHttp::new(Some(pin)).map_err(|e| e.to_string())?;
        client = client.with_pinned(Arc::new(pinned));
    }
    Ok(client)
}

/// The fingerprint saved for `address`, if the user trusted one.
fn saved_fingerprint(address: &str, cx: &App) -> Option<String> {
    Prefs::get(cx)
        .cores
        .iter()
        .find(|c| c.address == address)
        .and_then(|c| c.fingerprint.clone())
}

/// Saves `address` (with `fingerprint`, if one was trusted) in the list of cores.
fn remember(address: &str, fingerprint: Option<String>, cx: &mut App) {
    let address = address.to_string();
    Prefs::update(cx, |prefs| {
        match prefs.cores.iter_mut().find(|c| c.address == address) {
            Some(saved) => {
                if fingerprint.is_some() {
                    saved.fingerprint = fingerprint;
                }
            }
            None => prefs.cores.push(SavedCore {
                address,
                fingerprint,
            }),
        }
    });
}

impl CoresDialog {
    pub fn new(state: Entity<HostState>, on_local: bool, cx: &mut Context<Self>) -> Self {
        if on_local && let Ok(mut local) = LOCAL_VERSION.lock() {
            *local = Some(state.read(cx).info.version.clone());
        }
        let address = cx.new(|cx| {
            TextInput::new(cx)
                .with_label(t(CoreMsg::CoreAddress))
                .with_placeholder("192.168.1.5")
        });
        Self {
            state,
            on_local,
            focus: cx.focus_handle(),
            address,
            username: parts::input(t(CoreMsg::Username), false, cx),
            password: parts::input(t(CoreMsg::Password), true, cx),
            step: Step::Address,
            busy: false,
            error: None,
        }
    }

    /// Asks the core at `address` who it is, with the certificate trusted for it.
    fn probe(&mut self, address: &str, cx: &mut Context<Self>) {
        let url = match parse_address(address) {
            Ok(url) => url,
            Err(err) => {
                self.error = Some(err.to_string());
                cx.notify();
                return;
            }
        };
        let fingerprint = saved_fingerprint(&display_address(&url), cx);
        let client = match client(url.clone(), fingerprint.as_deref()) {
            Ok(client) => client,
            Err(err) => {
                self.error = Some(err);
                cx.notify();
                return;
            }
        };
        self.error = None;
        self.step = Step::Probing;
        cx.notify();
        let probe = client.clone();
        parts::spawn(
            cx,
            async move { probe.info().await },
            move |this, result, _| match result {
                Ok(info) => {
                    let ours = LOCAL_VERSION.lock().ok().and_then(|v| v.clone());
                    let gap =
                        ours.map_or(VersionGap::Same, |ours| version_gap(&ours, &info.version));
                    this.step = Step::SignIn {
                        client,
                        name: info.core_name,
                        gap,
                        version: info.version,
                    }
                }
                Err(CoreError::Untrusted {
                    fingerprint: Some(fingerprint),
                    ..
                }) => this.step = Step::Trust { url, fingerprint },
                Err(CoreError::CertificateChanged { presented, .. }) => {
                    this.error = Some(
                        CoreError::CertificateChanged {
                            host: url.host_str().unwrap_or_default().to_string(),
                            pinned: String::new(),
                            presented: presented.clone(),
                        }
                        .to_string(),
                    );
                    this.step = Step::Trust {
                        url,
                        fingerprint: presented,
                    };
                }
                Err(err) => {
                    this.error = Some(err.to_string());
                    this.step = Step::Address;
                }
            },
        );
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        let address = self.address.read(cx).text().to_string();
        self.probe(&address, cx);
    }

    fn trust(&mut self, cx: &mut Context<Self>) {
        let Step::Trust { url, fingerprint } = &self.step else {
            return;
        };
        let address = display_address(url);
        remember(&address, Some(fingerprint.clone()), cx);
        self.probe(&address, cx);
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let Step::SignIn { client, .. } = &self.step else {
            return;
        };
        if self.busy {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        if username.is_empty() || password.is_empty() {
            self.error = Some(
                t(if username.is_empty() {
                    CoreMsg::UsernameRequired
                } else {
                    CoreMsg::PasswordRequired
                })
                .to_string(),
            );
            cx.notify();
            return;
        }
        let client = client.clone();
        let address = display_address(client.base());
        self.busy = true;
        self.error = None;
        cx.notify();
        parts::spawn(
            cx,
            async move { session::sign_in_to(client, &username, &Secret::new(password)).await },
            move |this, result, cx| {
                this.busy = false;
                match result {
                    Ok(session) => {
                        remember(&address, None, cx);
                        this.switch(Some(session), cx);
                    }
                    Err(err) => this.error = Some(err.message()),
                }
            },
        );
    }

    /// Shows `session`'s core, or this computer's for `None`.
    fn switch(&mut self, session: Option<session::Session>, cx: &mut Context<Self>) {
        cx.emit(CoresEvent::Close);
        self.state.update(cx, |_, cx| {
            cx.emit(HostEvent::SwitchCore(session.map(Box::new)))
        });
    }

    fn forget(&mut self, address: String, cx: &mut Context<Self>) {
        Prefs::update(cx, |prefs| prefs.cores.retain(|c| c.address != address));
        cx.notify();
    }

    fn list(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = crate::pages::parts::current_address(&self.state, cx);
        let mut rows = vec![
            SettingRow::new(t(CoreMsg::ThisComputerCore))
                .key("core-local")
                .hint(t(CoreMsg::ThisComputerHint))
                .child(if self.on_local {
                    settings::tag(t(CoreMsg::CurrentCore), colors::GREEN).into_any_element()
                } else {
                    Button::new("use-local", t(CoreMsg::UseCore))
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.switch(None, cx)))
                        .into_any_element()
                })
                .into_any_element(),
        ];
        for (index, saved) in Prefs::get(cx).cores.clone().into_iter().enumerate() {
            let is_current = !self.on_local && current.as_deref() == Some(saved.address.as_str());
            let address = saved.address.clone();
            let forget = saved.address.clone();
            rows.push(
                SettingRow::new(saved.address.clone())
                    .key(SharedString::from(format!("core-{index}")))
                    .child(if is_current {
                        settings::tag(t(CoreMsg::CurrentCore), colors::GREEN).into_any_element()
                    } else {
                        Button::new(("use-core", index), t(CoreMsg::UseCore))
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.address.update(cx, |input, cx| {
                                    input.set_text(SharedString::from(address.clone()), cx)
                                });
                                let address = address.clone();
                                this.probe(&address, cx);
                            }))
                            .into_any_element()
                    })
                    .child(
                        Button::new(("forget-core", index), t(CoreMsg::RemoveCore))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .disabled(is_current)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.forget(forget.clone(), cx)),
                            ),
                    )
                    .into_any_element(),
            );
        }
        settings::card(rows).into_any_element()
    }

    fn step_view(&self) -> AnyElement {
        match &self.step {
            Step::Address | Step::Probing => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(text::H3)
                        .text_color(ink::text())
                        .child(t(CoreMsg::ConnectToCore)),
                )
                .child(parts::field(t(CoreMsg::CoreAddress), &self.address))
                .child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .child(t(CoreMsg::CoreAddressHint)),
                )
                .into_any_element(),
            Step::Trust { url, fingerprint } => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(text::H3)
                        .text_color(ink::text())
                        .child(t(CoreMsg::TrustTitle)),
                )
                .child(parts::paragraph(tr!(
                    CoreMsg::TrustBody,
                    host = url.host_str().unwrap_or_default()
                )))
                .child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(ink::secondary())
                        .child(t(CoreMsg::FingerprintCaption)),
                )
                .child(
                    div()
                        .p_3()
                        .rounded(lodestone_ui::theme::radius::LG)
                        .bg(colors::GRAY_900)
                        .child(parts::mono(launcher_core::cores::tls::format_fingerprint(
                            fingerprint,
                        ))),
                )
                .child(parts::paragraph(t(CoreMsg::FingerprintHelp)))
                .into_any_element(),
            Step::SignIn {
                name, gap, version, ..
            } => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(text::H3)
                        .text_color(ink::text())
                        .child(tr!(CoreMsg::SignInTo, name = name)),
                )
                .when(*gap != VersionGap::Same, |this| {
                    let ours = LOCAL_VERSION
                        .lock()
                        .ok()
                        .and_then(|v| v.clone())
                        .unwrap_or_default();
                    let incompatible = *gap == VersionGap::Incompatible;
                    let color = if incompatible {
                        colors::YELLOW
                    } else {
                        ink::muted()
                    };
                    this.child(div().text_size(text::SMALL).text_color(color).child(
                        if incompatible {
                            tr!(CoreMsg::VersionIncompatible, theirs = version, ours = ours)
                        } else {
                            tr!(CoreMsg::VersionPatch, theirs = version, ours = ours)
                        },
                    ))
                })
                .child(parts::field(t(CoreMsg::Username), &self.username))
                .child(parts::field(t(CoreMsg::Password), &self.password))
                .into_any_element(),
        }
    }
}

impl Render for CoresDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.entity().downgrade();
        let (action, label): (fn(&mut Self, &mut Context<Self>), CoreMsg) = match self.step {
            Step::Address | Step::Probing => (Self::connect, CoreMsg::Connect),
            Step::Trust { .. } => (Self::trust, CoreMsg::TrustCertificate),
            Step::SignIn { .. } => (Self::sign_in, CoreMsg::SignIn),
        };
        let loading = self.busy || matches!(self.step, Step::Probing);
        let list = self.list(cx);
        let step = self.step_view();
        Dialog::new("cores", t(CoreMsg::Cores), self.focus.clone())
            .width(px(600.))
            .on_close(move |_, cx| {
                close.update(cx, |_, cx| cx.emit(CoresEvent::Close)).ok();
            })
            .child(
                div()
                    .id("cores-body")
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            action(this, cx);
                        }
                    }))
                    .child(parts::paragraph(t(CoreMsg::CoresLead)))
                    .child(list)
                    .child(step)
                    .children(self.error.clone().map(parts::error)),
            )
            .footer(
                Button::new("cores-action", t(label))
                    .variant(ButtonVariant::Primary)
                    .loading(loading)
                    .on_click(cx.listener(move |this, _, _, cx| action(this, cx))),
            )
            .when(!matches!(self.step, Step::Address), |dialog| {
                dialog.footer(
                    Button::new("cores-back", t(CommonMsg::Back)).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.step = Step::Address;
                            this.error = None;
                            cx.notify();
                        },
                    )),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_as_the_dashboard_does() {
        assert_eq!(version_gap("0.5.2", "0.5.2"), VersionGap::Same);
        assert_eq!(version_gap("0.5.2", "0.5.3-beta.1"), VersionGap::Patch);
        assert_eq!(version_gap("0.5.2", "0.6.0"), VersionGap::Incompatible);
        assert_eq!(version_gap("0.5.2", "1.5.2"), VersionGap::Incompatible);
        assert_eq!(version_gap("v0.5.2", "0.5.2+build"), VersionGap::Same);
        // unreadable versions don't warn
        assert_eq!(version_gap("dev", "0.5.2"), VersionGap::Same);
    }
}
