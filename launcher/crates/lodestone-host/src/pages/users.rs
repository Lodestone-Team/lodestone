//! Settings › Users: the core's accounts and what each may do, after the dashboard's
//! `pages/settings/UserSettings.tsx` and `components/UserBox.tsx`
//! (`launcher/docs/host-parity.md`, §7.2).
//!
//! The owner sees every user (`GET /user/list`), creates and deletes them, changes
//! their passwords and permissions, and sees their API keys and Minecraft accounts.
//! Everyone else sees that only the owner manages users.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, div, prelude::*,
};
use launcher_core::cores::admin::{
    EventInner, PublicUser, events::UserEventInner, users::Permission,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, DialogSlot, Dropdown, Icon, IconButton,
        settings::{self, SettingRow, SettingsQuery, SettingsSearch},
        window_layer,
    },
    theme::{colors, ink, px, text, weights},
};

use super::{
    account_dialogs::{self, AccountDialog, AccountDialogEvent},
    api_keys::{self, ApiKeysSection},
    minecraft_links::{self, MinecraftLinksSection},
    parts::{self, Confirm},
    permissions,
};
use crate::{
    i18n::{CommonMsg, CoreMsg, t, tr},
    state::{HostEvent, HostState},
};

struct Deleting {
    user: PublicUser,
    focus: FocusHandle,
    busy: bool,
}

/// The selected user's own sections.
struct Selected {
    uid: String,
    api_keys: Entity<ApiKeysSection>,
    links: Entity<MinecraftLinksSection>,
}

pub struct UsersPage {
    pub state: Entity<HostState>,
    users: Option<Result<Vec<PublicUser>, String>>,
    selected: Option<Selected>,
    /// The user whose "…" menu is open.
    menu: Option<String>,
    dialog: DialogSlot<Self, Entity<AccountDialog>>,
    deleting: DialogSlot<Self, Deleting>,
    /// A permission change is on its way; `dirty` users changed again meanwhile.
    sending: bool,
    dirty: Vec<String>,
    /// The Settings' search field: the page shows the users and settings that match.
    search: Entity<SettingsSearch>,
    _subscriptions: Vec<Subscription>,
}

impl UsersPage {
    pub fn new(
        state: Entity<HostState>,
        search: Entity<SettingsSearch>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe_search = cx.observe(&search, |_, _, cx| cx.notify());
        let subscription = cx.subscribe(&state, |this, _, event, cx| {
            if let HostEvent::Core(event) = event
                && let EventInner::UserEvent(user) = &event.event_inner
                && matches!(
                    user.user_event_inner,
                    UserEventInner::UserCreated
                        | UserEventInner::UserDeleted
                        | UserEventInner::UsernameChanged { .. }
                        | UserEventInner::PermissionChanged { .. }
                )
                && !this.sending
            {
                this.load(cx);
            }
        });
        let mut this = Self {
            state,
            users: None,
            selected: None,
            menu: None,
            dialog: DialogSlot::new(|this: &mut Self| &mut this.dialog),
            deleting: DialogSlot::new(|this: &mut Self| &mut this.deleting),
            sending: false,
            dirty: Vec::new(),
            search,
            _subscriptions: vec![subscription, observe_search],
        };
        this.load(cx);
        this
    }

    fn owner(&self, cx: &gpui::App) -> bool {
        self.state.read(cx).me.is_owner
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if !self.owner(cx) {
            return;
        }
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move { admin.users().await },
            |this, result, cx| {
                this.users = Some(result.map_err(|err| err.to_string()));
                // the selected user was deleted
                if let Some(selected) = &this.selected
                    && this.user(&selected.uid).is_none()
                    && matches!(this.users, Some(Ok(_)))
                {
                    this.selected = None;
                }
                cx.notify();
            },
        );
    }

    fn user(&self, uid: &str) -> Option<&PublicUser> {
        self.users
            .as_ref()?
            .as_ref()
            .ok()?
            .iter()
            .find(|u| u.uid == uid)
    }

    fn select(&mut self, uid: Option<String>, cx: &mut Context<Self>) {
        self.menu = None;
        self.selected = uid.and_then(|uid| {
            let user = self.user(&uid)?.clone();
            let state = self.state.clone();
            let search = self.search.clone();
            let owner = (user.uid.clone(), user.username.clone());
            Some(Selected {
                uid,
                api_keys: cx.new(|cx| {
                    let mut section = ApiKeysSection::new(
                        state.clone(),
                        api_keys::Owner::User(owner.0.clone(), owner.1.clone()),
                        cx,
                    );
                    section.set_search(search.clone());
                    section
                }),
                links: cx.new(|cx| {
                    let mut section = MinecraftLinksSection::new(
                        state,
                        minecraft_links::Owner::User(owner.0, owner.1),
                        cx,
                    );
                    section.set_search(search);
                    section
                }),
            })
        });
        cx.notify();
    }

    /// For screenshots: opens the first user's menu.
    #[cfg(feature = "screenshot")]
    pub fn open_menu_for_screenshot(&mut self, cx: &mut Context<Self>) {
        if let Some(Ok(users)) = &self.users {
            self.menu = users.first().map(|user| user.uid.clone());
            cx.notify();
        }
    }

    fn open_dialog(&mut self, uid: Option<String>, cx: &mut Context<Self>) {
        self.menu = None;
        let state = self.state.clone();
        // other users sign in from other devices: the owner sets up a password first
        let owner_first = uid.is_none() && account_dialogs::needs_owner_account(state.read(cx));
        let (dialog, subscription) = account_dialogs::open(
            move |cx| match uid {
                Some(uid) => AccountDialog::change_password(state, uid, cx),
                None if owner_first => AccountDialog::owner_account(state, cx),
                None => AccountDialog::create_user(state, cx),
            },
            |this: &mut Self, dialog, event, cx| {
                if this.dialog.current() != Some(dialog) {
                    return;
                }
                match event {
                    AccountDialogEvent::Close => {}
                    AccountDialogEvent::Created(user) => {
                        if let Some(Ok(users)) = this.users.as_mut()
                            && !users.iter().any(|u| u.uid == user.uid)
                        {
                            users.push((**user).clone());
                        }
                    }
                    AccountDialogEvent::Changed => parts::saved(&this.state, cx),
                    AccountDialogEvent::OwnerSetUp(owner) => {
                        if let Some(Ok(users)) = this.users.as_mut()
                            && let Some(user) = users.iter_mut().find(|u| u.uid == owner.uid)
                        {
                            *user = (**owner).clone();
                        }
                        // on to the new user it was for
                        this.dialog.close(cx);
                        this.open_dialog(None, cx);
                        return;
                    }
                }
                this.dialog.close(cx);
                cx.notify();
            },
            cx,
        );
        self._subscriptions.push(subscription);
        self.dialog.open(dialog, cx);
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let Some(deleting) = self.deleting.current_mut() else {
            return;
        };
        if deleting.busy {
            return;
        }
        deleting.busy = true;
        let uid = deleting.user.uid.clone();
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move { admin.delete_user(&uid).await.map(|()| uid) },
            |this, result, cx| {
                this.deleting.close(cx);
                match result {
                    Ok(uid) => {
                        if let Some(Ok(users)) = this.users.as_mut() {
                            users.retain(|u| u.uid != uid);
                        }
                        if this.selected.as_ref().is_some_and(|s| s.uid == uid) {
                            this.selected = None;
                        }
                    }
                    Err(err) => parts::toast(&this.state, parts::Level::Error, err.to_string(), cx),
                }
            },
        );
    }

    /// A permission changed: applied here at once, then sent one change at a time,
    /// each with the user's newest permissions, so quick changes don't undo each
    /// other (as the dashboard queues them).
    fn change_permission(
        &mut self,
        uid: &str,
        permission: Permission,
        instance: Option<String>,
        value: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(users)) = self.users.as_mut() else {
            return;
        };
        let Some(user) = users.iter_mut().find(|u| u.uid == uid) else {
            return;
        };
        match instance {
            Some(instance) => permission.set_instance(&mut user.permissions, &instance, value),
            None => permission.set_flag(&mut user.permissions, value),
        }
        if !self.dirty.iter().any(|d| d == uid) {
            self.dirty.push(uid.to_string());
        }
        cx.notify();
        self.send_next(cx);
    }

    fn send_next(&mut self, cx: &mut Context<Self>) {
        if self.sending || self.dirty.is_empty() {
            return;
        }
        let uid = self.dirty.remove(0);
        let Some(permissions) = self.user(&uid).map(|u| u.permissions.clone()) else {
            return self.send_next(cx);
        };
        self.sending = true;
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move {
                admin
                    .set_permissions(&uid, &permissions)
                    .await
                    .map(|()| (uid, permissions))
            },
            |this, result, cx| {
                this.sending = false;
                match result {
                    Ok((uid, permissions)) => {
                        // the signed-in user's own permissions changed too
                        this.state.update(cx, |state, cx| {
                            if state.me.uid == uid {
                                state.me.permissions = permissions;
                                cx.notify();
                            }
                        });
                        if this.dirty.is_empty() {
                            parts::saved(&this.state, cx);
                        }
                    }
                    Err(err) => {
                        parts::toast(&this.state, parts::Level::Error, err.to_string(), cx);
                        this.load(cx);
                    }
                }
                this.send_next(cx);
            },
        );
    }

    fn user_row(&self, user: &PublicUser, cx: &mut Context<Self>) -> SettingRow {
        let me = self.state.read(cx).me.uid.clone();
        let is_self = user.uid == me;
        let mut tags = Vec::new();
        if is_self {
            tags.push(t(CoreMsg::You));
        }
        if user.is_admin {
            tags.push(t(CoreMsg::Admin));
        }
        if user.is_owner {
            tags.push(t(CoreMsg::Owner));
        }
        let uid = user.uid.clone();
        let open = uid.clone();
        let menu_open = self.menu.as_deref() == Some(&user.uid);
        let menu = menu_open.then(|| {
            let password_uid = uid.clone();
            let delete_user = user.clone();
            (
                SharedString::from(format!("user-menu-{uid}")),
                settings::popover_panel(SharedString::from(format!("user-menu-panel-{uid}")))
                    .w(px(200.))
                    .child(
                        Button::new(
                            SharedString::from(format!("user-password-{uid}")),
                            t(CoreMsg::ChangePasswordMenu),
                        )
                        .variant(ButtonVariant::Ghost)
                        .full_width()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_dialog(Some(password_uid.clone()), cx)
                        })),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("user-delete-{uid}")),
                            t(CoreMsg::DeleteUser),
                        )
                        .variant(ButtonVariant::Ghost)
                        .full_width()
                        .disabled(is_self)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            let focus = cx.focus_handle();
                            this.deleting.open(
                                Deleting {
                                    user: delete_user.clone(),
                                    focus,
                                    busy: false,
                                },
                                cx,
                            );
                        })),
                    ),
                {
                    let this = cx.entity().downgrade();
                    move |_: &mut Window, cx: &mut App| {
                        this.update(cx, |this, cx| {
                            this.menu = None;
                            cx.notify();
                        })
                        .ok();
                    }
                },
            )
        });
        let menu_uid = uid.clone();
        SettingRow::new(user.username.clone())
            .key(SharedString::from(format!("user-{uid}")))
            .search_text(tags.join(" "))
            .leading(parts::avatar(&user.uid, &user.username, 32.))
            .child(
                div().flex().flex_row().gap_1().children(
                    tags.into_iter()
                        .map(|tag| settings::tag(tag, colors::GRAY_400)),
                ),
            )
            .child(
                settings::link(
                    SharedString::from(format!("open-user-{uid}")),
                    t(CommonMsg::Open),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.select(Some(open.clone()), cx))),
            )
            .child(
                Dropdown::new(
                    IconButton::new(
                        SharedString::from(format!("user-more-{uid}")),
                        Icon::Ellipsis,
                    )
                    .label(t(CommonMsg::MoreActions))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = if this.menu.as_deref() == Some(&menu_uid) {
                            None
                        } else {
                            Some(menu_uid.clone())
                        };
                        cx.notify();
                    })),
                )
                .align_end()
                .when_some(menu, |this, (id, panel, dismiss)| {
                    this.popover(id, panel, dismiss)
                }),
            )
    }

    fn list_view(&self, query: &SettingsQuery, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let me = self.state.read(cx).me.uid.clone();
        // the title matches (or nothing is typed): every user
        let all = query.matches([t(CoreMsg::AllMembers)]);
        let (title, body): (String, AnyElement) = match &self.users {
            None => (
                t(CoreMsg::AllMembers).to_string(),
                settings::card([parts::empty_row(t(CoreMsg::LoadingUsers))]).into_any_element(),
            ),
            Some(Err(err)) => (
                t(CoreMsg::AllMembers).to_string(),
                parts::error(err.clone()),
            ),
            Some(Ok(users)) => {
                let mut users = users.clone();
                // yourself first, then by name
                users.sort_by(|a, b| {
                    (b.uid == me)
                        .cmp(&(a.uid == me))
                        .then_with(|| a.username.to_lowercase().cmp(&b.username.to_lowercase()))
                });
                let rows: Vec<AnyElement> = users
                    .iter()
                    .map(|u| self.user_row(u, cx))
                    .filter(|row| all || query.matches_row(row))
                    .map(IntoElement::into_any_element)
                    .collect();
                query.add_found(rows.len());
                if rows.is_empty() {
                    return Vec::new();
                }
                (
                    tr!(CoreMsg::AllMembersCount, count = users.len()),
                    settings::card(rows).into_any_element(),
                )
            }
        };
        // loading, or the list didn't load: nothing to search
        if query.is_active() && query.found() == 0 {
            return Vec::new();
        }
        let description =
            (!query.is_active()).then(|| parts::paragraph(t(CoreMsg::UsersDescription)));
        description
            .into_iter()
            .chain([div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .justify_between()
                        .child(settings::section(title, None))
                        .child(
                            Button::new("create-user", t(CoreMsg::CreateUser))
                                .icon(Icon::Plus)
                                .on_click(cx.listener(|this, _, _, cx| this.open_dialog(None, cx))),
                        ),
                )
                .child(body)
                .into_any_element()])
            .collect()
    }

    fn user_view(
        &self,
        selected: &Selected,
        query: &SettingsQuery,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(user) = self.user(&selected.uid).cloned() else {
            return Vec::new();
        };
        let state = self.state.read(cx);
        let safe_mode = state.settings.as_ref().is_some_and(|s| s.safe_mode);
        let instances = state.instances.clone().unwrap_or_default();
        let this = cx.entity().downgrade();
        let uid = user.uid.clone();
        let on_change: permissions::OnChange =
            Rc::new(move |permission, instance, value, _, cx| {
                let uid = uid.clone();
                this.update(cx, |this, cx| {
                    this.change_permission(&uid, permission, instance, value, cx)
                })
                .ok();
            });
        let normal = permissions::rows(
            "user-perm",
            &Permission::NORMAL,
            &user.permissions,
            &instances,
            false,
            &|_, _| true,
            None,
            on_change.clone(),
        );
        let unsafe_rows = permissions::rows(
            "user-perm-unsafe",
            &Permission::UNSAFE,
            &user.permissions,
            &instances,
            false,
            // safe mode keeps the unsafe permissions off
            &|_, _| !safe_mode,
            None,
            on_change,
        );
        let normal = query
            .section(
                t(CoreMsg::Permissions),
                Some(tr!(CoreMsg::PermissionsLead, username = user.username).into()),
            )
            .card(normal)
            .finish()
            .map(IntoElement::into_any_element);
        let unsafe_section = query
            .section(
                t(CoreMsg::UnsafePermissions),
                Some(t(CoreMsg::UnsafePermissionsLead).into()),
            )
            .card_with(unsafe_rows, |card| {
                card.border_color(colors::RED_FADED.opacity(50))
            })
            .finish()
            .map(IntoElement::into_any_element);
        if query.is_active() {
            query.add_found(selected.api_keys.read(cx).found(query));
            query.add_found(selected.links.read(cx).found(query));
        }
        let mut sections = vec![
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(parts::avatar(&user.uid, &user.username, 32.))
                .child(
                    div()
                        .text_size(text::H2)
                        .font_weight(weights::BOLD.face())
                        .text_color(ink::text())
                        .child(user.username.clone()),
                )
                .child(
                    settings::link("all-users", t(CoreMsg::AllUsers))
                        .on_click(cx.listener(|this, _, _, cx| this.select(None, cx))),
                )
                .into_any_element(),
        ];
        sections.extend(query.status());
        sections.extend(normal);
        sections.extend(unsafe_section);
        sections.push(selected.api_keys.clone().into_any_element());
        sections.push(selected.links.clone().into_any_element());
        sections
    }
}

impl Render for UsersPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.owner(cx) {
            return parts::page([parts::paragraph(t(CoreMsg::UsersOwnerOnly))]);
        }
        let query = self.search.read(cx).query();
        let sections = match &self.selected {
            Some(selected) => {
                let selected = Selected {
                    uid: selected.uid.clone(),
                    api_keys: selected.api_keys.clone(),
                    links: selected.links.clone(),
                };
                self.user_view(&selected, &query, cx)
            }
            None => {
                let list = self.list_view(&query, cx);
                query.status().into_iter().chain(list).collect()
            }
        };
        let mut dialogs = self
            .dialog
            .render(window, |dialog| Some(dialog.clone().into_any_element()));
        dialogs.extend(self.deleting.render(window, |deleting| {
            let this = cx.entity().downgrade();
            let close = cx.entity().downgrade();
            Some(
                Confirm {
                    id: "delete-user",
                    title: t(CoreMsg::DeleteUserTitle).into(),
                    body: tr!(CoreMsg::DeleteUserConfirm, name = deleting.user.username).into(),
                    action: t(CommonMsg::Delete).into(),
                    danger: true,
                    busy: deleting.busy,
                }
                .render(
                    deleting.focus.clone(),
                    move |_, cx| {
                        this.update(cx, |this, cx| this.delete(cx)).ok();
                    },
                    move |_, cx| {
                        close
                            .update(cx, |this, cx| {
                                this.deleting.close(cx);
                            })
                            .ok();
                    },
                )
                .into_any_element(),
            )
        }));
        parts::page(sections).children(window_layer(window, dialogs))
    }
}
