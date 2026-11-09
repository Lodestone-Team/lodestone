//! Minecraft accounts linked to Lodestone users, after the dashboard's
//! `components/Settings/MinecraftLinks.tsx`: the signed-in user's own on the Profile,
//! and another user's on their page in Settings › Users (the owner's). Links are made
//! in Lodestone Launcher; here they can only be removed.

use gpui::{
    AnyElement, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, prelude::FluentBuilder as _,
};
use launcher_core::cores::{CoreError, admin::users::MinecraftLink};
use lodestone_ui::components::{
    Button, ButtonVariant,
    settings::{self, SettingRow},
};

use super::parts::{self, Confirm};
use crate::{
    i18n::{CommonMsg, CoreMsg, t, tr},
    state::HostState,
};

/// Whose accounts a section lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Me,
    /// Another user's (`uid`, `username`): through the owner's list of every link.
    User(String, String),
}

struct Removing {
    link: MinecraftLink,
    focus: FocusHandle,
    busy: bool,
}

pub struct MinecraftLinksSection {
    state: Entity<HostState>,
    owner: Owner,
    links: Option<Result<Vec<MinecraftLink>, String>>,
    removing: Option<Removing>,
}

impl MinecraftLinksSection {
    pub fn new(state: Entity<HostState>, owner: Owner, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            state,
            owner,
            links: None,
            removing: None,
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
                    Owner::Me => admin.own_minecraft_links().await,
                    Owner::User(uid, _) => admin
                        .all_minecraft_links()
                        .await
                        .map(|links| links.into_iter().filter(|l| l.uid == uid).collect()),
                }
            },
            |this, result, _| {
                this.links = Some(result.map_err(|err| match err {
                    // a core from before the links
                    CoreError::NotFound(_) => t(CoreMsg::MinecraftTooOld).to_string(),
                    err => err.to_string(),
                }))
            },
        );
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        let Some(removing) = self.removing.as_mut() else {
            return;
        };
        if removing.busy {
            return;
        }
        removing.busy = true;
        let link = removing.link.clone();
        let any_user = matches!(self.owner, Owner::User(..));
        let admin = self.state.read(cx).admin.clone();
        parts::spawn(
            cx,
            async move {
                admin
                    .unlink_minecraft(&link.minecraft_uuid, any_user)
                    .await
                    .map(|()| link)
            },
            |this, result, cx| match result {
                Ok(link) => {
                    this.removing = None;
                    parts::toast(
                        &this.state,
                        parts::Level::Success,
                        tr!(CoreMsg::MinecraftRemoved, name = link.minecraft_name),
                        cx,
                    );
                    this.load(cx);
                }
                Err(err) => {
                    if let Some(removing) = this.removing.as_mut() {
                        removing.busy = false;
                    }
                    parts::toast(&this.state, parts::Level::Error, err.to_string(), cx);
                }
            },
        );
    }
}

impl Render for MinecraftLinksSection {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (lead, empty) = match &self.owner {
            Owner::Me => (
                t(CoreMsg::MinecraftOwnLead).to_string(),
                t(CoreMsg::MinecraftOwnEmpty).to_string(),
            ),
            Owner::User(_, username) => (
                tr!(CoreMsg::MinecraftUserLead, username = username),
                tr!(CoreMsg::MinecraftUserEmpty, username = username),
            ),
        };
        let rows: Vec<AnyElement> = match &self.links {
            None => Vec::new(),
            Some(Err(err)) => vec![parts::empty_row(err.clone())],
            Some(Ok(links)) if links.is_empty() => vec![parts::empty_row(empty)],
            Some(Ok(links)) => links
                .iter()
                .map(|link| {
                    let to_remove = link.clone();
                    SettingRow::new(link.minecraft_name.clone())
                        .key(SharedString::from(format!("mc-{}", link.minecraft_uuid)))
                        .below(parts::mono(link.minecraft_uuid.clone()))
                        .child(
                            Button::new(
                                SharedString::from(format!("unlink-{}", link.minecraft_uuid)),
                                t(CommonMsg::Remove),
                            )
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.removing = Some(Removing {
                                        link: to_remove.clone(),
                                        focus: cx.focus_handle(),
                                        busy: false,
                                    });
                                    cx.notify();
                                },
                            )),
                        )
                        .into_any_element()
                })
                .collect(),
        };
        let confirm = self.removing.as_ref().map(|removing| {
            let this = cx.entity().downgrade();
            let close = cx.entity().downgrade();
            parts::overlay(
                window,
                Confirm {
                    id: "unlink-minecraft",
                    title: t(CoreMsg::MinecraftRemoveTitle).into(),
                    body: tr!(
                        CoreMsg::MinecraftRemoveBody,
                        name = removing.link.minecraft_name
                    )
                    .into(),
                    action: t(CommonMsg::Remove).into(),
                    danger: true,
                    busy: removing.busy,
                }
                .render(
                    removing.focus.clone(),
                    move |_, cx| {
                        this.update(cx, |this, cx| this.remove(cx)).ok();
                    },
                    move |_, cx| {
                        close
                            .update(cx, |this, cx| {
                                this.removing = None;
                                cx.notify();
                            })
                            .ok();
                    },
                ),
            )
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(settings::section(
                t(CoreMsg::MinecraftAccounts),
                Some(lead.into()),
            ))
            .when(!rows.is_empty(), |this| this.child(settings::card(rows)))
            .children(confirm)
    }
}
