//! Settings > Developer: what the command line may do (the control socket, its
//! access, traces and game logs) and which programs are connected; "Sign in as the
//! Minecraft Launcher", and the dialogs that explain it before it is turned on and
//! offer to sign accounts out when it is turned off.

use super::{LauncherShell, Modal};
use crate::theme::px;
use crate::{components::segmented, theme::colors};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Icon, Toggle,
        settings::{SettingRow, card, section},
    },
    i18n::{CommonMsg, ShellMsg as M, ShellPlural as P, format, t, tr, trn},
    theme::{self, fonts, leading, radius, text, weights},
};
use gpui::ClipboardItem;
use gpui::{AnyElement, Context, FocusHandle, IntoElement, SharedString, Window, div, prelude::*};
use launcher_core::settings::{CliAccess, CliSettings};

/// The disclosure shown before the option is turned on, as (heading, paragraph).
pub fn disclosure() -> Vec<(&'static str, SharedString)> {
    vec![
        (t(M::WhatThisDoes), t(M::WhatThisDoesBody).into()),
        (t(M::Risks), t(M::RisksBody).into()),
        (t(M::WhyHere), t(M::WhyHereBody).into()),
        (
            t(M::WhenSignIn),
            tr!(M::WhenSignInBody, add = t(M::AddMicrosoftAccount)).into(),
        ),
        (
            t(M::WhatLauncherGets),
            tr!(
                M::WhatLauncherGetsBody,
                keychain = t(CommonMsg::KeychainInFull),
                badge = t(M::LauncherIdentity),
            )
            .into(),
        ),
        (t(M::HowToUndo), t(M::HowToUndoBody).into()),
    ]
}

fn paragraph(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(text.into())
}

/// Settings > Developer: the command line's settings and connected programs, then
/// "Sign in as the Minecraft Launcher".
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let enabled = shell.accounts.read(cx).launcher_identity_enabled();
    let sign_in = card([SettingRow::new(t(M::SignInAsLauncher))
        .hint(t(M::SignInAsLauncherHint))
        .child(
            Toggle::new("launcher-identity", enabled)
                .label(t(M::SignInAsLauncher))
                .on_change(cx.listener(|this, value: &bool, window, cx| {
                    this.request_launcher_identity(*value, window, cx)
                })),
        )
        .into_any_element()]);
    let accounts = shell.accounts.read(cx);
    let (offline_on, offline_forced) = (accounts.offline_enabled(), accounts.offline_forced());
    let offline = card([SettingRow::new(t(M::OfflineAccountsSetting))
        .hint(if offline_forced {
            SharedString::from(t(M::OfflineAccountsAlwaysOn))
        } else {
            tr!(M::OfflineAccountsSettingHint, add = t(M::AddOfflineAccount)).into()
        })
        .child(
            Toggle::new("offline-accounts", offline_on)
                .label(t(M::OfflineAccountsSetting))
                .disabled(offline_forced)
                .on_change(cx.listener(|this, value: &bool, _, cx| {
                    let on = *value;
                    let saved = this
                        .accounts
                        .update(cx, |accounts, cx| accounts.set_offline_accounts(on, cx));
                    this.show_save_error(saved, cx);
                })),
        )
        .into_any_element()]);
    div()
        .flex()
        .flex_col()
        .gap_8()
        .w_full()
        .child(
            section(t(M::CommandLineSection), Some(t(M::CommandLineLead).into()))
                .child(command_line(shell, cx)),
        )
        .child(
            section(
                t(M::CodingAgentsSection),
                Some(t(M::CodingAgentsLead).into()),
            )
            .child(coding_agents(shell, cx)),
        )
        .children(clients(cx))
        .child(
            section(t(M::DiagnosticsSection), None).child(card([SettingRow::new(t(
                M::PerfMonitor,
            ))
            .hint(t(M::PerfMonitorHint))
            .child(
                Button::new("open-perf-monitor", t(M::PerfOpen))
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let sources = this.perf_sources();
                        super::perf_monitor::open(sources, cx);
                    })),
            )
            .into_any_element()])),
        )
        .child(section(t(M::SignInSection), None).child(sign_in))
        .child(section(t(M::Accounts), None).child(offline))
        .child(section(t(M::ConnectSection), None).child(shell.connect_service.clone()))
        .child(
            section(t(M::FirstTimeSetupSection), None).child(card([SettingRow::new(t(
                M::ResetFirstTimeSetup,
            ))
            .hint(t(M::ResetFirstTimeSetupHint))
            .child(
                Button::new("reset-first-time-setup", t(CommonMsg::Reset))
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.instances
                            .update(cx, |instances, cx| instances.reset_first_time_setup(cx));
                        this.undo_toast.show(
                            t(M::FirstTimeSetupReset),
                            false,
                            |this: &mut LauncherShell| &mut this.undo_toast,
                            cx,
                        );
                    })),
            )
            .into_any_element()])),
        )
}

/// Settings > Developer > Command line.
fn command_line(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> AnyElement {
    let cli = shell.instances.read(cx).cli_settings();
    let status = crate::control::status(cx).map(|status| status.read(cx).clone());
    let socket = status
        .as_ref()
        .map(|status| status.socket.clone())
        .unwrap_or_else(|| {
            launcher_control::paths::socket_path(shell.accounts.read(cx).data_dir().root())
        });
    let note = status.as_ref().and_then(|status| {
        status.error.as_ref().map(|error| {
            (
                SharedString::from(tr!(M::SocketError, error = error)),
                colors::RED_200,
            )
        })
    });
    let set = |change: fn(&mut CliSettings, bool)| {
        cx.listener(move |this, value: &bool, _, cx| {
            let mut cli = this.instances.read(cx).cli_settings();
            change(&mut cli, *value);
            this.instances
                .update(cx, |instances, cx| instances.set_cli_settings(cli, cx));
        })
    };
    let root = shell.accounts.read(cx).data_dir().root().to_path_buf();
    let path_text: SharedString = socket.display().to_string().into();
    let socket_hint: SharedString = if cli.enabled {
        path_text.clone()
    } else {
        format!("{} · {}", t(M::SocketClosed), path_text).into()
    };
    card([
        SettingRow::new(t(M::AllowCli))
            .hint(t(M::AllowCliHint))
            .note_opt(note)
            .child(
                Toggle::new("cli-enabled", cli.enabled).on_change(set(|cli, on| cli.enabled = on)),
            )
            .into_any_element(),
        SettingRow::new(t(M::CliAccess))
            .hint(t(M::CliAccessHint))
            .child(segmented(
                "cli-access",
                vec![
                    (CliAccess::Full, t(M::CliFullControl).into()),
                    (CliAccess::ReadOnly, t(M::CliReadOnly).into()),
                ],
                Some(&cli.access),
                !cli.enabled,
                {
                    let instances = shell.instances.clone();
                    move |access, _, cx| {
                        instances.update(cx, |instances, cx| {
                            let mut cli = instances.cli_settings();
                            cli.access = access;
                            instances.set_cli_settings(cli, cx);
                        });
                    }
                },
            ))
            .into_any_element(),
        SettingRow::new(t(M::DebugTraces))
            .hint(t(M::DebugTracesHint))
            .child(Toggle::new("cli-traces", cli.traces).on_change(set(|cli, on| cli.traces = on)))
            .into_any_element(),
        SettingRow::new(t(M::CliGameLogs))
            .hint(t(M::CliGameLogsHint))
            .child(
                Toggle::new("cli-game-logs", cli.game_logs)
                    .on_change(set(|cli, on| cli.game_logs = on)),
            )
            .into_any_element(),
        SettingRow::new(t(M::SocketPath))
            .hint(socket_hint)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("cli-copy-socket", t(M::CopyPath))
                            .size(ButtonSize::Slim)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    path_text.to_string(),
                                ))
                            }),
                    )
                    .child(
                        Button::new("cli-open-data", t(M::OpenDataFolder))
                            .size(ButtonSize::Slim)
                            .on_click(move |_, _, cx| cx.open_with_system(&root)),
                    ),
            )
            .into_any_element(),
    ])
    .into_any_element()
}

/// Settings > Developer > Connected programs, while the socket is open.
fn clients(cx: &mut Context<LauncherShell>) -> Option<AnyElement> {
    let status = crate::control::status(cx)?.read(cx).clone();
    if !status.running {
        return None;
    }
    let today = chrono::Local::now().date_naive();
    let rows: Vec<AnyElement> = if status.clients.is_empty() {
        vec![SettingRow::new(t(M::NoClients)).into_any_element()]
    } else {
        status
            .clients
            .iter()
            .map(|client| {
                let name: SharedString = client
                    .name
                    .clone()
                    .unwrap_or_else(|| t(M::UnnamedClient).to_string())
                    .into();
                let mut facts = Vec::new();
                if let Some(pid) = client.pid {
                    facts.push(tr!(M::ClientPid, pid = pid));
                }
                facts.push(tr!(
                    M::ClientSince,
                    time = format::day_and_time(chrono::DateTime::from(client.since), today)
                ));
                if client.subscriptions > 0 {
                    facts.push(trn!(P::Subscriptions, client.subscriptions));
                }
                let connection = client.connection;
                SettingRow::new(name)
                    .hint(facts.join(" · "))
                    .child(
                        Button::new(
                            SharedString::from(format!("cli-disconnect-{connection}")),
                            t(M::Disconnect),
                        )
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Danger)
                        .on_click(move |_, _, cx| crate::control::disconnect(connection, cx)),
                    )
                    .into_any_element()
            })
            .collect()
    };
    Some(
        section(t(M::ConnectedClients), None)
            .child(card(rows))
            .into_any_element(),
    )
}

impl LauncherShell {
    /// Settings > Developer is on screen.
    pub fn shows_developer_settings(&self) -> bool {
        self.settings_modal.current().is_some()
            && self.settings_page == super::SettingsPage::Developer
    }

    /// What the performance monitor reads.
    pub fn perf_sources(&self) -> crate::perf::Sources {
        crate::perf::Sources {
            instances: self.instances.clone(),
            accounts: self.accounts.clone(),
            content: Some(self.content.clone()),
            worlds: Some(self.worlds.clone()),
        }
    }

    /// Says in the window's toast why a developer setting couldn't be saved.
    fn show_save_error(&mut self, saved: Result<(), SharedString>, cx: &mut Context<Self>) {
        if let Err(message) = saved {
            self.undo_toast.show(
                message,
                true,
                |this: &mut LauncherShell| &mut this.undo_toast,
                cx,
            );
        }
    }

    /// Turning the option on asks first; turning it off asks what to do with the
    /// accounts that used it, if there are any.
    fn request_launcher_identity(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let focus = cx.focus_handle();
        if enabled {
            window.focus(&focus, cx);
            self.modal.open(Modal::EnableLauncherIdentity { focus }, cx);
        } else {
            let usernames: Vec<String> = self
                .accounts
                .read(cx)
                .launcher_identity_accounts()
                .into_iter()
                .map(|account| account.username)
                .collect();
            if usernames.is_empty() {
                let saved = self
                    .accounts
                    .update(cx, |accounts, cx| accounts.set_launcher_identity(false, cx));
                self.show_save_error(saved, cx);
            } else {
                window.focus(&focus, cx);
                self.modal
                    .open(Modal::DisableLauncherIdentity { focus, usernames }, cx);
            }
        }
        cx.notify();
    }

    /// Opens the disclosure, for the offscreen screenshots.
    pub fn preview_enable_launcher_identity(&mut self, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.modal.open(Modal::EnableLauncherIdentity { focus }, cx);
    }

    pub(super) fn render_enable_launcher_identity(
        &self,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        Dialog::new(
            "enable-launcher-identity",
            t(M::SignInAsLauncherTitle),
            focus,
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
        .child(
            div()
                .id("launcher-identity-disclosure")
                .flex()
                .flex_col()
                .gap_3()
                .max_h(px(340.))
                .overflow_y_scroll()
                .pr_2()
                .children(disclosure().into_iter().map(|(heading, body)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(theme::text_size(text::MEDIUM))
                                .font_weight(weights::BOLD.into())
                                .text_color(crate::theme::ink::text())
                                .child(heading),
                        )
                        .child(paragraph(body))
                })),
        )
        .footer(
            Button::new("confirm-launcher-identity", t(M::TurnItOn))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| {
                    let saved = this
                        .accounts
                        .update(cx, |accounts, cx| accounts.set_launcher_identity(true, cx));
                    this.close_modal(cx);
                    this.show_save_error(saved, cx);
                })),
        )
        .footer(
            Button::new("cancel-launcher-identity", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
        )
        .into_any_element()
    }

    pub(super) fn render_disable_launcher_identity(
        &self,
        focus: FocusHandle,
        usernames: &[String],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let accounts = match usernames {
            [one] => tr!(M::OneLauncherAccount, name = one),
            many => trn!(P::LauncherAccounts, many.len(), names = format::list(many)),
        };
        Dialog::new(
            "disable-launcher-identity",
            t(M::TurnOffLauncherTitle),
            focus,
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(paragraph(t(M::TurnOffLauncherBody)))
                .child(paragraph(tr!(
                    M::TurnOffLauncherAccounts,
                    accounts = accounts,
                    keychain = t(CommonMsg::Keychain),
                ))),
        )
        .footer(
            Button::new("disable-and-sign-out", t(M::TurnOffAndSignOut))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(|this, _, _, cx| {
                    let saved = this.accounts.update(cx, |accounts, cx| {
                        let saved = accounts.set_launcher_identity(false, cx);
                        accounts.sign_out_launcher_identity_accounts(cx);
                        saved
                    });
                    this.close_modal(cx);
                    this.show_save_error(saved, cx);
                })),
        )
        .footer(
            Button::new("disable-keep-accounts", t(M::TurnOffOnly))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| {
                    let saved = this
                        .accounts
                        .update(cx, |accounts, cx| accounts.set_launcher_identity(false, cx));
                    this.close_modal(cx);
                    this.show_save_error(saved, cx);
                })),
        )
        .footer(
            Button::new("cancel-disable-launcher-identity", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
        )
        .into_any_element()
    }
}

/// Settings > Developer > Coding agents: where the skills are, and the prompt that sets
/// an agent up with the command line.
fn coding_agents(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> AnyElement {
    let data = shell.accounts.read(cx).data_dir().clone();
    let skills = crate::agent_skills::dir(&data);
    let custom = std::env::var_os("LODESTONE_LAUNCHER_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(|_| data.root().to_path_buf());
    let prompt: SharedString = crate::agent_skills::prompt(
        &skills,
        launcher_core::update::Build::current(),
        custom.as_deref(),
    )
    .into();
    let copied = shell.agent_prompt_copied;
    let skills_text: SharedString = skills.display().to_string().into();
    card([
        SettingRow::new(t(M::AgentSkills))
            .hint(skills_text)
            .child(
                Button::new("open-agent-skills", t(CommonMsg::OpenFolder))
                    .size(ButtonSize::Slim)
                    .on_click(move |_, _, cx| cx.open_with_system(&skills)),
            )
            .into_any_element(),
        SettingRow::new(t(M::AgentPrompt))
            .hint(t(M::AgentPromptHint))
            .child(
                Button::new(
                    "copy-agent-prompt",
                    if copied {
                        t(CommonMsg::Copied)
                    } else {
                        t(M::CopyPrompt)
                    },
                )
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Slim)
                .icon(if copied { Icon::Check } else { Icon::Copy })
                .on_click({
                    let prompt = prompt.clone();
                    cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(prompt.to_string()));
                        this.agent_prompt_copied = true;
                        cx.notify();
                        this.agent_prompt_reset = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(2))
                                .await;
                            this.update(cx, |this, cx| {
                                this.agent_prompt_copied = false;
                                cx.notify();
                            })
                            .ok();
                        }));
                    })
                }),
            )
            .into_any_element(),
        // The prompt itself, to read (and select) before pasting it.
        div()
            .px_6()
            .pb_5()
            .child(
                div()
                    .p_3()
                    .rounded(radius::LG)
                    .bg(colors::GRAY_900)
                    .border_1()
                    .border_color(crate::theme::ink::border())
                    .font_family(fonts::mono())
                    .text_size(theme::text_size(text::SMALL))
                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                    .text_color(crate::theme::ink::secondary())
                    .whitespace_normal()
                    .child(crate::components::RichText::new(prompt).id("agent-prompt-text")),
            )
            .into_any_element(),
    ])
    .into_any_element()
}
