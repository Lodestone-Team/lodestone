//! Settings › Profile: the signed-in user's name and password, the Host's appearance
//! (language, theme, zoom, console colours), their API keys and Minecraft accounts,
//! after the dashboard's `pages/settings/profile.tsx` (`launcher/docs/host-parity.md`,
//! §7.3 and §1.8). The appearance is saved on this computer ([`crate::prefs`]).

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div,
};
use launcher_core::settings::Language;
use lodestone_ui::{
    components::{
        Button, ButtonSize, DialogSlot, Toggle, segmented,
        settings::{self, SettingRow},
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
    prefs::{Prefs, Theme},
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

pub struct ProfilePage {
    pub state: Entity<HostState>,
    username: Entity<TextSetting>,
    api_keys: Entity<ApiKeysSection>,
    links: Entity<MinecraftLinksSection>,
    dialog: DialogSlot<Self, Entity<AccountDialog>>,
    _subscriptions: Vec<Subscription>,
}

impl ProfilePage {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
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
        let api_keys = cx.new(|cx| ApiKeysSection::new(state.clone(), api_keys::Owner::Me, cx));
        let links =
            cx.new(|cx| MinecraftLinksSection::new(state.clone(), minecraft_links::Owner::Me, cx));
        let subscriptions = vec![
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
            username,
            api_keys,
            links,
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

    fn change_password(&mut self, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let uid = state.read(cx).me.uid.clone();
        let (dialog, subscription) = account_dialogs::open(
            move |cx| AccountDialog::change_password(state, uid, cx),
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

    fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let prefs = Prefs::get(cx).clone();
        let language = segmented(
            "language",
            LANGUAGES
                .iter()
                .map(|l| (*l, SharedString::from(language_name(*l))))
                .collect(),
            Some(&prefs.language),
            false,
            |language: Language, _, cx: &mut App| Prefs::update(cx, |p| p.language = language),
        );
        let theme = segmented(
            "theme",
            Theme::ALL
                .iter()
                .map(|theme| (*theme, SharedString::from(theme_name(*theme).0)))
                .collect(),
            Some(&prefs.theme),
            false,
            |theme: Theme, _, cx: &mut App| Prefs::update(cx, |p| p.theme = theme),
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
        settings::section(
            t(CoreMsg::Appearance),
            Some(t(CoreMsg::AppearanceLead).into()),
        )
        .child(settings::card([
            SettingRow::new(t(CoreMsg::Language))
                .below(language)
                .into_any_element(),
            SettingRow::new(t(CoreMsg::Theme))
                .hint(theme_name(prefs.theme).1)
                .below(theme)
                .into_any_element(),
            SettingRow::new(t(CoreMsg::Zoom))
                .hint(tr!(CoreMsg::ZoomHint, modifier = modifier))
                .child(zoom_control)
                .into_any_element(),
            SettingRow::new(t(CoreMsg::ColoredConsole))
                .child(
                    Toggle::new("colored-console", prefs.colored_console)
                        .label(t(CoreMsg::ColoredConsole))
                        .on_change(|value, _, cx| {
                            let value = *value;
                            Prefs::update(cx, |p| p.colored_console = value)
                        }),
                )
                .into_any_element(),
        ]))
        .into_any_element()
    }
}

impl Render for ProfilePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let account = settings::section(t(CoreMsg::Account), None)
            .child(settings::card([
                self.username.clone().into_any_element(),
                SettingRow::new(t(CoreMsg::ProfilePassword))
                    .child(
                        Button::new("change-password", t(CoreMsg::ChangePassword))
                            .on_click(cx.listener(|this, _, _, cx| this.change_password(cx))),
                    )
                    .into_any_element(),
            ]))
            .into_any_element();
        let dialog = window_layer(
            window,
            self.dialog
                .render(window, |dialog| Some(dialog.clone().into_any_element())),
        );
        parts::page([
            account,
            self.appearance(cx),
            self.api_keys.clone().into_any_element(),
            self.links.clone().into_any_element(),
        ])
        .text_size(text::MEDIUM)
        .children(dialog)
    }
}
