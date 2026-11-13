//! Where the window is: the dashboard's routes, as values.
//!
//! The shell shows one [`Route`] at a time; [`crate::pages::open`] makes its page.
//! Adding a page means a variant here, an arm there, and the page's own file. The
//! settings aren't a route: they open in a dialog over the page ([`SettingsPage`]).

use launcher_core::cores::admin::{
    FullCoreInfo, InstanceInfo, PublicUser,
    models::{GlobalSettingsData, InstancePermission, MinecraftVariant},
};

/// A page of the window.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    /// No instance selected (the dashboard's `/`).
    Home,
    /// An instance's tab (`/dashboard/<tab>?instance=<uuid>`).
    Instance { uuid: String, tab: InstanceTab },
}

/// An instance's tabs, in the sidebar's order (`pages/InstanceTabs/tabList.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstanceTab {
    Overview,
    Settings,
    Console,
    Files,
    /// Backups of a Minecraft Java server (not in the dashboard).
    Backups,
    /// "Mods" for Fabric and Forge, "Plugins" for Paper and Spigot.
    Mods,
    EventLogs,
}

impl InstanceTab {
    pub const ALL: [InstanceTab; 7] = [
        InstanceTab::Overview,
        InstanceTab::Settings,
        InstanceTab::Console,
        InstanceTab::Files,
        InstanceTab::Backups,
        InstanceTab::Mods,
        InstanceTab::EventLogs,
    ];
}

/// What a tab needs to be shown, from the dashboard's `SelectedInstanceInfo.tsx`.
pub struct TabContext<'a> {
    pub me: &'a PublicUser,
    pub info: &'a FullCoreInfo,
    pub settings: Option<&'a GlobalSettingsData>,
    pub instance: &'a InstanceInfo,
}

/// What the Mods tab is called for an instance, if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModSupport {
    Mods,
    Plugins,
}

/// Mods for Fabric, Forge and NeoForge, plugins for Paper and Spigot (`utils/mods.ts`).
pub fn mod_support(instance: &InstanceInfo) -> Option<ModSupport> {
    use launcher_core::cores::admin::models::Game;
    match &instance.game_type {
        Game::MinecraftJava { variant } => match variant {
            MinecraftVariant::Fabric | MinecraftVariant::Forge | MinecraftVariant::NeoForge => {
                Some(ModSupport::Mods)
            }
            MinecraftVariant::Paper | MinecraftVariant::Spigot => Some(ModSupport::Plugins),
            _ => None,
        },
        _ => None,
    }
}

impl InstanceTab {
    /// Whether the sidebar lists the tab.
    pub fn visible(self, cx: &TabContext) -> bool {
        match self {
            InstanceTab::Mods => {
                mod_support(cx.instance).is_some()
                    && cx.me.may(InstancePermission::ReadFiles, &cx.instance.uuid)
            }
            // the core backs up Minecraft servers only; listing needs to read files
            InstanceTab::Backups => {
                matches!(
                    cx.instance.game_type,
                    launcher_core::cores::admin::models::Game::MinecraftJava { .. }
                ) && cx.me.may(InstancePermission::ReadFiles, &cx.instance.uuid)
            }
            _ => true,
        }
    }
}

/// The settings' pages (`SettingsLeftNav.tsx`), shown in the Settings dialog over the
/// page (`crate::shell::settings`), as the dashboard's settings modal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsPage {
    /// Core › General.
    General,
    /// Core › Users (the owner's), with a user selected or not.
    Users,
    /// Account › Profile.
    Profile,
    /// Lodestone Host's own updates (package D; not in the dashboard).
    Updates,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 4] = [
        SettingsPage::General,
        SettingsPage::Users,
        SettingsPage::Profile,
        SettingsPage::Updates,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::models::{Game, UserPermission};

    fn instance(variant: MinecraftVariant) -> InstanceInfo {
        serde_json::from_value(serde_json::json!({
            "uuid": "i1", "name": "SMP", "port": 25565, "state": "Stopped",
            "game_type": { "type": "MinecraftJava", "variant": variant },
        }))
        .unwrap()
    }

    #[test]
    fn tabs_follow_the_dashboards_rules() {
        let owner = PublicUser {
            uid: "1".into(),
            username: "boss".into(),
            is_owner: true,
            is_admin: false,
            permissions: UserPermission::default(),
            has_password: true,
        };
        let mut user = owner.clone();
        user.is_owner = false;
        let info = FullCoreInfo::default();
        let fabric = instance(MinecraftVariant::Fabric);
        let vanilla = instance(MinecraftVariant::Vanilla);
        let cx = |me, instance, settings| TabContext {
            me,
            info: &info,
            settings,
            instance,
        };
        assert!(InstanceTab::Mods.visible(&cx(&owner, &fabric, None)));
        assert!(!InstanceTab::Mods.visible(&cx(&owner, &vanilla, None)));
        assert!(
            !InstanceTab::Mods.visible(&cx(&user, &fabric, None)),
            "needs to read files"
        );
        assert!(matches!(fabric.game_type, Game::MinecraftJava { .. }));
        assert!(InstanceTab::Backups.visible(&cx(&owner, &vanilla, None)));
        assert!(
            !InstanceTab::Backups.visible(&cx(&user, &vanilla, None)),
            "needs to read files"
        );
        let mut reader = user.clone();
        reader.permissions.can_read_instance_file.push("i1".into());
        assert!(InstanceTab::Backups.visible(&cx(&reader, &vanilla, None)));
        let docker: InstanceInfo = serde_json::from_value(serde_json::json!({
            "uuid": "DOCKER-abc", "name": "Box", "port": 0, "state": "Running",
            "game_type": { "type": "Generic", "game_name": "x", "game_display_name": "x" },
        }))
        .unwrap();
        assert!(
            !InstanceTab::Backups.visible(&cx(&owner, &docker, None)),
            "only Minecraft servers have backups"
        );
        assert_eq!(
            InstanceTab::ALL
                .iter()
                .position(|t| *t == InstanceTab::Backups),
            Some(4),
            "after Files"
        );
    }
}
