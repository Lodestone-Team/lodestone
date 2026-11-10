//! The Settings dialog, after the dashboard's settings modal (`SettingsLayout.tsx`,
//! `SettingsLeftNav.tsx`) and the launcher's Settings: a large dialog over the page,
//! with the pages listed on the left in groups (Core, Account, Lodestone Host) and the
//! open page on the right. The page behind stays as it was; Escape, the × and a click
//! on the backdrop close it. Dialogs that a page opens (Open port, Create user, …)
//! open over it.

use gpui::{AnyElement, AnyView, Context, FocusHandle, IntoElement, Window, div, prelude::*};
use lodestone_ui::{
    components::{
        Dialog, Icon, KeyboardFocus,
        a11y::{Accessible, Role},
        settings::content_width,
    },
    theme::{self, colors, fonts, ink, leading, px, radius, text, weights},
};

use super::HostShell;
use crate::{
    i18n::{CoreMsg, ShellMsg, t},
    pages::{self, parts},
    routes::SettingsPage,
};

/// The pages, in groups with a caption (`None`: the app's name).
const GROUPS: [(Option<ShellMsg>, &[SettingsPage]); 3] = [
    (
        Some(ShellMsg::SettingsCore),
        &[SettingsPage::General, SettingsPage::Users],
    ),
    (Some(ShellMsg::SettingsAccount), &[SettingsPage::Profile]),
    (None, &[SettingsPage::Updates]),
];

/// A page's name, in the list and over the page.
pub fn page_name(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::General => t(ShellMsg::SettingsGeneral),
        SettingsPage::Users => t(ShellMsg::SettingsUsers),
        SettingsPage::Profile => t(ShellMsg::SettingsProfile),
        SettingsPage::Updates => t(CoreMsg::UpdatesTitle),
    }
}

fn page_icon(page: SettingsPage) -> Icon {
    match page {
        SettingsPage::General => Icon::Server,
        SettingsPage::Users => Icon::UserGroup,
        SettingsPage::Profile => Icon::CircleUser,
        SettingsPage::Updates => Icon::CircleUp,
    }
}

/// What the open Settings dialog keeps: its focus, and the page it shows. A dialog
/// that is closing keeps drawing the page it had.
pub struct SettingsModal {
    focus: FocusHandle,
    page: SettingsPage,
    view: AnyView,
}

impl HostShell {
    /// Opens the Settings over the page, on `page` if given, else on the page they
    /// showed last. Already open, they move to `page`.
    pub fn open_settings(&mut self, page: Option<SettingsPage>, cx: &mut Context<Self>) {
        let page = page.unwrap_or(self.settings_page);
        if self.settings.is_open() {
            self.set_settings_page(page, cx);
            return;
        }
        self.settings_page = page;
        let view = pages::open_settings(page, self.state.clone(), cx);
        let focus = cx.focus_handle();
        self.settings.open(SettingsModal { focus, page, view }, cx);
    }

    /// Shows `page` in the open Settings.
    pub fn set_settings_page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        self.settings_page = page;
        let state = self.state.clone();
        let Some(modal) = self.settings.current_mut() else {
            return;
        };
        if modal.page != page {
            modal.page = page;
            modal.view = pages::open_settings(page, state, cx);
            cx.notify();
        }
    }

    /// Closes the Settings, if they are open: they fade out over the page.
    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings.close(cx);
    }

    /// The page the open Settings show, if they are open.
    pub fn settings_open(&self) -> Option<SettingsPage> {
        self.settings.current().map(|modal| modal.page)
    }

    /// The page view the open Settings show, for the development screenshots.
    pub fn settings_view(&self) -> Option<AnyView> {
        self.settings.current().map(|modal| modal.view.clone())
    }

    pub(super) fn render_settings(
        &self,
        modal: &SettingsModal,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewport = window.viewport_size();
        let top = gpui::px(crate::TOP_BAR_HEIGHT + 1.0);
        // A window too small for the dialog to float gets it edge to edge, under the
        // top bar (the dashboard's full-window modal below 760 × 560).
        let small = viewport.width < px(760.) || viewport.height < px(560.);
        let (width, height) = if small {
            (viewport.width, viewport.height - top)
        } else {
            (
                (viewport.width - px(96.)).min(px(1080.)),
                viewport.height - px(96.),
            )
        };
        let page = modal.page;
        let entity = cx.entity().downgrade();
        Dialog::new("host-settings", t(ShellMsg::Settings), modal.focus.clone())
            .frameless(width, height)
            .when(small, |dialog| dialog.edge_to_edge(top))
            .on_close(move |_, cx| {
                entity.update(cx, |this, cx| this.close_settings(cx)).ok();
            })
            .child(self.render_settings_nav(page, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(
                        div()
                            .map(|this| {
                                if small {
                                    this.px_6().pt_5()
                                } else {
                                    this.px_10().pt_8()
                                }
                            })
                            .pb_5()
                            // clear of the dialog's ×, at the corner
                            .pr_12()
                            .child(
                                div()
                                    .id("settings-page-title")
                                    .role(Role::Heading)
                                    .aria_level(1)
                                    .aria_label(page_name(page))
                                    .accessibility_id("settings-page-title")
                                    .font_family(fonts::heading())
                                    .text_size(theme::text_size(text::H2))
                                    .line_height(theme::line_height(text::H2, leading::TIGHT))
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(page_name(page)),
                            ),
                    )
                    .child(
                        div().relative().flex_1().min_h_0().child(
                            div()
                                .id(("settings-page", page as usize))
                                .size_full()
                                .overflow_y_scroll()
                                .map(|this| if small { this.px_6() } else { this.px_10() })
                                .pb_10()
                                .child(
                                    div()
                                        .w_full()
                                        .max_w(content_width())
                                        .child(modal.view.clone()),
                                ),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// The signed-in user, the pages in their groups, and the Host's version.
    fn render_settings_nav(&self, current: SettingsPage, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let me = state.me.clone();
        let core_name = state
            .settings
            .as_ref()
            .map(|s| s.core_name.clone())
            .unwrap_or_else(|| state.info.core_name.clone());
        let mut nav = div()
            .id("settings-nav")
            .flex()
            .flex_col()
            .flex_none()
            .gap_0p5()
            .w(px(232.))
            .h_full()
            .p_3()
            .overflow_y_scroll()
            .bg(colors::GRAY_875)
            .border_r_1()
            .border_color(ink::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .pt_2()
                    .pb_3()
                    .child(parts::avatar(&me.uid, &me.username, 36.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(me.username.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(text::SMALL)
                                    .text_color(ink::muted())
                                    .child(core_name),
                            ),
                    ),
            );
        for (group, pages) in GROUPS {
            nav = nav.child(
                div()
                    .px_2p5()
                    .pt_4()
                    .pb_1()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::muted())
                    .child(group.map_or(crate::APP_NAME, t)),
            );
            for &page in pages {
                let selected = page == current;
                nav = nav.child(
                    div()
                        .id(("settings-nav", page as usize))
                        .accessible(
                            Role::Tab,
                            format!("settings-nav-{page:?}").to_lowercase(),
                            page_name(page),
                        )
                        .aria_selected(selected)
                        .keyboard_focus()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2p5()
                        .px_2p5()
                        .py_1p5()
                        .rounded(radius::LG)
                        .cursor_pointer()
                        .font_weight(weights::BOLD.face())
                        .map(|this| {
                            if selected {
                                this.bg(colors::GRAY_800).text_color(ink::text())
                            } else {
                                this.text_color(ink::secondary()).hover(|style| {
                                    style.bg(colors::GRAY_850).text_color(ink::text())
                                })
                            }
                        })
                        .child(page_icon(page).render(
                            px(13.),
                            if selected {
                                colors::GRAY_300
                            } else {
                                colors::GRAY_500
                            },
                        ))
                        .child(div().flex_1().min_w_0().child(page_name(page)))
                        .on_press(
                            cx.listener(move |this, _, _, cx| this.set_settings_page(page, cx)),
                        ),
                );
            }
        }
        nav.child(div().flex_1())
            .child(
                div()
                    .px_2p5()
                    .pt_4()
                    .text_size(theme::text_size(text::CAPTION))
                    .text_color(ink::faint())
                    .child(format!("{} {}", crate::APP_NAME, env!("CARGO_PKG_VERSION"))),
            )
            .into_any_element()
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use std::sync::Arc;

    use gpui::{Entity, TestAppContext, VisualTestContext};
    use http_client::{AsyncBody, HttpClient, Request};
    use launcher_core::{
        Secret,
        cores::{
            CoreClient,
            admin::{AdminClient, FullCoreInfo, PublicUser, models::UserPermission},
        },
    };

    use super::*;
    use crate::{routes::Route, session::Session, shell::HostShell, state::HostState};

    /// A core that never answers: every request fails at once.
    struct Offline;

    impl HttpClient for Offline {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&url::Url> {
            None
        }

        fn send(
            &self,
            _: Request<AsyncBody>,
        ) -> futures::future::BoxFuture<'static, anyhow::Result<http_client::Response<AsyncBody>>>
        {
            Box::pin(async { Err(anyhow::anyhow!("offline")) })
        }
    }

    /// The shell, signed in as the owner of a core that doesn't answer.
    fn shell(cx: &mut TestAppContext) -> (Entity<HostShell>, &mut VisualTestContext) {
        cx.update(|cx| {
            lodestone_ui::components::init(cx);
            crate::pages::init(cx);
            crate::prefs::Prefs::init_for_tests(cx);
        });
        let core = CoreClient::new(
            Arc::new(Offline),
            url::Url::parse("http://127.0.0.1:9").unwrap(),
        );
        let session = Session {
            admin: AdminClient::new(core, Secret::new(String::from("test-token"))),
            info: FullCoreInfo::default(),
            me: PublicUser {
                uid: "1".into(),
                username: "owner".into(),
                is_owner: true,
                is_admin: false,
                permissions: UserPermission::default(),
                has_password: true,
            },
        };
        let (shell, cx) = cx.add_window_view(|_, cx| {
            let state = cx.new(|cx| HostState::new(session, cx));
            HostShell::new(state, cx)
        });
        cx.run_until_parked();
        (shell, cx)
    }

    fn open_page(shell: &Entity<HostShell>, cx: &mut VisualTestContext) -> Option<SettingsPage> {
        shell.read_with(cx, |shell, _| shell.settings_open())
    }

    /// Whether the last frame drew the Settings dialog (its ×).
    fn drawn(cx: &mut VisualTestContext) -> bool {
        cx.debug_bounds("dialog-close").is_some()
    }

    #[test]
    fn settings_open_as_a_dialog_over_the_page_on_the_page_asked_for() {
        let mut cx = TestAppContext::single();
        let (shell, cx) = shell(&mut cx);
        assert!(!drawn(cx));

        // as a link to a setting does, through the state
        let state = shell.read_with(cx, |shell, _| shell.state.clone());
        state.update(cx, |state, cx| {
            state.open_settings(SettingsPage::Updates, cx)
        });
        cx.run_until_parked();
        assert_eq!(open_page(&shell, cx), Some(SettingsPage::Updates));
        assert!(drawn(cx), "a dialog");
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.route.clone()),
            Route::Home,
            "the page behind stays"
        );

        // the list on the left moves between pages, in the same dialog
        shell.update(cx, |shell, cx| {
            shell.set_settings_page(SettingsPage::Users, cx)
        });
        cx.run_until_parked();
        assert_eq!(open_page(&shell, cx), Some(SettingsPage::Users));
        let view = shell
            .read_with(cx, |shell, _| shell.settings_view())
            .unwrap();
        assert!(view.downcast::<crate::pages::users::UsersPage>().is_ok());

        // asked for again while open: the same dialog, on that page
        state.update(cx, |state, cx| {
            state.open_settings(SettingsPage::Profile, cx)
        });
        cx.run_until_parked();
        assert_eq!(open_page(&shell, cx), Some(SettingsPage::Profile));
    }

    #[test]
    fn escape_closes_the_settings_and_they_fade_out() {
        let mut cx = TestAppContext::single();
        let (shell, cx) = shell(&mut cx);
        shell.update(cx, |shell, cx| {
            shell.open_settings(Some(SettingsPage::General), cx)
        });
        cx.run_until_parked();
        assert!(drawn(cx));

        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(open_page(&shell, cx), None, "Escape closes them");
        assert!(drawn(cx), "still drawn while they fade out");
        cx.executor().advance_clock(lodestone_ui::components::EXIT);
        cx.run_until_parked();
        assert!(!drawn(cx), "gone after");

        // they open again on the page shown last
        shell.update(cx, |shell, cx| {
            shell.open_settings(Some(SettingsPage::Profile), cx);
            shell.close_settings(cx);
            shell.open_settings(None, cx);
        });
        assert_eq!(open_page(&shell, cx), Some(SettingsPage::Profile));
    }

    #[test]
    fn going_to_another_page_closes_the_settings() {
        let mut cx = TestAppContext::single();
        let (shell, cx) = shell(&mut cx);
        shell.update(cx, |shell, cx| {
            shell.open_settings(Some(SettingsPage::General), cx);
            shell.navigate(Route::Home, cx);
        });
        assert_eq!(open_page(&shell, cx), None);
    }
}
