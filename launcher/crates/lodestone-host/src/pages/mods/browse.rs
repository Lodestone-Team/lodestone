//! The Browse view: package A's Modrinth search (`ModrinthSearchView`), set up for the
//! server's mods or plugins: one box for its game version and loader (ticked), the
//! "Installed" tag, and for mods whether players need a result too. A pick opens the
//! project (`project.rs`); the search keeps its text and page meanwhile.

use std::collections::HashSet;

use gpui::{AnyElement, AppContext as _, Context, Entity, IntoElement, Subscription, Window};
use launcher_core::cores::admin::mods::ModrinthProjectType;

use super::{ModsPage, Target};
use crate::{
    i18n::{InstanceMsg as M, t, tr},
    pages::create_instance::{ModrinthSearchEvent, ModrinthSearchView},
};

pub struct Browse {
    search: Entity<ModrinthSearchView>,
    _subscription: Subscription,
}

impl Browse {
    pub fn new(
        target: &Target,
        admin: launcher_core::cores::admin::AdminClient,
        _window: &mut Window,
        cx: &mut Context<ModsPage>,
    ) -> Self {
        let plugin = target.is_plugin();
        let label = tr!(
            M::ModsOnlyCompatible,
            gameVersion = target.game_version,
            loader = target.loader.as_str()
        );
        let (game_version, loader) = (
            target.game_version.clone(),
            target.loader.as_str().to_string(),
        );
        let search = cx.new(|cx| {
            let view = ModrinthSearchView::new(
                admin,
                if plugin {
                    ModrinthProjectType::Plugin
                } else {
                    ModrinthProjectType::Mod
                },
                t(if plugin {
                    M::ModsSearchPlugin
                } else {
                    M::ModsSearchMod
                }),
                cx,
            )
            .with_compat_filter(game_version, loader, label, cx);
            if plugin {
                view
            } else {
                view.with_client_side()
            }
        });
        let subscription = cx.subscribe(&search, |this, _, event, cx| match event {
            ModrinthSearchEvent::Picked(pick) => this.open_project(
                pick.project_id.clone(),
                pick.version_id.clone(),
                pick.hit.clone(),
                cx,
            ),
        });
        Self {
            search,
            _subscription: subscription,
        }
    }
}

impl ModsPage {
    pub(super) fn render_browse(&self, _target: &Target, cx: &mut Context<Self>) -> AnyElement {
        let Some(browse) = &self.browse else {
            return gpui::div().into_any_element();
        };
        let installed: HashSet<String> = self
            .list
            .iter()
            .flat_map(|l| l.mods.iter())
            .filter_map(|m| m.modrinth.as_ref().map(|m| m.project_id.clone()))
            .collect();
        browse
            .search
            .update(cx, |search, cx| search.set_installed(installed, cx));
        browse.search.clone().into_any_element()
    }
}
