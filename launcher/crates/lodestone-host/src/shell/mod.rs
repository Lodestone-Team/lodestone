//! The dashboard's layout (`components/DashboardLayout`): the top bar, the sidebar with
//! the selected instance, its tabs and every instance, and the page of the current
//! [`Route`]; the Settings open in a dialog over it ([`settings`]).

pub mod settings;
pub mod sidebar;
pub(crate) mod top_bar;

use gpui::{
    AnyView, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, prelude::*,
};
use launcher_core::cores::admin::InstanceState;
use lodestone_ui::{
    components::{
        DialogSlot, UnderDialogs,
        settings::{FindSetting, SettingsSearch},
    },
    theme::{colors, fonts, ink, layout, px, text, weights},
};

use crate::{
    i18n::{ShellMsg, t},
    pages::{
        self,
        create_instance::{CreateInstance, CreateInstanceEvent},
    },
    routes::{InstanceTab, Route, SettingsPage},
    shell::settings::SettingsModal,
    state::{HostEvent, HostState},
};

pub struct HostShell {
    pub state: Entity<HostState>,
    pub route: Route,
    page: Option<(Route, AnyView)>,
    create: DialogSlot<HostShell, Entity<CreateInstance>>,
    /// The other cores (package D), from the footer's "Change core".
    cores: DialogSlot<HostShell, Entity<pages::cores::CoresDialog>>,
    /// The Settings dialog, over the page.
    settings: DialogSlot<HostShell, SettingsModal>,
    /// The Settings' page shown last, where they open again.
    settings_page: SettingsPage,
    /// The Settings' search field, over every page: what is typed stays as the
    /// pages change.
    settings_search: Entity<SettingsSearch>,
    /// The core shown is this computer's (not another one's).
    pub on_local: bool,
    /// Asks to accept the Minecraft EULA when a server doesn't start without it.
    eula: Entity<pages::eula_prompt::EulaPrompt>,
    /// The bell in the top bar and its list (package D).
    pub notifications: Entity<crate::notifications::Notifications>,
    /// The window title last set.
    title: String,
    /// The sidebar's width in pixels at 100% zoom, dragged by its edge (the
    /// dashboard's `leftNavSize`), and whether a drag is going on.
    pub sidebar_width: f32,
    sidebar_dragging: bool,
    /// The top bar was pressed: the next mouse move drags the window.
    pub top_bar_drag_pending: bool,
    /// How tall the sidebar's selected-instance block was when last drawn, so the
    /// empty block keeps its space and nothing under it moves (the dashboard's fixed
    /// `h-[17.625rem]`).
    pub selected_block_height: Option<gpui::Pixels>,
    _subscriptions: Vec<Subscription>,
}

impl HostShell {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.check_route(cx)),
            cx.subscribe(&state, |this, _, event, cx| match event {
                HostEvent::Navigate(route) => this.navigate(route.clone(), cx),
                HostEvent::OpenSettings(page) => this.open_settings(Some(*page), cx),
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
            create: DialogSlot::new(|this: &mut HostShell| &mut this.create),
            cores: DialogSlot::new(|this: &mut HostShell| &mut this.cores),
            settings: DialogSlot::new(|this: &mut HostShell| &mut this.settings),
            settings_page: SettingsPage::General,
            settings_search: cx.new(|cx| SettingsSearch::new("settings-search", cx)),
            on_local: true,
            eula,
            notifications,
            title: String::new(),
            sidebar_width: cx
                .try_global::<crate::prefs::Prefs>()
                .and_then(|prefs| prefs.data.sidebar_width)
                .unwrap_or(layout::SIDEBAR_DEFAULT)
                .clamp(layout::SIDEBAR_MIN, layout::SIDEBAR_MAX),
            sidebar_dragging: false,
            top_bar_drag_pending: false,
            selected_block_height: None,
            _subscriptions: subscriptions,
        }
    }

    /// The sidebar's edge, dragged to `x` in a window `width` wide (window pixels).
    fn drag_sidebar(&mut self, x: f32, width: f32, cx: &mut Context<Self>) {
        let zoom = lodestone_ui::theme::zoom::get();
        let (dragged, folded) = layout::sidebar_drag(x / zoom, width / zoom);
        // the Host's sidebar doesn't fold into a rail
        let dragged = if folded { layout::SIDEBAR_MIN } else { dragged };
        if dragged != self.sidebar_width {
            self.sidebar_width = dragged;
            cx.notify();
        }
    }

    /// The drag ended: keep the width for the next start.
    fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.sidebar_dragging) {
            let width = self.sidebar_width;
            crate::prefs::Prefs::update(cx, |prefs| prefs.sidebar_width = Some(width));
        }
    }

    /// ⌘F (Ctrl+F): to the search field of the open Settings, or of the instance
    /// Settings tab shown. Elsewhere the key does nothing here.
    fn find_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_open() {
            self.settings_search
                .update(cx, |search, cx| search.focus(window, cx));
            return;
        }
        let page = self.page.as_ref().and_then(|(_, view)| {
            view.clone()
                .downcast::<pages::instance_settings::InstanceSettingsPage>()
                .ok()
        });
        match page {
            Some(page) => page.update(cx, |page, cx| page.focus_search(window, cx)),
            None => cx.propagate(),
        }
    }

    /// Shows `route`. The Settings close, so it isn't hidden under them.
    pub fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.close_settings(cx);
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
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| match event {
            pages::cores::CoresEvent::Close => {
                // not one still fading out
                if this.cores.current() == Some(&dialog) {
                    this.cores.close(cx);
                }
            }
        });
        self._subscriptions.push(subscription);
        self.cores.open(dialog, cx);
    }

    pub fn open_create(&mut self, cx: &mut Context<Self>) {
        self.open_create_with(None, cx);
    }

    /// The "New server" dialog, if it is open.
    pub fn create_dialog(&self) -> Option<Entity<CreateInstance>> {
        self.create.current().cloned()
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
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| match event {
            CreateInstanceEvent::Close => {
                // not one still fading out
                if this.create.current() == Some(&dialog) {
                    this.create.close(cx);
                }
            }
        });
        self._subscriptions.push(subscription);
        self.create.open(dialog, cx);
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
        }
    }

    /// The banner under the top bar while the core answers but its live updates
    /// stopped (the dashboard's `TopBanner`), with Reload.
    fn degraded_banner(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        div()
            .flex()
            .flex_none()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .px(px(16.))
            .py(px(6.))
            .bg(colors::YELLOW.opacity(15))
            .border_b_1()
            .border_color(colors::YELLOW.opacity(40))
            .text_size(text::MEDIUM)
            .text_color(ink::text())
            .child(div().min_w_0().child(t(ShellMsg::DegradedBanner)))
            .child(
                lodestone_ui::components::Button::new("degraded-reload", t(ShellMsg::Reload))
                    .size(lodestone_ui::components::ButtonSize::Slim)
                    .variant(lodestone_ui::components::ButtonVariant::Secondary)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        state.update(cx, |state, cx| state.reconnect(cx));
                    })),
            )
    }

    /// The window's title: the page (or the Settings' page), then the app (the
    /// dashboard's `pageTitle`).
    fn title(&self, cx: &Context<Self>) -> String {
        if let Some(page) = self.settings_open() {
            return format!("{} – {}", settings::page_name(page), crate::APP_NAME);
        }
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
        };
        match page {
            Some(page) => format!("{page} – {}", crate::APP_NAME),
            None => crate::APP_NAME.to_string(),
        }
    }
}

/// How wide a page's content may grow, as the dashboard's tabs (`max-w-4xl`, and
/// `max-w-6xl` for the console and the files).
pub fn page_max_width(route: &Route) -> Option<gpui::Pixels> {
    match route {
        Route::Home => None,
        Route::Instance {
            tab: InstanceTab::Console | InstanceTab::Files,
            ..
        } => Some(gpui::px(1152.)),
        Route::Instance { .. } => Some(gpui::px(896.)),
    }
}

/// Pages that scroll themselves, beside a list that jumps to their sections: they
/// fill the page's height instead of growing it, so the page doesn't scroll them.
pub fn page_scrolls_itself(route: &Route) -> bool {
    matches!(
        route,
        Route::Instance {
            tab: InstanceTab::Settings,
            ..
        }
    )
}

impl Render for HostShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page(cx);
        let heading = self.heading(cx);
        let error = self.state.read(cx).error.clone();
        let toasts = self.state.read(cx).toasts.clone();
        let title = self.title(cx);
        if self.title != title {
            window.set_window_title(&title);
            self.title = title;
        }
        let degraded = matches!(
            self.state.read(cx).connection,
            crate::state::Connection::Degraded(_)
        );
        let max_width = page_max_width(&self.route);
        let content = div()
            .flex_1()
            .when(page_scrolls_itself(&self.route), |this| this.min_h_0())
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
            .child(div().flex_1().min_h_0().child(page));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .on_action(
                cx.listener(|this, _: &FindSetting, window, cx| this.find_setting(window, cx)),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if !this.sidebar_dragging {
                        return;
                    }
                    if event.pressed_button != Some(gpui::MouseButton::Left) {
                        return this.end_sidebar_drag(cx);
                    }
                    let width = f32::from(window.viewport_size().width);
                    this.drag_sidebar(f32::from(event.position.x), width, cx);
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_sidebar_drag(cx)),
            )
            .child(UnderDialogs::new(
                "under-dialogs",
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(top_bar::render(self.notifications.clone(), window, cx))
                    .when(degraded, |this| this.child(self.degraded_banner(cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .child(sidebar::render(self, cx))
                            // the sidebar's edge, to drag
                            .child(
                                div()
                                    .id("sidebar-edge")
                                    .flex_none()
                                    .w(gpui::px(4.))
                                    .ml(gpui::px(-4.))
                                    .h_full()
                                    .cursor_col_resize()
                                    .hover(|style| style.bg(colors::GRAY_700))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.sidebar_dragging = true;
                                            cx.stop_propagation();
                                        }),
                                    ),
                            )
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
                    ),
            ))
            .children(
                self.create
                    .render(window, |dialog| Some(dialog.clone().into_any_element())),
            )
            .children(
                self.cores
                    .render(window, |dialog| Some(dialog.clone().into_any_element())),
            )
            .children(self.settings.render(window, |modal| {
                Some(self.render_settings(modal, window, cx))
            }))
            .child(self.eula.clone())
            .child(toasts)
            .min_w(layout::sidebar_width())
    }
}

/// Lodestone's logo (the dashboard's `Logo`), not the launcher's: a dark wordmark on
/// the light theme.
pub fn logo() -> &'static str {
    if lodestone_ui::theme::palette::is_light() {
        "images/logo_light.svg"
    } else {
        "images/host_logo.svg"
    }
}
