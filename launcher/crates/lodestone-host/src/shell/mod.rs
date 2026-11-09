//! The dashboard's layout (`components/DashboardLayout`): the top bar, the sidebar with
//! the selected instance, its tabs and every instance, and the page of the current
//! [`Route`].

mod sidebar;
mod top_bar;

use gpui::{
    AnyView, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, prelude::*,
};
use lodestone_ui::{
    components::Tabs,
    theme::{colors, fonts, ink, layout, px, text, weights},
};

use crate::{
    i18n::{ShellMsg, t},
    pages::{
        self,
        create_instance::{CreateInstance, CreateInstanceEvent},
    },
    routes::{InstanceTab, Route, SettingsPage},
    state::{HostEvent, HostState},
};

pub struct HostShell {
    pub state: Entity<HostState>,
    pub route: Route,
    page: Option<(Route, AnyView)>,
    create: Option<Entity<CreateInstance>>,
    /// Asks to accept the Minecraft EULA when a server doesn't start without it.
    eula: Entity<pages::eula_prompt::EulaPrompt>,
    /// The bell in the top bar and its list (package D).
    pub notifications: Entity<crate::notifications::Notifications>,
    _subscriptions: Vec<Subscription>,
}

impl HostShell {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.check_route(cx)),
            cx.subscribe(&state, |this, _, event, cx| match event {
                HostEvent::Navigate(route) => this.navigate(route.clone(), cx),
                HostEvent::OpenCreateInstance => this.open_create(cx),
                _ => {}
            }),
        ];
        let eula = {
            let state = state.clone();
            cx.new(|cx| pages::eula_prompt::EulaPrompt::new(state, cx))
        };
        let notifications = {
            let state = state.clone();
            cx.new(|cx| crate::notifications::Notifications::new(state, cx))
        };
        Self {
            state,
            route: Route::Home,
            page: None,
            create: None,
            eula,
            notifications,
            _subscriptions: subscriptions,
        }
    }

    pub fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        if self.route != route {
            self.route = route;
            cx.notify();
        }
    }

    /// Selects an instance, on its Overview.
    pub fn select(&mut self, uuid: &str, cx: &mut Context<Self>) {
        self.navigate(
            Route::Instance {
                uuid: uuid.to_string(),
                tab: InstanceTab::Overview,
            },
            cx,
        );
    }

    pub fn open_create(&mut self, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let dialog = cx.new(|cx| CreateInstance::new(state, cx));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| match event {
            CreateInstanceEvent::Close => {
                this.create = None;
                cx.notify();
            }
        });
        self._subscriptions.push(subscription);
        self.create = Some(dialog);
        cx.notify();
    }

    /// Back to Home when the selected instance is gone (deleted, or no longer visible
    /// to the user), as the dashboard's `InstanceViewLayout` does.
    fn check_route(&mut self, cx: &mut Context<Self>) {
        if let Route::Instance { uuid, .. } = &self.route {
            let state = self.state.read(cx);
            if state.instances.is_some() && state.instance(uuid).is_none() {
                self.route = Route::Home;
            }
        }
        cx.notify();
    }

    /// The page for the route, made again when the route changes.
    fn page(&mut self, cx: &mut Context<Self>) -> AnyView {
        if let Some((route, view)) = &self.page
            && *route == self.route
        {
            return view.clone();
        }
        let view = pages::open(&self.route, self.state.clone(), cx);
        self.page = Some((self.route.clone(), view.clone()));
        view
    }

    /// The page shown, for the development screenshots.
    pub fn page_view(&self) -> Option<AnyView> {
        self.page.as_ref().map(|(_, view)| view.clone())
    }

    /// The page's heading, as `InstanceTabs.tsx` and the settings show it.
    fn heading(&self, cx: &Context<Self>) -> Option<String> {
        match &self.route {
            Route::Home => None,
            Route::Instance { uuid, tab } => {
                let state = self.state.read(cx);
                let instance = state.instance(uuid)?;
                Some(sidebar::tab_name(*tab, instance).to_string())
            }
            Route::Settings(page) => Some(
                match page {
                    SettingsPage::General => t(ShellMsg::SettingsGeneral),
                    SettingsPage::Users => t(ShellMsg::SettingsUsers),
                    SettingsPage::Profile => t(ShellMsg::SettingsProfile),
                }
                .to_string(),
            ),
        }
    }
}

impl Render for HostShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page(cx);
        let heading = self.heading(cx);
        let settings_tabs = match &self.route {
            Route::Settings(current) => {
                let selected = SettingsPage::ALL
                    .iter()
                    .position(|page| page == current)
                    .unwrap_or(0);
                let this = cx.entity().downgrade();
                Some(
                    Tabs::new(
                        "settings-pages",
                        [
                            t(ShellMsg::SettingsGeneral),
                            t(ShellMsg::SettingsUsers),
                            t(ShellMsg::SettingsProfile),
                        ],
                    )
                    .selected(selected)
                    .on_select(move |index, _, cx| {
                        let page = SettingsPage::ALL[*index];
                        this.update(cx, |this, cx| this.navigate(Route::Settings(page), cx))
                            .ok();
                    }),
                )
            }
            _ => None,
        };
        let error = self.state.read(cx).error.clone();
        let toasts = self.state.read(cx).toasts.clone();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(top_bar::render(self.notifications.clone(), window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(sidebar::render(self, cx))
                    .child(
                        div()
                            .id("main")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .px(px(40.))
                            .py(px(32.))
                            .gap(px(16.))
                            .bg(colors::GRAY_800)
                            .when_some(error, |this, error| {
                                this.child(
                                    div()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(lodestone_ui::theme::radius::MD)
                                        .bg(colors::RED_FADED.opacity(25))
                                        .text_size(text::MEDIUM)
                                        .text_color(colors::RED_200)
                                        .child(error),
                                )
                            })
                            .when_some(heading, |this, heading| {
                                this.child(
                                    div()
                                        .font_family(fonts::heading())
                                        .text_size(text::XLARGE_2)
                                        .font_weight(weights::BOLD.face())
                                        .text_color(ink::text())
                                        .child(heading),
                                )
                            })
                            .children(settings_tabs)
                            .child(div().flex_1().min_h_0().child(page)),
                    ),
            )
            .when_some(self.create.clone(), |this, dialog| this.child(dialog))
            .child(self.eula.clone())
            .child(toasts)
            .min_w(layout::sidebar_width())
    }
}
