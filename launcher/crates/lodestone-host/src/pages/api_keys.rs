//! API keys, after the dashboard's `components/Settings/ApiKeys`: the signed-in user's
//! own keys on the Profile (list, revoke, create), and another user's keys on their
//! page in Settings › Users (list and revoke). The key itself is shown once, in a
//! dialog only Done closes, and is never kept, logged or put in a toast.

use std::rc::Rc;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Subscription, Window, div,
    prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        EventInner, UserPermission,
        events::UserEventInner,
        users::{
            ApiKeyInfo, ApiKeyScope, CreateApiKeyRequest, CreatedApiKey, Permission,
            count_permissions,
        },
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Icon, TextInput, segmented,
        settings::{self, SettingRow, SettingsQuery, SettingsSearch},
        window_layer,
    },
    theme::{colors, fonts, ink, px, radius, text, weights},
};

use super::{
    account_dialogs::{self, AccountDialog, AccountDialogEvent},
    parts::{self, Confirm},
    permissions,
};
use crate::{
    i18n::{CommonMsg, CoreMsg, CorePlural, t, tr, trn},
    state::{HostEvent, HostState},
};

/// The longest name a key may have.
const NAME_MAX: usize = 64;

/// "Full access", "Custom: 3 servers, 5 permissions", or a launcher device's scope.
pub fn scope_summary(scope: &ApiKeyScope) -> String {
    match scope {
        ApiKeyScope::Full | ApiKeyScope::Unknown => t(CoreMsg::ScopeFull).to_string(),
        ApiKeyScope::Launcher {
            console_read,
            console_write,
        } => t(match (console_read, console_write) {
            (true, true) => CoreMsg::ScopeLauncherConsole,
            (true, false) => CoreMsg::ScopeLauncherConsoleRead,
            (false, true) => CoreMsg::ScopeLauncherConsoleWrite,
            (false, false) => CoreMsg::ScopeLauncher,
        })
        .to_string(),
        ApiKeyScope::Custom { permissions } => {
            let (granted, instances) = count_permissions(permissions);
            tr!(
                CoreMsg::ScopeCustom,
                servers = trn!(CorePlural::ScopeServers, instances),
                permissions = trn!(CorePlural::ScopePermissions, granted)
            )
        }
    }
}

/// An API key error, for people: the core refuses key management to key sessions.
pub fn error_message(err: &CoreError) -> String {
    match err {
        CoreError::Forbidden(message) if message.contains("signed-in session") => {
            t(CoreMsg::PasswordSessionOnly).to_string()
        }
        err => err.to_string(),
    }
}

/// Whose keys a section lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    /// The signed-in user's: they can create keys too.
    Me,
    /// Another user's (`uid`, `username`), for the owner.
    User(String, String),
}

struct Revoking {
    key: ApiKeyInfo,
    focus: FocusHandle,
    busy: bool,
}

/// The key just created, until its dialog is closed.
struct Created {
    key: CreatedApiKey,
    focus: FocusHandle,
}

pub struct ApiKeysSection {
    state: Entity<HostState>,
    owner: Owner,
    keys: Option<Result<Vec<ApiKeyInfo>, String>>,
    revoking: DialogSlot<Self, Revoking>,
    create: DialogSlot<Self, Entity<CreateKeyDialog>>,
    created: DialogSlot<Self, Created>,
    /// The owner account to set up before the first key: keys are for other devices
    /// and scripts, which sign in remotely.
    owner_account: DialogSlot<Self, Entity<AccountDialog>>,
    /// The Settings' search, on a page that has one: only the keys that match show.
    search: Option<Entity<SettingsSearch>>,
    _subscriptions: Vec<Subscription>,
}

/// Whether `query` finds `key`, by its name or the start of its secret.
fn key_matches(query: &SettingsQuery, key: &ApiKeyInfo) -> bool {
    query.matches([key.name.as_str(), key.display.as_str()])
}

impl ApiKeysSection {
    /// Shows only what matches `search`.
    pub fn set_search(&mut self, search: Entity<SettingsSearch>) {
        self.search = Some(search);
    }

    /// While `query` is active, how many settings it finds here: every key (or the
    /// empty list's line) when it matches the section's title, else the keys it finds.
    pub fn found(&self, query: &SettingsQuery) -> usize {
        let keys = match &self.keys {
            Some(Ok(keys)) => keys.as_slice(),
            _ => &[],
        };
        if query.matches([t(CoreMsg::ApiKeys)]) {
            return keys.len().max(1);
        }
        keys.iter().filter(|key| key_matches(query, key)).count()
    }

    pub fn new(state: Entity<HostState>, owner: Owner, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe(&state, |this, _, event, cx| {
            // a key made or revoked elsewhere
            if let HostEvent::Core(event) = event
                && let EventInner::UserEvent(user) = &event.event_inner
                && matches!(
                    user.user_event_inner,
                    UserEventInner::ApiKeyCreated { .. } | UserEventInner::ApiKeyRevoked { .. }
                )
            {
                this.load(cx);
            }
        });
        let mut this = Self {
            state,
            owner,
            keys: None,
            revoking: DialogSlot::new(|this: &mut Self| &mut this.revoking),
            create: DialogSlot::new(|this: &mut Self| &mut this.create),
            created: DialogSlot::new(|this: &mut Self| &mut this.created),
            owner_account: DialogSlot::new(|this: &mut Self| &mut this.owner_account),
            search: None,
            _subscriptions: vec![subscription],
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let owner = self.owner.clone();
        parts::spawn(
            cx,
            async move {
                match owner {
                    Owner::Me => admin.own_api_keys().await,
                    Owner::User(uid, _) => admin.user_api_keys(&uid).await,
                }
            },
            |this, result, _| this.keys = Some(result.map_err(|err| error_message(&err))),
        );
    }

    fn revoke(&mut self, cx: &mut Context<Self>) {
        let Some(revoking) = self.revoking.current_mut() else {
            return;
        };
        if revoking.busy {
            return;
        }
        revoking.busy = true;
        let key = revoking.key.clone();
        let admin = self.state.read(cx).admin.clone();
        let uid = match &self.owner {
            Owner::Me => None,
            Owner::User(uid, _) => Some(uid.clone()),
        };
        parts::spawn(
            cx,
            async move {
                admin
                    .revoke_api_key(uid.as_deref(), &key.id)
                    .await
                    .map(|()| key)
            },
            |this, result, cx| match result {
                Ok(key) => {
                    this.revoking.close(cx);
                    if let Some(Ok(keys)) = this.keys.as_mut() {
                        keys.retain(|k| k.id != key.id);
                    }
                    parts::toast(
                        &this.state,
                        parts::Level::Success,
                        tr!(CoreMsg::ApiKeyRevoked, name = key.name),
                        cx,
                    );
                }
                Err(err) => {
                    if let Some(revoking) = this.revoking.current_mut() {
                        revoking.busy = false;
                    }
                    parts::toast(&this.state, parts::Level::Error, error_message(&err), cx);
                }
            },
        );
    }

    fn open_create(&mut self, cx: &mut Context<Self>) {
        let state = self.state.clone();
        if account_dialogs::needs_owner_account(state.read(cx)) {
            let (dialog, subscription) = account_dialogs::open(
                move |cx| AccountDialog::owner_account(state, cx),
                |this: &mut Self, dialog, event, cx| {
                    if this.owner_account.current() != Some(dialog) {
                        return;
                    }
                    this.owner_account.close(cx);
                    // on to the key it was for
                    if matches!(event, AccountDialogEvent::OwnerSetUp(_)) {
                        this.open_create(cx);
                    }
                },
                cx,
            );
            self._subscriptions.push(subscription);
            self.owner_account.open(dialog, cx);
            return;
        }
        let dialog = cx.new(|cx| CreateKeyDialog::new(state, cx));
        self._subscriptions
            .push(cx.subscribe(&dialog, |this, dialog, event, cx| {
                // not one still fading out
                if this.create.current() != Some(&dialog) {
                    return;
                }
                match event {
                    CreateKeyEvent::Close => {
                        this.create.close(cx);
                    }
                    CreateKeyEvent::Created(created) => {
                        this.create.close(cx);
                        if let Some(Ok(keys)) = this.keys.as_mut() {
                            keys.retain(|k| k.id != created.info.id);
                            keys.insert(0, created.info.clone());
                        }
                        parts::toast(
                            &this.state,
                            parts::Level::Success,
                            tr!(CoreMsg::ApiKeyCreated, name = created.info.name),
                            cx,
                        );
                        let focus = cx.focus_handle();
                        this.created.open(
                            Created {
                                key: (**created).clone(),
                                focus,
                            },
                            cx,
                        );
                    }
                }
                cx.notify();
            }));
        self.create.open(dialog, cx);
    }

    fn key_row(&self, key: &ApiKeyInfo, cx: &mut Context<Self>) -> AnyElement {
        let now = chrono::Local::now().timestamp();
        let expired = key.expired(now);
        let expiry = match key.expires_at {
            None => t(CoreMsg::ExpiresNever).to_string(),
            Some(at) if expired => tr!(CoreMsg::ExpiredOn, date = parts::date(at)),
            Some(at) => tr!(CoreMsg::ExpiresOn, date = parts::date(at)),
        };
        let last_used = match key.last_used_at {
            None => t(CoreMsg::LastUsedNever).to_string(),
            Some(at) => tr!(CoreMsg::LastUsed, time = parts::ago(at)),
        };
        let revoke_key = key.clone();
        SettingRow::new(key.name.clone())
            .key(SharedString::from(format!("api-key-{}", key.id)))
            .leading(div())
            .child(
                Button::new(
                    SharedString::from(format!("revoke-{}", key.id)),
                    t(CoreMsg::Revoke),
                )
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let focus = cx.focus_handle();
                    this.revoking.open(
                        Revoking {
                            key: revoke_key.clone(),
                            focus,
                            busy: false,
                        },
                        cx,
                    );
                })),
            )
            .below(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .text_size(text::SMALL)
                    .child(
                        div()
                            .font_family(fonts::mono())
                            .text_color(ink::muted())
                            .child(key.display.clone()),
                    )
                    .child(
                        div()
                            .text_color(ink::secondary())
                            .child(scope_summary(&key.scope)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_x_3()
                            .text_color(ink::muted())
                            .child(tr!(CoreMsg::CreatedOn, date = parts::date(key.created_at)))
                            .child(
                                div()
                                    .when(expired, |this| this.text_color(colors::RED_200))
                                    .child(expiry),
                            )
                            .child(last_used),
                    ),
            )
            .into_any_element()
    }

    fn render_revoking(&self, revoking: &Revoking, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let close = cx.entity().downgrade();
        Confirm {
            id: "revoke-key",
            title: t(CoreMsg::RevokeTitle).into(),
            body: tr!(
                CoreMsg::RevokeBody,
                name = revoking.key.name,
                display = revoking.key.display
            )
            .into(),
            action: t(CoreMsg::Revoke).into(),
            danger: true,
            busy: revoking.busy,
        }
        .render(
            revoking.focus.clone(),
            move |_, cx| {
                this.update(cx, |this, cx| this.revoke(cx)).ok();
            },
            move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.revoking.close(cx);
                    })
                    .ok();
            },
        )
        .into_any_element()
    }

    fn render_created(&self, created: &Created, cx: &mut Context<Self>) -> AnyElement {
        let key = created.key.key.clone();
        let core = self.state.read(cx).admin.core().base().clone();
        let core_url = format!(
            "{}://{}",
            core.scheme(),
            core.host_str().unwrap_or("localhost")
        ) + &core.port().map(|p| format!(":{p}")).unwrap_or_default();
        let cli =
            format!("LODESTONE_URL={core_url} LODESTONE_TOKEN={key} lodestone_cli instance list");
        let state = self.state.clone();
        let code = move |id: &'static str, text_: String, copied: CoreMsg| {
            let state = state.clone();
            let copy_text = text_.clone();
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .p_3()
                .rounded(radius::LG)
                .border_1()
                .border_color(ink::border())
                .bg(colors::GRAY_900)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(fonts::mono())
                        .text_size(text::SMALL)
                        .text_color(ink::text())
                        .child(lodestone_ui::components::RichText::new(text_).selectable(true)),
                )
                .child(
                    Button::new(id, t(CommonMsg::Copy))
                        .icon(Icon::Copy)
                        .on_click(move |_, _, cx| {
                            parts::copy(&copy_text, cx);
                            parts::toast(&state, parts::Level::Success, t(copied), cx);
                        }),
                )
        };
        // only Done closes it: the key would be lost
        Dialog::new(
            "new-api-key",
            tr!(CoreMsg::NewKeyTitle, name = created.key.info.name),
            created.focus.clone(),
        )
        .width(px(600.))
        .keep_open_on_backdrop()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(text::H3)
                        .font_weight(weights::BOLD.face())
                        .text_color(colors::YELLOW)
                        .child(t(CoreMsg::NewKeyCopyNow)),
                )
                .child(code("copy-key", key, CoreMsg::KeyCopied))
                .child(parts::paragraph(t(CoreMsg::NewKeyUseWithCli)))
                .child(code("copy-cli", cli, CoreMsg::CliCopied))
                .child(parts::paragraph(t(CoreMsg::NewKeyStore))),
        )
        .footer(
            Button::new("new-key-done", t(CommonMsg::Done))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.created.close(cx);
                })),
        )
        .into_any_element()
    }
}

impl Render for ApiKeysSection {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (lead, empty) = match &self.owner {
            Owner::Me => (
                t(CoreMsg::ApiKeysOwnLead).to_string(),
                t(CoreMsg::ApiKeysOwnEmpty).to_string(),
            ),
            Owner::User(_, username) => (
                tr!(CoreMsg::ApiKeysUserLead, username = username),
                tr!(CoreMsg::ApiKeysUserEmpty, username = username),
            ),
        };
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).query())
            .unwrap_or_default();
        // the title matches (or nothing is typed): every key
        let all = query.matches([t(CoreMsg::ApiKeys)]);
        let hidden = !all && self.found(&query) == 0;
        let body: AnyElement = match &self.keys {
            None => {
                settings::card([parts::empty_row(t(CoreMsg::LoadingApiKeys))]).into_any_element()
            }
            Some(Err(err)) => parts::warning(err.clone()),
            Some(Ok(keys)) if keys.is_empty() => {
                settings::card([parts::empty_row(empty)]).into_any_element()
            }
            Some(Ok(keys)) => {
                let keys = keys.clone();
                settings::card(
                    keys.iter()
                        .filter(|key| all || key_matches(&query, key))
                        .map(|key| self.key_row(key, cx))
                        .collect::<Vec<_>>(),
                )
                .into_any_element()
            }
        };
        let header = div()
            .flex()
            .flex_row()
            .items_end()
            .justify_between()
            .gap_4()
            .child(settings::section(t(CoreMsg::ApiKeys), Some(lead.into())))
            .when(self.owner == Owner::Me, |this| {
                this.child(
                    Button::new("create-api-key", t(CoreMsg::CreateApiKey))
                        .icon(Icon::Plus)
                        .on_click(cx.listener(|this, _, _, cx| this.open_create(cx))),
                )
            });
        let mut dialogs = self
            .revoking
            .render(window, |revoking| Some(self.render_revoking(revoking, cx)));
        dialogs.extend(
            self.create
                .render(window, |dialog| Some(dialog.clone().into_any_element())),
        );
        dialogs.extend(
            self.created
                .render(window, |created| Some(self.render_created(created, cx))),
        );
        dialogs.extend(
            self.owner_account
                .render(window, |dialog| Some(dialog.clone().into_any_element())),
        );
        if hidden {
            return div().children(window_layer(window, dialogs));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(header)
            .child(body)
            .children(window_layer(window, dialogs))
    }
}

/// How long a new key lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expiry {
    Days30,
    Days90,
    Year,
    Never,
}

impl Expiry {
    const ALL: [Expiry; 4] = [Expiry::Days30, Expiry::Days90, Expiry::Year, Expiry::Never];

    fn label(self) -> &'static str {
        t(match self {
            Expiry::Days30 => CoreMsg::Expiry30d,
            Expiry::Days90 => CoreMsg::Expiry90d,
            Expiry::Year => CoreMsg::Expiry1y,
            Expiry::Never => CoreMsg::ExpiryNever,
        })
    }

    /// `expires_at` for a key made at `now` (unix seconds).
    fn expires_at(self, now: i64) -> Option<i64> {
        let days = match self {
            Expiry::Days30 => 30,
            Expiry::Days90 => 90,
            Expiry::Year => 365,
            Expiry::Never => return None,
        };
        Some(now + days * 24 * 60 * 60)
    }
}

pub enum CreateKeyEvent {
    Close,
    Created(Box<CreatedApiKey>),
}

/// The "Create API key" dialog (`CreateApiKeyDialog.tsx`).
pub struct CreateKeyDialog {
    state: Entity<HostState>,
    focus: FocusHandle,
    name: Entity<TextInput>,
    expiry: Expiry,
    custom: bool,
    permissions: UserPermission,
    error: Option<String>,
    busy: bool,
}

impl EventEmitter<CreateKeyEvent> for CreateKeyDialog {}

impl CreateKeyDialog {
    fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
            name: parts::input(t(CoreMsg::ApiKeyName), false, cx),
            expiry: Expiry::Days90,
            custom: false,
            permissions: UserPermission::default(),
            error: None,
            busy: false,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.name.read(cx).text().trim().to_string();
        self.error = if name.is_empty() {
            Some(t(CoreMsg::ApiKeyNameRequired).to_string())
        } else if name.chars().count() > NAME_MAX {
            Some(tr!(CoreMsg::ApiKeyNameTooLong, max = NAME_MAX))
        } else if self.custom && count_permissions(&self.permissions).0 == 0 {
            Some(t(CoreMsg::ChooseOne).to_string())
        } else {
            None
        };
        if self.error.is_some() {
            cx.notify();
            return;
        }
        let request = CreateApiKeyRequest {
            name,
            expires_at: self.expiry.expires_at(chrono::Local::now().timestamp()),
            scope: if self.custom {
                ApiKeyScope::Custom {
                    permissions: self.permissions.clone(),
                }
            } else {
                ApiKeyScope::Full
            },
        };
        self.busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move { admin.create_api_key(&request).await },
            |this, result, cx| {
                this.busy = false;
                match result {
                    Ok(created) => cx.emit(CreateKeyEvent::Created(Box::new(created))),
                    Err(err) => this.error = Some(error_message(&err)),
                }
            },
        );
    }

    fn scope_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let me = state.me.clone();
        let safe_mode = state.settings.as_ref().is_some_and(|s| s.safe_mode);
        let mut instances = state.instances.clone().unwrap_or_default();
        instances.sort_by(|a, b| a.name.cmp(&b.name));
        let macros = state.info.capabilities.macros;
        let this = cx.entity().downgrade();
        let on_change: permissions::OnChange =
            Rc::new(move |permission, instance, value, _, cx| {
                this.update(cx, |this, cx| {
                    match instance {
                        Some(instance) => {
                            permission.set_instance(&mut this.permissions, &instance, value)
                        }
                        None => permission.set_flag(&mut this.permissions, value),
                    }
                    cx.notify();
                })
                .ok();
            });
        let allowed = |permission: Permission, instance: Option<&str>| {
            if permission.per_instance() && instance.is_none() {
                return false;
            }
            permission.grantable_by(&me, instance)
        };
        let not_yours = t(CoreMsg::NotYours);
        let normal = permissions::rows(
            "key-scope",
            &Permission::NORMAL,
            &self.permissions,
            &instances,
            true,
            &allowed,
            Some(not_yours),
            on_change.clone(),
        );
        let unsafe_rows = permissions::rows(
            "key-scope-unsafe",
            &Permission::UNSAFE,
            &self.permissions,
            &instances,
            true,
            &allowed,
            Some(not_yours),
            on_change,
        );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(!me.is_owner, |this| {
                this.child(parts::paragraph(t(CoreMsg::OnlyYours)))
            })
            .child(settings::card(
                normal.into_iter().map(IntoElement::into_any_element),
            ))
            .child(
                div()
                    .text_size(text::H3)
                    .font_weight(weights::BOLD.face())
                    .text_color(colors::RED_200)
                    .child(t(CoreMsg::KeyUnsafe)),
            )
            .when(safe_mode, |this| {
                this.child(
                    div()
                        .text_size(text::MEDIUM)
                        .text_color(colors::RED_200)
                        .child(t(CoreMsg::KeyUnsafeSafeMode)),
                )
            })
            .child(settings::card(
                unsafe_rows.into_iter().map(IntoElement::into_any_element),
            ))
            .child(SettingRow::new(t(CoreMsg::KeyOwnerOnly)).hint(if macros {
                t(CoreMsg::KeyOwnerOnlyHint)
            } else {
                t(CoreMsg::NoScripting)
            }))
            .into_any_element()
    }
}

impl Render for CreateKeyDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.entity().downgrade();
        let this = cx.entity().downgrade();
        let expiry = segmented(
            "key-expiry",
            Expiry::ALL
                .iter()
                .map(|e| (*e, SharedString::from(e.label())))
                .collect(),
            Some(&self.expiry),
            false,
            move |value: Expiry, _, cx| {
                this.update(cx, |this, cx| {
                    this.expiry = value;
                    cx.notify();
                })
                .ok();
            },
        );
        let this = cx.entity().downgrade();
        let access = segmented(
            "key-access",
            vec![
                (false, SharedString::from(t(CoreMsg::AccessFull))),
                (true, SharedString::from(t(CoreMsg::AccessCustom))),
            ],
            Some(&self.custom),
            false,
            move |value: bool, _, cx| {
                this.update(cx, |this, cx| {
                    this.custom = value;
                    cx.notify();
                })
                .ok();
            },
        );
        let label = |text_: CoreMsg| {
            div()
                .text_size(text::MEDIUM)
                .font_weight(weights::MEDIUM.face())
                .text_color(ink::secondary())
                .child(t(text_))
        };
        let scope = self.custom.then(|| self.scope_editor(cx));
        Dialog::new(
            "create-api-key-dialog",
            t(CoreMsg::CreateApiKey),
            self.focus.clone(),
        )
        .width(px(720.))
        .on_close(move |_, cx| {
            close
                .update(cx, |_, cx| cx.emit(CreateKeyEvent::Close))
                .ok();
        })
        .child(
            div()
                .id("create-api-key-body")
                .max_h(px(560.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_4()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" {
                        this.submit(cx);
                    }
                }))
                .child(parts::paragraph(t(CoreMsg::CreateApiKeyLead)))
                .child(parts::field(t(CoreMsg::ApiKeyName), &self.name))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(label(CoreMsg::ExpiresIn))
                        .child(expiry),
                )
                .when(self.expiry == Expiry::Never, |this| {
                    this.child(parts::warning(t(CoreMsg::NeverWarning)))
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(label(CoreMsg::Access))
                        .child(access),
                )
                .children(scope)
                .children(self.error.clone().map(parts::error)),
        )
        .footer(
            Button::new("create-api-key-submit", t(CoreMsg::CreateKey))
                .variant(ButtonVariant::Primary)
                .loading(self.busy)
                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
        )
        .footer(
            Button::new("create-api-key-cancel", t(CommonMsg::Cancel))
                .disabled(self.busy)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(CreateKeyEvent::Close))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_summed_up() {
        lodestone_ui::i18n::force(lodestone_ui::i18n::Locale::En);
        assert_eq!(scope_summary(&ApiKeyScope::Full), "Full access");
        let mut permissions = UserPermission::default();
        Permission::ViewInstance.set_instance(&mut permissions, "a", true);
        Permission::StartInstance.set_instance(&mut permissions, "b", true);
        Permission::CreateInstance.set_flag(&mut permissions, true);
        let summary = scope_summary(&ApiKeyScope::Custom { permissions });
        assert!(
            summary.contains("2 servers") && summary.contains("3 permissions"),
            "{summary}"
        );
    }

    #[test]
    fn expiry_counts_days() {
        assert_eq!(Expiry::Never.expires_at(100), None);
        assert_eq!(Expiry::Days30.expires_at(0), Some(30 * 86_400));
    }

    #[test]
    fn key_sessions_get_their_own_message() {
        lodestone_ui::i18n::force(lodestone_ui::i18n::Locale::En);
        let err =
            CoreError::Forbidden("API keys can only be managed from a signed-in session".into());
        assert_eq!(error_message(&err), t(CoreMsg::PasswordSessionOnly));
    }
}
