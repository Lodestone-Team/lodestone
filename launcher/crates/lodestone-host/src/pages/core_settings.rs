//! Settings › General: the core's name and address, what it lists publicly, its information and the danger zone, after the dashboard's
//! `pages/settings/CoreSettings.tsx` (`launcher/docs/host-parity.md`, §7.1).
//!
//! Only the owner changes these; everyone else sees them read-only. Settings an older
//! core doesn't have are hidden, as the dashboard does.
//!
//! "Allow access from other devices" is off for Lodestone Host's core until its owner
//! shares it: turning it on (or anything else that only makes sense from other devices,
//! such as the public listing) first asks for the owner account's username and password
//! when the owner has none (`account_dialogs`).

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, KeyDownEvent,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::*,
};
use launcher_core::cores::admin::{
    core_settings::{
        CORE_NAME_MAX, CoreNameProblem, CoreSwitch, DOMAIN_MAX, PortProblem, PublicListingOverview,
        PublicScopeChange, core_name_problem, domain_too_long, parse_port,
    },
    models::TlsMode,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, TextInput, Toggle,
        settings::{self, SettingRow},
        window_layer,
    },
    theme::{colors, ink, text},
};

use super::{
    account_dialogs::{self, AccountDialog, AccountDialogEvent},
    parts::{self, Confirm, TextSetting, TextSettingEvent},
};
use crate::{
    i18n::{CommonMsg, CoreMsg, t, tr},
    state::{HostEvent, HostState},
};

/// Whether turning `switch` on only makes sense from other devices, so an owner without
/// a password sets one up first: remote access itself, the public listing (anyone who
/// can reach the core sees it) and plain HTTP from the network.
pub fn shares_the_core(switch: CoreSwitch) -> bool {
    matches!(
        switch,
        CoreSwitch::AllowRemoteAccess
            | CoreSwitch::ListInstancesPublicly
            | CoreSwitch::AllowLanHttp
    )
}

/// A switch that asks first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asking {
    SafeModeOff,
    CodeExecutionOff,
    LanHttpOn,
    RemoteAccessOff,
}

struct OpenPort {
    focus: FocusHandle,
    input: Entity<TextInput>,
    error: Option<String>,
    busy: bool,
}

pub struct CoreSettingsPage {
    pub state: Entity<HostState>,
    name: Entity<TextSetting>,
    domain: Entity<TextSetting>,
    /// The settings as the core sent them, to tell which an older core lacks.
    raw: Option<serde_json::Value>,
    overview: Option<PublicListingOverview>,
    /// The instance whose listing is being changed.
    listing_saving: Option<String>,
    /// A switch being changed.
    switching: Option<CoreSwitch>,
    asking: DialogSlot<Self, (Asking, FocusHandle)>,
    open_port: DialogSlot<Self, OpenPort>,
    /// The owner account to set up before the core is shared.
    owner_account: DialogSlot<Self, Entity<AccountDialog>>,
    /// The switch to flip once the owner account is set up.
    after_owner_account: Option<(CoreSwitch, bool)>,
    _subscriptions: Vec<Subscription>,
}

impl CoreSettingsPage {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).settings.clone().unwrap_or_default();
        let address = state
            .read(cx)
            .admin
            .core()
            .base()
            .host_str()
            .unwrap_or("")
            .to_string();
        let name = cx.new(|cx| {
            TextSetting::new(
                "core-name",
                t(CoreMsg::CoreName),
                &settings.core_name,
                |name| {
                    core_name_problem(name).map(|problem| match problem {
                        CoreNameProblem::Empty => t(CoreMsg::CoreNameEmpty).to_string(),
                        CoreNameProblem::TooLong => {
                            tr!(CoreMsg::CoreNameTooLong, max = CORE_NAME_MAX)
                        }
                    })
                },
                cx,
            )
        });
        let domain = cx.new(|cx| {
            TextSetting::new(
                "core-domain",
                t(CoreMsg::Domain),
                settings.domain.as_deref().unwrap_or(""),
                |domain| {
                    domain_too_long(domain).then(|| tr!(CoreMsg::DomainTooLong, max = DOMAIN_MAX))
                },
                cx,
            )
            .with_hint(t(CoreMsg::DomainHint))
            .with_placeholder(address, cx)
        });
        let subscriptions = vec![
            cx.subscribe(&name, |this, _, event, cx| {
                let TextSettingEvent::Submit(value) = event;
                this.save_name(value.clone(), cx);
            }),
            cx.subscribe(&domain, |this, _, event, cx| {
                let TextSettingEvent::Submit(value) = event;
                this.save_domain(value.clone(), cx);
            }),
            cx.observe(&state, |this, state, cx| this.follow(&state, cx)),
            cx.subscribe(&state, |this, _, event, cx| {
                if matches!(event, HostEvent::InstancesReloaded) {
                    this.load(cx);
                }
            }),
        ];
        let mut this = Self {
            state,
            name,
            domain,
            raw: None,
            overview: None,
            listing_saving: None,
            switching: None,
            asking: DialogSlot::new(|this: &mut Self| &mut this.asking),
            open_port: DialogSlot::new(|this: &mut Self| &mut this.open_port),
            owner_account: DialogSlot::new(|this: &mut Self| &mut this.owner_account),
            after_owner_account: None,
            _subscriptions: subscriptions,
        };
        this.follow(&this.state.clone(), cx);
        this.load(cx);
        this
    }

    /// Fetches what only this page shows: which settings the core has, and the
    /// public listing (for the owner).
    fn load(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let admin = state.admin.clone();
        let owner = state.me.is_owner;
        parts::spawn(
            cx,
            async move {
                let raw = admin.global_settings_raw().await.ok();
                let overview = if owner {
                    admin.public_listing_overview().await.ok()
                } else {
                    None
                };
                (raw, overview)
            },
            |this, (raw, overview), _| {
                if raw.is_some() {
                    this.raw = raw;
                }
                this.overview = overview;
            },
        );
    }

    /// The state changed: keep the fields in step with the core.
    fn follow(&mut self, state: &Entity<HostState>, cx: &mut Context<Self>) {
        let state = state.read(cx);
        let owner = state.me.is_owner;
        let settings = state.settings.clone().unwrap_or_default();
        self.name.update(cx, |field, cx| {
            field.set_disabled(!owner, cx);
            field.set_value(&settings.core_name, cx);
        });
        self.domain.update(cx, |field, cx| {
            field.set_disabled(!owner, cx);
            field.set_value(settings.domain.as_deref().unwrap_or(""), cx);
        });
        cx.notify();
    }

    fn supports(&self, field: &str) -> bool {
        self.raw
            .as_ref()
            .is_some_and(|raw| raw.get(field).is_some_and(|v| v.is_boolean()))
    }

    fn save_name(&mut self, name: String, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let value = name.clone();
        parts::spawn(
            cx,
            async move { admin.set_core_name(&value).await },
            move |this, result, cx| {
                if result.is_ok() {
                    this.state.update(cx, |state, cx| {
                        if let Some(settings) = state.settings.as_mut() {
                            settings.core_name = name.clone();
                        }
                        state.info.core_name = name.clone();
                        cx.notify();
                    });
                    parts::saved(&this.state, cx);
                }
                this.name.update(cx, |field, cx| {
                    field.finish(result.map(|()| name).map_err(|e| e.to_string()), cx)
                });
            },
        );
    }

    fn save_domain(&mut self, domain: String, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let value = domain.clone();
        parts::spawn(
            cx,
            async move { admin.set_domain(&value).await },
            move |this, result, cx| {
                if result.is_ok() {
                    this.state.update(cx, |state, cx| {
                        if let Some(settings) = state.settings.as_mut() {
                            settings.domain = Some(domain.clone());
                        }
                        cx.notify();
                    });
                    parts::saved(&this.state, cx);
                }
                this.domain.update(cx, |field, cx| {
                    field.finish(result.map(|()| domain).map_err(|e| e.to_string()), cx)
                });
            },
        );
    }

    /// A switch was flipped: some ask first, as the dashboard does. Those that only make
    /// sense from other devices ask for the owner account first, when there is none.
    fn flip(&mut self, switch: CoreSwitch, value: bool, cx: &mut Context<Self>) {
        if value
            && shares_the_core(switch)
            && account_dialogs::needs_owner_account(self.state.read(cx))
        {
            // setting up the account turns remote access on by itself
            let then = (switch != CoreSwitch::AllowRemoteAccess).then_some((switch, value));
            self.open_owner_account(then, cx);
            return;
        }
        let ask = match (switch, value) {
            (CoreSwitch::SafeMode, false) => Some(Asking::SafeModeOff),
            (CoreSwitch::CodeExecution, false) => Some(Asking::CodeExecutionOff),
            (CoreSwitch::AllowLanHttp, true) => Some(Asking::LanHttpOn),
            (CoreSwitch::AllowRemoteAccess, false) => Some(Asking::RemoteAccessOff),
            _ => None,
        };
        match ask {
            Some(ask) => {
                let focus = cx.focus_handle();
                self.asking.open((ask, focus), cx);
            }
            None => self.set_switch(switch, value, cx),
        }
    }

    /// Asks for the owner account's username and password, then flips `then`.
    fn open_owner_account(&mut self, then: Option<(CoreSwitch, bool)>, cx: &mut Context<Self>) {
        self.after_owner_account = then;
        let state = self.state.clone();
        let (dialog, subscription) = account_dialogs::open(
            move |cx| AccountDialog::owner_account(state, cx),
            |this: &mut Self, dialog, event, cx| {
                if this.owner_account.current() != Some(dialog) {
                    return;
                }
                this.owner_account.close(cx);
                let then = this.after_owner_account.take();
                if let (AccountDialogEvent::OwnerSetUp(_), Some((switch, value))) = (event, then) {
                    this.flip(switch, value, cx);
                }
                cx.notify();
            },
            cx,
        );
        self._subscriptions.push(subscription);
        self.owner_account.open(dialog, cx);
    }

    /// Flips "Allow access from other devices" on as a user would, and with `owner`
    /// (a username and password) fills in and sends the owner account dialog it opens:
    /// for the development screenshots (`--share-core`, `--owner`).
    #[cfg(feature = "screenshot")]
    pub fn share_core_for_screenshot(
        &mut self,
        owner: Option<(&str, &str)>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.owner_account.current().is_none() {
            self.flip(CoreSwitch::AllowRemoteAccess, true, cx);
        }
        let Some(dialog) = self.owner_account.current().cloned() else {
            return false;
        };
        if let Some((username, password)) = owner {
            dialog.update(cx, |dialog, cx| {
                dialog.fill_and_submit(username, password, cx)
            });
        }
        true
    }

    fn set_switch(&mut self, switch: CoreSwitch, value: bool, cx: &mut Context<Self>) {
        if self.switching.is_some() {
            return;
        }
        self.switching = Some(switch);
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move {
                admin.set_core_switch(switch, value).await?;
                Ok::<_, launcher_core::cores::CoreError>(())
            },
            move |this, result, cx| {
                this.switching = None;
                this.asking.close(cx);
                match result {
                    Ok(()) => {
                        this.state.update(cx, |state, cx| {
                            if let Some(settings) = state.settings.as_mut() {
                                switch.set(settings, value);
                            }
                            // the core decides the final state (a lock): read it back
                            if switch == CoreSwitch::CodeExecution {
                                state.reload(cx);
                            }
                            cx.notify();
                        });
                        parts::saved(&this.state, cx);
                    }
                    Err(err) => parts::toast(&this.state, parts::Level::Error, err.to_string(), cx),
                }
            },
        );
    }

    fn change_listing(&mut self, uuid: String, change: PublicScopeChange, cx: &mut Context<Self>) {
        if self.listing_saving.is_some() {
            return;
        }
        self.listing_saving = Some(uuid.clone());
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let target = uuid.clone();
        parts::spawn(
            cx,
            async move { admin.set_public_scope(&target, &change).await },
            move |this, result, cx| {
                this.listing_saving = None;
                match result {
                    Ok(scope) => {
                        if let Some(entry) = this
                            .overview
                            .as_mut()
                            .and_then(|o| o.instances.iter_mut().find(|i| i.uuid == uuid))
                        {
                            entry.scope = scope;
                        }
                        parts::saved(&this.state, cx);
                    }
                    Err(err) => parts::toast(&this.state, parts::Level::Error, err.to_string(), cx),
                }
            },
        );
    }

    fn open_port_dialog(&mut self, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_label(t(CoreMsg::OpenPortPort))
                .with_text("25565")
        });
        let focus = cx.focus_handle();
        self.open_port.open(
            OpenPort {
                focus,
                input,
                error: None,
                busy: false,
            },
            cx,
        );
    }

    fn submit_open_port(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.open_port.current_mut() else {
            return;
        };
        if dialog.busy {
            return;
        }
        let port = match parse_port(dialog.input.read(cx).text()) {
            Ok(port) => port,
            Err(problem) => {
                dialog.error = Some(
                    t(match problem {
                        PortProblem::Required => CoreMsg::PortRequired,
                        PortProblem::NotNumber => CoreMsg::PortNotNumber,
                        PortProblem::Range => CoreMsg::PortRange,
                    })
                    .to_string(),
                );
                cx.notify();
                return;
            }
        };
        dialog.busy = true;
        dialog.error = None;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move { admin.open_port(port).await },
            move |this, result, cx| match result {
                Ok(()) => {
                    this.open_port.close(cx);
                    parts::toast(
                        &this.state,
                        parts::Level::Info,
                        tr!(CoreMsg::PortOpened, port = port),
                        cx,
                    );
                }
                // the dialog stays open with the error
                Err(err) => {
                    if let Some(dialog) = this.open_port.current_mut() {
                        dialog.busy = false;
                        dialog.error = Some(err.to_string());
                    }
                }
            },
        );
    }

    fn switch_row(
        &self,
        switch: CoreSwitch,
        label: CoreMsg,
        hint: impl Into<SharedString>,
        owner: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let settings = self.state.read(cx).settings.clone().unwrap_or_default();
        let locked = switch == CoreSwitch::CodeExecution && settings.code_execution_locked;
        SettingRow::new(t(label))
            .hint(hint)
            .child(
                Toggle::new(
                    SharedString::from(format!("{switch:?}")),
                    switch.get(&settings),
                )
                .label(t(label))
                .disabled(!owner || locked || self.switching.is_some())
                .on_change(
                    cx.listener(move |this, value: &bool, _, cx| this.flip(switch, *value, cx)),
                ),
            )
            .into_any_element()
    }

    fn info_rows(&self, cx: &App) -> Vec<AnyElement> {
        let info = &self.state.read(cx).info;
        let row = |label: CoreMsg, value: String| {
            SettingRow::new(t(label))
                .child(settings::value(value, false))
                .into_any_element()
        };
        let mut rows = vec![
            row(CoreMsg::InfoCoreVersion, info.version.clone()),
            row(
                CoreMsg::InfoHostVersion,
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            row(CoreMsg::InfoOs, info.os.clone()),
            row(CoreMsg::InfoArch, info.arch.clone()),
            row(CoreMsg::InfoCpu, info.cpu.clone()),
            row(CoreMsg::InfoCpuCores, info.cpu_count.to_string()),
            row(
                CoreMsg::InfoTotalRam,
                if info.total_ram > 0 {
                    // in binary units, as the OS counts memory: 48 GB, not 52
                    lodestone_ui::i18n::format::bytes_binary(info.total_ram)
                } else {
                    t(CoreMsg::InfoNoRam).to_string()
                },
            ),
            row(
                CoreMsg::InfoHttps,
                match info.tls.mode {
                    TlsMode::SelfSigned => t(CoreMsg::TlsSelfSigned).to_string(),
                    TlsMode::UserProvided => t(CoreMsg::TlsUserProvided).to_string(),
                    TlsMode::None | TlsMode::Unknown => t(CoreMsg::TlsNone).to_string(),
                },
            ),
        ];
        if let Some(fingerprint) = &info.tls.fingerprint_sha256 {
            rows.push(
                SettingRow::new(t(CoreMsg::InfoFingerprint))
                    .below(parts::mono(fingerprint.clone()))
                    .into_any_element(),
            );
        }
        rows
    }

    fn render_asking(
        &self,
        (asking, focus): &(Asking, FocusHandle),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (asking, focus) = (*asking, focus.clone());
        let macros = self.state.read(cx).info.capabilities.macros;
        let (switch, value, title, body, action, danger) = match asking {
            Asking::SafeModeOff => (
                CoreSwitch::SafeMode,
                false,
                CoreMsg::SafeModeOffTitle,
                CoreMsg::SafeModeOffBody,
                CoreMsg::TurnOff,
                false,
            ),
            Asking::CodeExecutionOff => (
                CoreSwitch::CodeExecution,
                false,
                CoreMsg::CodeExecutionOffTitle,
                if macros {
                    CoreMsg::CodeExecutionOffBody
                } else {
                    CoreMsg::CodeExecutionOffBodyJavaOnly
                },
                CoreMsg::TurnOff,
                true,
            ),
            Asking::LanHttpOn => (
                CoreSwitch::AllowLanHttp,
                true,
                CoreMsg::LanHttpConfirmTitle,
                CoreMsg::LanHttpConfirmBody,
                CoreMsg::LanHttpAllow,
                true,
            ),
            Asking::RemoteAccessOff => (
                CoreSwitch::AllowRemoteAccess,
                false,
                CoreMsg::RemoteAccessOffTitle,
                CoreMsg::RemoteAccessOffBody,
                CoreMsg::TurnOff,
                false,
            ),
        };
        let this = cx.entity().downgrade();
        let close = cx.entity().downgrade();
        Confirm {
            id: "core-confirm",
            title: t(title).into(),
            body: t(body).into(),
            action: t(action).into(),
            danger,
            busy: self.switching.is_some(),
        }
        .render(
            focus,
            move |_, cx| {
                this.update(cx, |this, cx| this.set_switch(switch, value, cx))
                    .ok();
            },
            move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.asking.close(cx);
                    })
                    .ok();
            },
        )
        .into_any_element()
    }

    fn render_open_port(&self, dialog: &OpenPort, cx: &mut Context<Self>) -> AnyElement {
        let close = cx.entity().downgrade();
        Dialog::new("open-port", t(CoreMsg::OpenPort), dialog.focus.clone())
            .on_close(move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.open_port.close(cx);
                    })
                    .ok();
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.submit_open_port(cx);
                        }
                    }))
                    .child(parts::field(t(CoreMsg::OpenPortPort), &dialog.input))
                    .children(dialog.error.clone().map(parts::error)),
            )
            .footer(
                Button::new("open-port-submit", t(CoreMsg::OpenPortButton))
                    .variant(ButtonVariant::Primary)
                    .loading(dialog.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.submit_open_port(cx))),
            )
            .footer(
                Button::new("open-port-cancel", t(CommonMsg::Cancel)).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.open_port.close(cx);
                    },
                )),
            )
            .into_any_element()
    }
}

impl Render for CoreSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let owner = state.me.is_owner;
        let settings = state.settings.clone().unwrap_or_default();
        let macros = state.info.capabilities.macros;
        let mut sections: Vec<AnyElement> = Vec::new();

        if !owner {
            sections.push(parts::paragraph(t(CoreMsg::ReadOnly)));
        }
        if settings.allow_lan_http {
            sections.push(parts::warning(if owner {
                tr!(CoreMsg::LanHttpWarningOwner, setting = t(CoreMsg::LanHttp))
            } else {
                t(CoreMsg::LanHttpWarningOther).to_string()
            }));
        }

        // Core
        let mut core_rows = vec![
            self.name.clone().into_any_element(),
            self.domain.clone().into_any_element(),
        ];
        if owner && self.supports("allow_remote_access") {
            core_rows.push(self.switch_row(
                CoreSwitch::AllowRemoteAccess,
                CoreMsg::RemoteAccess,
                t(if settings.allow_remote_access {
                    CoreMsg::RemoteAccessHintOn
                } else {
                    CoreMsg::RemoteAccessHintOff
                }),
                owner,
                cx,
            ));
        }
        sections.push(
            settings::section(t(CoreMsg::SectionCore), None)
                .child(settings::card(core_rows))
                .into_any_element(),
        );

        // Public servers
        if owner && self.supports("list_instances_publicly") {
            let switch_on = settings.list_instances_publicly;
            let mut rows = vec![self.switch_row(
                CoreSwitch::ListInstancesPublicly,
                CoreMsg::ListPublicly,
                t(CoreMsg::ListPubliclyHint),
                owner,
                cx,
            )];
            if let Some(overview) = &self.overview {
                if overview.instances.is_empty() {
                    rows.push(parts::empty_row(t(CoreMsg::NoJavaServers)));
                }
                for instance in &overview.instances {
                    let uuid = instance.uuid.clone();
                    let this = cx.entity().downgrade();
                    rows.push(
                        super::public_listing::scope_row(
                            SharedString::from(format!("listing-{}", instance.uuid)),
                            instance.name.clone(),
                            instance.scope,
                            switch_on,
                            true,
                            self.listing_saving.as_deref() == Some(&instance.uuid),
                            move |change, _, cx| {
                                let uuid = uuid.clone();
                                this.update(cx, |this, cx| this.change_listing(uuid, change, cx))
                                    .ok();
                            },
                        )
                        .into_any_element(),
                    );
                }
            }
            sections.push(
                settings::section(
                    t(CoreMsg::SectionPublic),
                    Some(t(CoreMsg::SectionPublicLead).into()),
                )
                .child(settings::card(rows))
                .into_any_element(),
            );
        }

        // Information
        sections.push(
            settings::section(t(CoreMsg::SectionInfo), None)
                .child(settings::card(self.info_rows(cx)))
                .into_any_element(),
        );

        // Danger zone
        let mut danger = vec![self.switch_row(
            CoreSwitch::SafeMode,
            CoreMsg::SafeMode,
            t(CoreMsg::SafeModeHint),
            owner,
            cx,
        )];
        if owner {
            let mut hint = if macros {
                t(CoreMsg::CodeExecutionHint).to_string()
            } else {
                format!(
                    "{} {}",
                    t(CoreMsg::CodeExecutionHintJavaOnly),
                    t(CoreMsg::NoScripting)
                )
            };
            if settings.code_execution_locked {
                hint = format!("{hint} {}", t(CoreMsg::CodeExecutionLocked));
            }
            danger.push(self.switch_row(
                CoreSwitch::CodeExecution,
                CoreMsg::CodeExecution,
                hint,
                owner,
                cx,
            ));
            if self.supports("allow_lan_http") {
                danger.push(self.switch_row(
                    CoreSwitch::AllowLanHttp,
                    CoreMsg::LanHttp,
                    t(CoreMsg::LanHttpHint),
                    owner,
                    cx,
                ));
            }
            if self.supports("allow_local_claim") {
                // without a password, the claim is the only way to sign in: the core
                // refuses to turn it off
                let no_password = account_dialogs::needs_owner_account(self.state.read(cx));
                let hint = if no_password {
                    format!(
                        "{} {}",
                        t(CoreMsg::LocalClaimHint),
                        t(CoreMsg::LocalClaimNeedsPassword)
                    )
                } else {
                    t(CoreMsg::LocalClaimHint).to_string()
                };
                danger.push(self.switch_row(
                    CoreSwitch::AllowLocalClaim,
                    CoreMsg::LocalClaim,
                    hint,
                    owner && !no_password,
                    cx,
                ));
            }
        }
        danger.push(
            SettingRow::new(t(CoreMsg::OpenPort))
                .hint(if owner {
                    tr!(CoreMsg::OpenPortHint, upnp = "UPnP")
                } else {
                    t(CoreMsg::OpenPortNoPermission).to_string()
                })
                .child(
                    Button::new("open-port", t(CoreMsg::OpenPortButton))
                        .variant(ButtonVariant::Danger)
                        .disabled(!owner)
                        .on_click(cx.listener(|this, _, _, cx| this.open_port_dialog(cx))),
                )
                .into_any_element(),
        );
        sections.push(
            settings::section(
                t(CoreMsg::SectionDanger),
                Some(t(CoreMsg::SectionDangerLead).into()),
            )
            .child(settings::card(danger).border_color(colors::RED_FADED.opacity(50)))
            .into_any_element(),
        );

        let mut dialogs = self
            .asking
            .render(window, |asking| Some(self.render_asking(asking, cx)));
        dialogs.extend(
            self.open_port
                .render(window, |dialog| Some(self.render_open_port(dialog, cx))),
        );
        dialogs.extend(
            self.owner_account
                .render(window, |dialog| Some(dialog.clone().into_any_element())),
        );
        parts::page(sections)
            .text_color(ink::text())
            .text_size(text::MEDIUM)
            .children(window_layer(window, dialogs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sharing_asks_for_the_owner_account() {
        assert!(shares_the_core(CoreSwitch::AllowRemoteAccess));
        assert!(shares_the_core(CoreSwitch::ListInstancesPublicly));
        assert!(shares_the_core(CoreSwitch::AllowLanHttp));
        assert!(!shares_the_core(CoreSwitch::SafeMode));
        assert!(!shares_the_core(CoreSwitch::CodeExecution));
        assert!(!shares_the_core(CoreSwitch::AllowLocalClaim));
    }
}
