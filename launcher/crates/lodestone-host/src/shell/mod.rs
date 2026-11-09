//! The dashboard's layout (`components/DashboardLayout`): the top bar, the sidebar with
//! the selected instance, its tabs and every instance, and the page of the current
//! [`Route`].

pub mod sidebar;
mod top_bar;

use gpui::{
    AnyView, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, prelude::*,
};
use launcher_core::cores::admin::InstanceState;
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
    /// The other cores (package D), from the footer's "Change core".
    cores: Option<Entity<pages::cores::CoresDialog>>,
    /// The core shown is this computer's (not another one's).
    pub on_local: bool,
    /// Asks to accept the Minecraft EULA when a server doesn't start without it.
    eula: Entity<pages::eula_prompt::EulaPrompt>,
    /// The bell in the top bar and its list (package D).
    pub notifications: Entity<crate::notifications::Notifications>,
    /// The window title last set.
    title: String,
    _subscriptions: Vec<Subscription>,
}

impl HostShell {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.check_route(cx)),
            cx.subscribe(&state, |this, _, event, cx| match event {
                HostEvent::Navigate(route) => this.navigate(route.clone(), cx),
                HostEvent::OpenCreateInstance => this.open_create(cx),
                HostEvent::OpenCreateModpack(preset) => {
                    this.open_create_with(Some(preset.clone()), cx)
                }
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
            cores: None,
            on_local: true,
            eula,
            notifications,
            title: String::new(),
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

    /// The other cores, from the footer.
    pub fn open_cores(&mut self, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let on_local = self.on_local;
        let dialog = cx.new(|cx| pages::cores::CoresDialog::new(state, on_local, cx));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| match event {
            pages::cores::CoresEvent::Close => {
                this.cores = None;
                cx.notify();
            }
        });
        self._subscriptions.push(subscription);
        self.cores = Some(dialog);
        cx.notify();
    }

    pub fn open_create(&mut self, cx: &mut Context<Self>) {
        self.open_create_with(None, cx);
    }

    /// The "New server" dialog, if it is open.
    pub fn create_dialog(&self) -> Option<Entity<CreateInstance>> {
        self.create.clone()
    }

    /// The "New server" dialog, on a Modrinth modpack when `preset` is given.
    pub fn open_create_with(
        &mut self,
        preset: Option<crate::pages::create_instance::ModpackPreset>,
        cx: &mut Context<Self>,
    ) {
        let state = self.state.clone();
        let dialog = cx.new(|cx| {
            let mut dialog = CreateInstance::new(state, cx);
            if let Some(preset) = preset {
                dialog.open_modpack_preset(preset, cx);
            }
            dialog
        });
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

    /// The page's heading, as `InstanceTabs.tsx` and the settings show it: none on the
    /// Overview (its heading is the server's name), and the server's state beside the
    /// Console's.
    fn heading(&self, cx: &Context<Self>) -> Option<(String, Option<InstanceState>)> {
        match &self.route {
            Route::Home => None,
            Route::Instance { uuid, tab } => {
                if *tab == InstanceTab::Overview {
                    return None;
                }
                let state = self.state.read(cx);
                let instance = state.instance(uuid)?;
                let label = (*tab == InstanceTab::Console).then_some(instance.state);
                Some((sidebar::tab_name(*tab, instance).to_string(), label))
            }
            Route::Settings(page) => Some((settings_name(*page).to_string(), None)),
        }
    }

    /// The window's title: the page, then the app (the dashboard's `pageTitle`).
    fn title(&self, cx: &Context<Self>) -> String {
        let page = match &self.route {
            Route::Home => None,
            Route::Instance { uuid, tab } => {
                let state = self.state.read(cx);
                state.instance(uuid).map(|instance| {
                    if *tab == InstanceTab::Overview {
                        instance.name.clone()
                    } else {
                        format!("{} – {}", sidebar::tab_name(*tab, instance), instance.name)
                    }
                })
            }
            Route::Settings(page) => Some(settings_name(*page).to_string()),
        };
        match page {
            Some(page) => format!("{page} – {}", crate::APP_NAME),
            None => crate::APP_NAME.to_string(),
        }
    }
}

fn settings_name(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::General => t(ShellMsg::SettingsGeneral),
        SettingsPage::Users => t(ShellMsg::SettingsUsers),
        SettingsPage::Profile => t(ShellMsg::SettingsProfile),
        SettingsPage::Updates => t(crate::i18n::CoreMsg::UpdatesTitle),
    }
}

/// How wide a page's content may grow, as the dashboard's tabs (`max-w-4xl`, and
/// `max-w-6xl` for the console and the files) and its settings.
pub fn page_max_width(route: &Route) -> Option<gpui::Pixels> {
    match route {
        Route::Home => None,
        Route::Instance {
            tab: InstanceTab::Console | InstanceTab::Files,
            ..
        } => Some(gpui::px(1152.)),
        Route::Instance { .. } => Some(gpui::px(896.)),
        Route::Settings(_) => None,
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
                            t(crate::i18n::CoreMsg::UpdatesTitle),
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
        let title = self.title(cx);
        if self.title != title {
            window.set_window_title(&title);
            self.title = title;
        }
        let max_width = page_max_width(&self.route);
        let content = div()
            .flex_1()
            .w_full()
            .when_some(max_width, |this, width| this.max_w(width))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(16.))
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
            .when_some(heading, |this, (heading, state)| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .font_family(fonts::heading())
                                .text_size(text::XLARGE_2)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::text())
                                .child(heading),
                        )
                        .when_some(state, |this, state| {
                            let color = sidebar::state_color(state);
                            this.child(
                                div()
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded(lodestone_ui::theme::radius::MD)
                                    .bg(color.opacity(15))
                                    .text_size(text::MEDIUM)
                                    .font_weight(weights::MEDIUMBOLD.face())
                                    .text_color(color)
                                    .child(sidebar::state_name(state)),
                            )
                        }),
                )
            })
            .children(settings_tabs)
            .child(div().flex_1().min_h_0().child(page));
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
                            .bg(colors::GRAY_800)
                            .child(content),
                    ),
            )
            .when_some(self.create.clone(), |this, dialog| this.child(dialog))
            .when_some(self.cores.clone(), |this, dialog| {
                this.child(div().absolute().inset_0().child(dialog))
            })
            .child(self.eula.clone())
            .child(toasts)
            .min_w(layout::sidebar_width())
    }
}
