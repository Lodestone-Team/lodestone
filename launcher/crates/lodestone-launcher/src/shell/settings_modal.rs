//! The launcher's Settings: a large dialog over the window, with the account and the
//! pages listed on the left and the open page on the right. Other dialogs, such as
//! signing in, open over it.

use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, SharedString, Window, div, prelude::*, px,
};
use launcher_core::accounts::AccountKind;

use super::{LauncherShell, account_box, developer};
use crate::components::KeyboardFocus;
use crate::{
    accounts::SessionState,
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Icon, IconButton, ListRow, Toggle,
        settings::{
            CONTENT_WIDTH, SettingRow, card, left_overlay, link, muted_link, popover,
            popover_panel, section, select_button, tag, unsaved_bar,
        },
    },
    i18n::{self, CommonMsg, Language, ShellMsg, ShellMsg as M, t, tr},
    theme::{self, colors, fonts, leading, radius, text, weights},
    updater::UpdateState,
};

/// A page of the Settings dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    Defaults,
    Shared,
    General,
    Accounts,
    Developer,
}

impl SettingsPage {
    /// The pages, in groups with a caption.
    const GROUPS: [(M, &'static [SettingsPage]); 2] = [
        (
            M::Instances,
            &[SettingsPage::Defaults, SettingsPage::Shared],
        ),
        (
            M::GroupLauncher,
            &[
                SettingsPage::General,
                SettingsPage::Accounts,
                SettingsPage::Developer,
            ],
        ),
    ];

    pub fn title(self) -> &'static str {
        match self {
            SettingsPage::Defaults => t(M::InstanceDefaults),
            SettingsPage::Shared => t(M::SharedGameSettings),
            SettingsPage::General => t(M::General),
            SettingsPage::Accounts => t(M::Accounts),
            SettingsPage::Developer => t(M::Developer),
        }
    }

    fn lead(self) -> Option<&'static str> {
        match self {
            SettingsPage::Defaults => Some(t(M::DefaultsLead)),
            SettingsPage::Shared => Some(t(M::SharedLead)),
            SettingsPage::General | SettingsPage::Accounts | SettingsPage::Developer => None,
        }
    }

    fn icon(self) -> Icon {
        match self {
            SettingsPage::Defaults => Icon::Cube,
            SettingsPage::Shared => Icon::Copy,
            SettingsPage::General => Icon::Bars,
            SettingsPage::Accounts => Icon::User,
            SettingsPage::Developer => Icon::Gear,
        }
    }
}

/// What the open Settings dialog keeps.
pub struct SettingsModal {
    pub focus: FocusHandle,
}

impl LauncherShell {
    /// Opens Settings, at `page` if given, else at the page it showed last.
    pub fn open_settings(
        &mut self,
        page: Option<SettingsPage>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        if let Some(page) = page {
            self.settings_page = page;
        }
        if !self.settings_modal.is_open() {
            let focus = cx.focus_handle();
            if let Some(window) = window {
                window.focus(&focus, cx);
            }
            self.settings_modal.open(SettingsModal { focus }, cx);
        }
        cx.notify();
    }

    /// Changes Instance defaults without saving, for the offscreen screenshots.
    pub fn preview_unsaved_defaults(&mut self, cx: &mut Context<Self>) {
        self.launcher_settings
            .update(cx, |settings, cx| settings.preview_change(cx));
    }

    /// Opens the pages over the page in a narrow window, for the offscreen screenshots.
    pub fn preview_settings_nav_open(&mut self) {
        self.settings_nav_open = true;
    }

    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_modal.close(cx);
    }

    pub fn settings_open(&self) -> bool {
        self.settings_modal.is_open()
    }

    pub(super) fn render_settings(
        &self,
        modal: &SettingsModal,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewport = window.viewport_size();
        let top = px(super::top_bar::TOP_BAR_HEIGHT + 1.0);
        // A window too small for the dialog to float gets it edge to edge, under the
        // top bar.
        let small = viewport.width < px(760.) || viewport.height < px(560.);
        // Too narrow for the sidebar: the pages open over the page from a button.
        let narrow = viewport.width < px(760.);
        let nav_overlay = (narrow && self.settings_nav_open).then(|| {
            left_overlay(
                "settings-overlay",
                self.render_settings_sidebar(true, cx),
                cx.listener(|this, _, _, cx| {
                    this.settings_nav_open = false;
                    cx.notify();
                }),
            )
        });
        let (width, height) = if small {
            (viewport.width, viewport.height - top)
        } else {
            (
                (viewport.width - px(96.)).min(px(1080.)),
                viewport.height - px(96.),
            )
        };
        let page = self.settings_page;
        let entity = cx.entity();
        let toast = match page {
            SettingsPage::Defaults => self.launcher_settings.read(cx).toast().render(),
            SettingsPage::Shared => self.shared_settings.read(cx).toast().render(),
            _ => None,
        }
        .map(IntoElement::into_any_element);
        // Unsaved changes to Instance defaults show a bar with Save and Discard on
        // every page, instead of the toast.
        let unsaved = {
            let settings = self.launcher_settings.read(cx);
            settings.has_changes(cx).then(|| settings.can_save(cx))
        };
        let toast = match unsaved {
            Some(can_save) => Some(
                unsaved_bar(
                    "settings-unsaved",
                    if page == SettingsPage::Defaults {
                        t(CommonMsg::UnsavedChanges)
                    } else {
                        t(M::UnsavedInDefaults)
                    },
                    can_save,
                    cx.listener(|this, _, _, cx| {
                        this.launcher_settings
                            .update(cx, |settings, cx| settings.discard(cx))
                    }),
                    cx.listener(|this, _, _, cx| {
                        this.launcher_settings.update(cx, |settings, cx| {
                            settings.save(cx);
                        })
                    }),
                )
                .into_any_element(),
            ),
            None => toast,
        };
        let content: AnyElement = match page {
            SettingsPage::Defaults => self.launcher_settings.clone().into_any_element(),
            SettingsPage::Shared => self.shared_settings.clone().into_any_element(),
            SettingsPage::General => self.render_general_page(cx).into_any_element(),
            SettingsPage::Accounts => self.render_accounts_page(cx).into_any_element(),
            SettingsPage::Developer => developer::render(self, cx).into_any_element(),
        };
        Dialog::new("launcher-settings", t(M::Settings), modal.focus.clone())
            .frameless(width, height)
            .when(small, |dialog| dialog.edge_to_edge(top))
            .on_close(move |window, cx| {
                entity.update(cx, |this, cx| this.request_close_settings(Some(window), cx))
            })
            .when(!narrow, |dialog| {
                dialog.child(self.render_settings_sidebar(false, cx))
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap_4()
                            .map(|this| {
                                if small {
                                    this.px_6().pt_5()
                                } else {
                                    this.px_10().pt_8()
                                }
                            })
                            .pb_5()
                            .when(narrow, |this| {
                                this.child(IconButton::new("settings-menu", Icon::Bars).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.settings_nav_open = !this.settings_nav_open;
                                        cx.notify();
                                    }),
                                ))
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .max_w(CONTENT_WIDTH)
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_family(fonts::HEADING)
                                            .text_size(theme::text_size(text::H2))
                                            .line_height(theme::line_height(
                                                text::H2,
                                                leading::TIGHT,
                                            ))
                                            .font_weight(weights::BOLD.into())
                                            .text_color(colors::GRAY_300)
                                            .child(page.title()),
                                    )
                                    .children(page.lead().map(|lead| {
                                        div()
                                            .text_size(theme::text_size(text::SMALL))
                                            .line_height(theme::line_height(
                                                text::SMALL,
                                                leading::NORMAL,
                                            ))
                                            .text_color(colors::GRAY_500)
                                            .child(lead)
                                    })),
                            )
                            .child(div().flex_1())
                            .child(IconButton::new("close-settings", Icon::Xmark).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.request_close_settings(Some(window), cx)
                                }),
                            )),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .id(("settings-page", page as usize))
                                    .size_full()
                                    .overflow_y_scroll()
                                    .map(|this| if small { this.px_6() } else { this.px_10() })
                                    .pb_24()
                                    .child(div().w_full().max_w(CONTENT_WIDTH).child(content)),
                            )
                            .children(toast),
                    )
                    .children(nav_overlay),
            )
            .into_any_element()
    }

    /// The account in use, the pages and the version.
    fn render_settings_sidebar(
        &self,
        in_overlay: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let accounts = self.accounts.read(cx);
        let active = accounts.active().cloned();
        let avatar = account_box::avatar(accounts, active.as_ref(), px(36.));
        let name = active
            .as_ref()
            .map_or_else(|| t(M::NoAccount).to_string(), |a| a.username.clone());
        let current = self.settings_page;
        let mut sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_0p5()
            .map(|this| {
                if in_overlay {
                    this.w_full()
                } else {
                    this.w(px(232.))
                        .h_full()
                        .p_3()
                        .bg(colors::GRAY_875)
                        .border_r_1()
                        .border_color(colors::FADE_700)
                }
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .pt_2()
                    .pb_3()
                    .child(avatar)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(name),
                            )
                            .child(div().flex().child(
                                muted_link("manage-accounts", t(M::ManageAccounts)).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.settings_page = SettingsPage::Accounts;
                                        this.settings_nav_open = false;
                                        cx.notify();
                                    }),
                                ),
                            )),
                    ),
            );
        for (group, pages) in SettingsPage::GROUPS {
            sidebar = sidebar.child(
                div()
                    .px_2p5()
                    .pt_4()
                    .pb_1()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::GRAY_500)
                    .child(t(group)),
            );
            for &page in pages {
                let selected = page == current;
                sidebar = sidebar.child(
                    div()
                        .id(("settings-nav", page as usize))
                        .keyboard_focus()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2p5()
                        .px_2p5()
                        .py_1p5()
                        .rounded(radius::LG)
                        .cursor_pointer()
                        .font_weight(weights::BOLD.into())
                        .map(|this| {
                            if selected {
                                this.bg(colors::GRAY_800).text_color(colors::GRAY_300)
                            } else {
                                this.text_color(colors::GRAY_400).hover(|style| {
                                    style.bg(colors::GRAY_850).text_color(colors::GRAY_300)
                                })
                            }
                        })
                        .child(page.icon().render(
                            px(13.),
                            if selected {
                                colors::GRAY_300
                            } else {
                                colors::GRAY_500
                            },
                        ))
                        // Long titles wrap rather than lose their end.
                        .child(div().flex_1().min_w_0().child(page.title()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_page = page;
                            this.settings_nav_open = false;
                            cx.notify();
                        })),
                );
            }
        }
        sidebar.child(div().flex_1()).child(
            div()
                .px_2p5()
                .text_size(theme::text_size(text::CAPTION))
                .text_color(colors::GRAY_FADED.opacity(30))
                .child(tr!(M::AppVersion, version = env!("CARGO_PKG_VERSION"))),
        )
    }

    /// Settings > General: the language, and "Group instances automatically". Both
    /// apply at once.
    fn render_general_page(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let instances = self.instances.read(cx);
        let auto_groups = instances.auto_groups();
        let language = instances.language();
        let current = match language {
            Language::System => tr!(
                ShellMsg::LanguageSystemIs,
                language = i18n::locale().native_name()
            ),
            other => i18n::resolve(other).native_name().to_string(),
        };
        let language_row = SettingRow::new(t(ShellMsg::Language)).child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    select_button(
                        "language",
                        div().font_weight(weights::BOLD.into()).child(current),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.language_menu_open = !this.language_menu_open;
                        cx.notify();
                    })),
                )
                .when(self.language_menu_open, |this| {
                    let close = cx.listener(|this, _: &(), _, cx| {
                        this.language_menu_open = false;
                        cx.notify();
                    });
                    this.child(popover(
                        "language-popover",
                        self.render_language_list(language, cx),
                        move |window, cx| close(&(), window, cx),
                    ))
                }),
        );
        let general = card([
            language_row.into_any_element(),
            SettingRow::new(t(M::GroupAutomatically))
                .hint(t(M::GroupAutomaticallyHint))
                .child(
                    Toggle::new("auto-groups", auto_groups).on_change(cx.listener(
                        |this, value: &bool, _, cx| {
                            let on = *value;
                            this.instances
                                .update(cx, |instances, cx| instances.set_auto_groups(on, cx));
                        },
                    )),
                )
                .into_any_element(),
        ]);
        div()
            .flex()
            .flex_col()
            .gap_8()
            .w_full()
            .child(general)
            .child(section(t(M::UpdatesSection), None).child(self.render_updates(cx)))
    }

    /// Settings > General > Updates: "Update automatically", and the running build
    /// with where updating stands and what can be done about it.
    fn render_updates(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let auto_update = self.instances.read(cx).auto_update();
        let updater = self.updater.read(cx);
        let state = updater.state().clone();
        let build = updater.build();
        let can_restart = updater.can_restart(cx);
        let label = match build {
            Some(build) => match build.short_commit() {
                Some(commit) => tr!(M::BuildNumberCommit, build = build.number, commit = commit),
                None => tr!(M::BuildNumber, build = build.number),
            },
            None => t(M::DevelopmentBuild).to_string(),
        };
        let (status, failed): (SharedString, bool) = match &state {
            UpdateState::Unavailable => (t(M::UpdatesUnavailable).into(), false),
            UpdateState::Idle => (t(M::UpdateNotChecked).into(), false),
            UpdateState::Checking => (t(M::UpdateChecking).into(), false),
            UpdateState::UpToDate => (t(M::UpToDate).into(), false),
            UpdateState::Available(manifest) => (
                tr!(M::UpdateAvailable, build = manifest.build).into(),
                false,
            ),
            UpdateState::Downloading { build, done, total } => (
                tr!(
                    M::UpdateDownloading,
                    build = build,
                    percent = i18n::format::percent(UpdateState::percent(done, *total))
                )
                .into(),
                false,
            ),
            UpdateState::Ready { build } if can_restart => {
                (tr!(M::UpdateReady, build = build).into(), false)
            }
            UpdateState::Ready { build } => {
                (tr!(M::UpdateReadyPlaying, build = build).into(), false)
            }
            UpdateState::Failed(error) => (tr!(M::UpdateFailed, error = error).into(), true),
        };
        let action = match &state {
            UpdateState::Unavailable => None,
            UpdateState::Available(_) => Some(
                Button::new("update-download", t(M::DownloadUpdate))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.updater
                            .update(cx, |updater, cx| updater.download_available(cx))
                    })),
            ),
            UpdateState::Ready { .. } => Some(
                Button::new("update-restart", t(M::RestartToUpdate))
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Slim)
                    .disabled(!can_restart)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.updater.update(cx, |updater, cx| updater.restart(cx))
                    })),
            ),
            UpdateState::Idle
            | UpdateState::Checking
            | UpdateState::UpToDate
            | UpdateState::Downloading { .. }
            | UpdateState::Failed(_) => Some(
                Button::new("update-check", t(M::CheckForUpdates))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .loading(matches!(state, UpdateState::Checking))
                    .disabled(matches!(
                        state,
                        UpdateState::Checking | UpdateState::Downloading { .. }
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.updater.update(cx, |updater, cx| updater.check(cx))
                    })),
            ),
        };
        let status_row = SettingRow::new(label)
            .map(|row| {
                if failed {
                    row.note(status, colors::RED_200)
                } else {
                    row.hint(status)
                }
            })
            .when_some(action, |row, action| row.child(action));
        card([
            SettingRow::new(t(M::UpdateAutomatically))
                .hint(t(M::UpdateAutomaticallyHint))
                .child(
                    Toggle::new("auto-update", auto_update)
                        .disabled(build.is_none())
                        .on_change(cx.listener(|this, value: &bool, _, cx| {
                            let on = *value;
                            this.instances
                                .update(cx, |instances, cx| instances.set_auto_update(on, cx));
                            this.updater
                                .update(cx, |updater, cx| updater.automatic_changed(cx));
                        })),
                )
                .into_any_element(),
            status_row.into_any_element(),
        ])
    }

    /// The Language list: System, then each language in its own name.
    fn render_language_list(
        &self,
        selected: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut list = popover_panel("language-list")
            .w(px(240.))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.language_menu_open = false;
                cx.notify();
            }));
        for (index, language) in Language::ALL.into_iter().enumerate() {
            let label: SharedString = match language {
                Language::System => t(ShellMsg::LanguageSystem).into(),
                other => i18n::resolve(other).native_name().into(),
            };
            let row = ListRow::new(("language-choice", index), label)
                .selected(language == selected)
                .when(language == Language::System, |row| {
                    row.detail(i18n::resolve(Language::System).native_name())
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.language_menu_open = false;
                    this.instances
                        .update(cx, |instances, cx| instances.set_language(language, cx));
                }));
            list = list.child(row);
        }
        list
    }

    fn render_accounts_page(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let accounts = self.accounts.read(cx);
        let active_id = accounts.active().map(|a| a.id.clone());
        let offline_enabled = accounts.offline_enabled();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, account) in accounts.accounts().iter().enumerate() {
            let in_use = active_id.as_deref() == Some(account.id.as_str());
            let needs_sign_in = matches!(
                accounts.session(&account.id),
                Some(SessionState::NeedsSignIn)
            );
            let kind = match account.kind {
                AccountKind::Offline => t(M::OfflineAccount),
                AccountKind::Microsoft => t(M::MicrosoftAccount),
            };
            let (switch_id, remove_id) = (account.id.clone(), account.id.clone());
            rows.push(
                SettingRow::new(account.username.clone())
                    .leading(account_box::avatar(accounts, Some(account), px(28.)))
                    .hint(kind)
                    .when(account.uses_launcher_identity(), |row| {
                        row.child(tag(t(M::LauncherIdentity), colors::BLUE_200))
                    })
                    .when(needs_sign_in, |row| {
                        row.child(
                            link(("account-sign-in", index), t(M::SignInAgain)).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_sign_in(Some(window), cx)
                                }),
                            ),
                        )
                    })
                    .map(|row| {
                        if in_use {
                            row.child(tag(t(M::InUse), colors::GREEN_300))
                        } else {
                            row.child(link(("account-use", index), t(M::Use)).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.accounts
                                        .update(cx, |accounts, cx| accounts.switch(&switch_id, cx));
                                }),
                            ))
                        }
                    })
                    .child(
                        muted_link(("account-remove", index), t(CommonMsg::Remove)).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.confirm_remove(&remove_id, Some(window), cx)
                            }),
                        ),
                    )
                    .into_any_element(),
            );
        }
        rows.push(
            SettingRow::new(t(M::AddAnAccount))
                .child(
                    Button::new("settings-add-microsoft", t(M::AddMicrosoftEllipsis))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_sign_in(Some(window), cx)),
                        ),
                )
                .when(offline_enabled, |row| {
                    row.child(
                        Button::new("settings-add-offline", t(M::AddOfflineEllipsis))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_offline(None, Some(window), cx)
                            })),
                    )
                })
                .into_any_element(),
        );
        card(rows)
    }
}
