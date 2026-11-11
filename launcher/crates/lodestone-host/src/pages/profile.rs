//! Settings › Profile: the signed-in user's name and password, the Host's appearance
//! (language, theme, zoom, console colours), their API keys and Minecraft accounts,
//! after the dashboard's `pages/settings/profile.tsx` (`launcher/docs/host-parity.md`,
//! §7.3 and §1.8). The appearance is saved on this computer ([`crate::prefs`]).

use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
use launcher_core::settings::Language;
use lodestone_ui::{
    components::{
        Button, ButtonSize, DialogSlot, Dropdown, ListRow, Toggle,
        a11y::Accessible as _,
        settings::{
            SearchRow, SettingRow, SettingsQuery, SettingsSearch, popover_panel, select_button,
        },
        window_layer,
    },
    i18n::Locale,
    theme::{ink, px, text, zoom},
};

use super::{
    account_dialogs::{self, AccountDialog, AccountDialogEvent},
    api_keys::{self, ApiKeysSection},
    minecraft_links::{self, MinecraftLinksSection},
    parts::{self, TextSetting, TextSettingEvent},
};
use crate::{
    i18n::{CommonMsg, CoreMsg, t, tr},
    prefs::{Prefs, PrefsData, Theme},
    state::HostState,
};

const LANGUAGES: [Language; 5] = [
    Language::System,
    Language::English,
    Language::French,
    Language::SimplifiedChinese,
    Language::TraditionalChinese,
];

/// A language's name, in itself; "System default (English)" for the system's.
fn language_name(language: Language) -> String {
    match language {
        Language::System => tr!(
            CoreMsg::LanguageAuto,
            language = lodestone_ui::i18n::resolve(Language::System).native_name()
        ),
        Language::English => Locale::En.native_name().to_string(),
        Language::French => Locale::Fr.native_name().to_string(),
        Language::SimplifiedChinese => Locale::ZhHans.native_name().to_string(),
        Language::TraditionalChinese => Locale::ZhHant.native_name().to_string(),
    }
}

fn theme_name(theme: Theme) -> (&'static str, &'static str) {
    match theme {
        Theme::System => (t(CoreMsg::ThemeSystem), t(CoreMsg::ThemeSystemHint)),
        Theme::Light => (t(CoreMsg::ThemeLight), t(CoreMsg::ThemeLightHint)),
        Theme::Dark => (t(CoreMsg::ThemeDark), t(CoreMsg::ThemeDarkHint)),
    }
}

/// The Appearance dropdown that is open, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Menu {
    Language,
    Theme,
}

pub struct ProfilePage {
    pub state: Entity<HostState>,
    menu: Option<Menu>,
    username: Entity<TextSetting>,
    api_keys: Entity<ApiKeysSection>,
    links: Entity<MinecraftLinksSection>,
    /// The Settings' search field: the page shows the settings that match it.
    search: Entity<SettingsSearch>,
    dialog: DialogSlot<Self, Entity<AccountDialog>>,
    _subscriptions: Vec<Subscription>,
}

impl ProfilePage {
    pub fn new(
        state: Entity<HostState>,
        search: Entity<SettingsSearch>,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = state.read(cx).me.username.clone();
        let username = cx.new(|cx| {
            TextSetting::new(
                "profile-username",
                t(CoreMsg::ProfileUsername),
                &name,
                |name| {
                    name.is_empty()
                        .then(|| t(CoreMsg::UsernameRequired).to_string())
                },
                cx,
            )
        });
        let api_keys = cx.new(|cx| {
            let mut section = ApiKeysSection::new(state.clone(), api_keys::Owner::Me, cx);
            section.set_search(search.clone());
            section
        });
        let links = cx.new(|cx| {
            let mut section =
                MinecraftLinksSection::new(state.clone(), minecraft_links::Owner::Me, cx);
            section.set_search(search.clone());
            section
        });
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.subscribe(&username, |this, _, event, cx| {
                let TextSettingEvent::Submit(name) = event;
                this.rename(name.clone(), cx);
            }),
            // a rename elsewhere
            cx.observe(&state, |this, state, cx| {
                let name = state.read(cx).me.username.clone();
                this.username
                    .update(cx, |field, cx| field.set_value(&name, cx));
            }),
        ];
        Self {
            state,
            menu: None,
            username,
            api_keys,
            links,
            search,
            dialog: DialogSlot::new(|this: &mut Self| &mut this.dialog),
            _subscriptions: subscriptions,
        }
    }

    fn rename(&mut self, name: String, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let admin = state.admin.clone();
        let uid = state.me.uid.clone();
        let value = name.clone();
        parts::spawn(
            cx,
            async move { admin.rename_user(&uid, &value).await },
            move |this, result, cx| {
                if result.is_ok() {
                    this.state.update(cx, |state, cx| {
                        state.me.username = name.clone();
                        cx.notify();
                    });
                    parts::saved(&this.state, cx);
                }
                this.username.update(cx, |field, cx| {
                    field.finish(result.map(|()| name).map_err(|e| e.to_string()), cx)
                });
            },
        );
    }

    /// Changes the password, or for an owner without one, sets up the owner account
    /// (which also lets other devices use the core).
    fn change_password(&mut self, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let uid = state.read(cx).me.uid.clone();
        let set_up = account_dialogs::needs_owner_account(state.read(cx));
        let (dialog, subscription) = account_dialogs::open(
            move |cx| {
                if set_up {
                    AccountDialog::owner_account(state, cx)
                } else {
                    AccountDialog::change_password(state, uid, cx)
                }
            },
            |this: &mut Self, dialog, event, cx| {
                if this.dialog.current() != Some(dialog) {
                    return;
                }
                if matches!(event, AccountDialogEvent::Changed) {
                    parts::saved(&this.state, cx);
                }
                this.dialog.close(cx);
            },
            cx,
        );
        self._subscriptions.push(subscription);
        self.dialog.open(dialog, cx);
    }

    /// A button showing the current choice, which opens the list of choices. Picking
    /// one saves it at once.
    #[allow(clippy::too_many_arguments)]
    fn dropdown<T: Copy + PartialEq + 'static>(
        &self,
        menu: Menu,
        id: &'static str,
        label: &'static str,
        options: Vec<(T, SharedString)>,
        current: T,
        apply: fn(&mut PrefsData, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu == Some(menu);
        let shown = options
            .iter()
            .find(|(value, _)| *value == current)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let page = cx.entity().downgrade();
        let button = select_button(
            SharedString::from(format!("{id}-select")),
            div().truncate().child(shown.clone()),
        )
        .aria_label(label)
        .aria_value(shown)
        .min_w(px(200.))
        .expandable(open, move |_, cx| {
            page.update(cx, |this, cx| this.toggle_menu(menu, cx)).ok();
        })
        .on_press(cx.listener(move |this, _, _, cx| this.toggle_menu(menu, cx)));
        Dropdown::new(button)
            .align_end()
            .when(open, |this| {
                let mut panel =
                    popover_panel(SharedString::from(format!("{id}-list"))).min_w(px(200.));
                for (index, (value, name)) in options.into_iter().enumerate() {
                    panel = panel.child(
                        ListRow::new(SharedString::from(format!("{id}-option-{index}")), name)
                            .selected(value == current)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = None;
                                cx.notify();
                                Prefs::update(cx, |p| apply(p, value));
                            })),
                    );
                }
                let page = cx.entity().downgrade();
                this.popover(
                    SharedString::from(format!("{id}-menu")),
                    panel,
                    move |_, cx| {
                        page.update(cx, |this, cx| {
                            this.menu = None;
                            cx.notify();
                        })
                        .ok();
                    },
                )
            })
            .into_any_element()
    }

    /// For screenshots: opens the `language` or `theme` list.
    #[cfg(feature = "screenshot")]
    pub fn open_menu_for_screenshot(&mut self, name: &str, cx: &mut Context<Self>) {
        self.menu = Some(if name == "theme" {
            Menu::Theme
        } else {
            Menu::Language
        });
        cx.notify();
    }

    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        cx.notify();
    }

    fn appearance(&self, query: &SettingsQuery, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prefs = Prefs::get(cx).clone();
        let language = self.dropdown(
            Menu::Language,
            "language",
            t(CoreMsg::Language),
            LANGUAGES
                .iter()
                .map(|l| (*l, SharedString::from(language_name(*l))))
                .collect(),
            prefs.language,
            |p, language| p.language = language,
            cx,
        );
        let theme = self.dropdown(
            Menu::Theme,
            "theme",
            t(CoreMsg::Theme),
            Theme::ALL
                .iter()
                .map(|theme| (*theme, SharedString::from(theme_name(*theme).0)))
                .collect(),
            prefs.theme,
            |p, theme| p.theme = theme,
            cx,
        );
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        let zoom_value = prefs.zoom;
        let zoom_control = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                Button::new("zoom-out", "−")
                    .size(ButtonSize::Slim)
                    .accessible_label(format!("{modifier} −"))
                    .on_click(move |_, _, cx| {
                        Prefs::update(cx, |p| p.zoom = zoom::step(zoom_value, false))
                    }),
            )
            .child(
                div()
                    .w(px(56.))
                    .flex()
                    .justify_center()
                    .text_color(ink::text())
                    .child(format!("{}%", (zoom_value * 100.).round())),
            )
            .child(
                Button::new("zoom-in", "+")
                    .size(ButtonSize::Slim)
                    .accessible_label(format!("{modifier} +"))
                    .on_click(move |_, _, cx| {
                        Prefs::update(cx, |p| p.zoom = zoom::step(zoom_value, true))
                    }),
            )
            .child(
                Button::new("zoom-reset", t(CommonMsg::Reset))
                    .size(ButtonSize::Slim)
                    .disabled((zoom_value - 1.0).abs() < 1e-3)
                    .on_click(|_, _, cx| Prefs::update(cx, |p| p.zoom = 1.0)),
            );
        query
            .section(
                t(CoreMsg::Appearance),
                Some(t(CoreMsg::AppearanceLead).into()),
            )
            .card([
                SettingRow::new(t(CoreMsg::Language))
                    .search_text(language_name(prefs.language))
                    .child(language),
                SettingRow::new(t(CoreMsg::Theme))
                    .hint(theme_name(prefs.theme).1)
                    .keywords(t(CoreMsg::ThemeKeywords))
                    .child(theme),
                SettingRow::new(t(CoreMsg::Zoom))
                    .hint(tr!(CoreMsg::ZoomHint, modifier = modifier))
                    .child(zoom_control),
                SettingRow::new(t(CoreMsg::ColoredConsole)).child(
                    Toggle::new("colored-console", prefs.colored_console)
                        .label(t(CoreMsg::ColoredConsole))
                        .on_change(|value, _, cx| {
                            let value = *value;
                            Prefs::update(cx, |p| p.colored_console = value)
                        }),
                ),
            ])
            .finish()
            .map(IntoElement::into_any_element)
    }
}

impl Render for ProfilePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let no_password = account_dialogs::needs_owner_account(self.state.read(cx));
        let query = self.search.read(cx).query();
        let username = self.state.read(cx).me.username.clone();
        let account = query
            .section(t(CoreMsg::Account), None)
            .card([
                SearchRow::new(
                    self.username.clone(),
                    [t(CoreMsg::ProfileUsername), username.as_str()],
                ),
                SettingRow::new(t(CoreMsg::ProfilePassword))
                    .hint_opt(no_password.then(|| t(CoreMsg::NoPasswordYet)))
                    .child(
                        Button::new(
                            "change-password",
                            t(if no_password {
                                CoreMsg::SetPassword
                            } else {
                                CoreMsg::ChangePassword
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.change_password(cx))),
                    )
                    .into(),
            ])
            .finish()
            .map(IntoElement::into_any_element);
        let appearance = self.appearance(&query, cx);
        // the API keys and accounts are drawn by views of their own, which match
        // what is typed themselves; their count is added here for the status line
        if query.is_active() {
            query.add_found(self.api_keys.read(cx).found(&query));
            query.add_found(self.links.read(cx).found(&query));
        }
        let dialog = window_layer(
            window,
            self.dialog
                .render(window, |dialog| Some(dialog.clone().into_any_element())),
        );
        parts::page(
            query
                .status()
                .into_iter()
                .chain(account)
                .chain(appearance)
                .chain([
                    self.api_keys.clone().into_any_element(),
                    self.links.clone().into_any_element(),
                ]),
        )
        .text_size(text::MEDIUM)
        .children(dialog)
    }
}
