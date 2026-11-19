//! What kind of thing a name is, shown as an icon before it instead of a "(folder)"
//! suffix.

use std::panic::Location;

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::auto_id;
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role},
    },
    i18n::{ValueMsg, t},
    theme::{colors, px},
};

/// The kinds of things the apps name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Folder,
    World,
    Mod,
    ResourcePack,
    ShaderPack,
    DataPack,
    Plugin,
    Modpack,
    Image,
    Server,
    Instance,
    Archive,
    File,
    Backup,
    Snapshot,
    Friend,
    Computer,
}

impl Kind {
    pub const ALL: [Kind; 17] = [
        Kind::Folder,
        Kind::World,
        Kind::Mod,
        Kind::ResourcePack,
        Kind::ShaderPack,
        Kind::DataPack,
        Kind::Plugin,
        Kind::Modpack,
        Kind::Image,
        Kind::Server,
        Kind::Instance,
        Kind::Archive,
        Kind::File,
        Kind::Backup,
        Kind::Snapshot,
        Kind::Friend,
        Kind::Computer,
    ];

    /// The icon that stands for the kind.
    pub fn icon(self) -> Icon {
        match self {
            Kind::Folder => Icon::Folder,
            Kind::World => Icon::Globe,
            Kind::Mod | Kind::Plugin => Icon::PuzzlePiece,
            Kind::ResourcePack | Kind::ShaderPack => Icon::Palette,
            Kind::DataPack => Icon::FilePen,
            Kind::Modpack => Icon::Cube,
            Kind::Image => Icon::Image,
            Kind::Server => Icon::Server,
            // not Play: on a card it looked like a button to press
            Kind::Instance => Icon::Block,
            Kind::Archive => Icon::FileZipper,
            Kind::File => Icon::File,
            Kind::Backup => Icon::ClockRotateLeft,
            Kind::Snapshot => Icon::FloppyDisk,
            Kind::Friend => Icon::User,
            Kind::Computer => Icon::Desktop,
        }
    }

    /// The kind's name, for the icon's accessible name.
    pub fn label(self) -> &'static str {
        t(match self {
            Kind::Folder => ValueMsg::KindFolder,
            Kind::World => ValueMsg::KindWorld,
            Kind::Mod => ValueMsg::KindMod,
            Kind::ResourcePack => ValueMsg::KindResourcePack,
            Kind::ShaderPack => ValueMsg::KindShaderPack,
            Kind::DataPack => ValueMsg::KindDataPack,
            Kind::Plugin => ValueMsg::KindPlugin,
            Kind::Modpack => ValueMsg::KindModpack,
            Kind::Image => ValueMsg::KindImage,
            Kind::Server => ValueMsg::KindServer,
            Kind::Instance => ValueMsg::KindInstance,
            Kind::Archive => ValueMsg::KindArchive,
            Kind::File => ValueMsg::KindFile,
            Kind::Backup => ValueMsg::KindBackup,
            Kind::Snapshot => ValueMsg::KindSnapshot,
            Kind::Friend => ValueMsg::KindFriend,
            Kind::Computer => ValueMsg::KindComputer,
        })
    }
}

/// A kind's icon in gray-500, 12px or 14px, named by the kind for screen readers.
#[derive(IntoElement)]
pub struct KindIcon {
    id: ElementId,
    kind: Kind,
    size: f32,
}

impl KindIcon {
    /// A 14px icon, the size beside a 14px name.
    #[track_caller]
    pub fn new(kind: Kind) -> Self {
        Self {
            id: auto_id(Location::caller(), &format!("{kind:?}")),
            kind,
            size: 14.,
        }
    }

    /// 12px, beside 12px text.
    pub fn small(mut self) -> Self {
        self.size = 12.;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }
}

impl RenderOnce for KindIcon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label: SharedString = self.kind.label().into();
        div()
            .id(self.id.clone())
            .accessible_id(Role::Image, &self.id, label)
            .flex_none()
            .child(self.kind.icon().render(px(self.size), colors::GRAY_500))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, Msg};

    #[test]
    fn every_kind_has_a_translated_name() {
        for kind in Kind::ALL {
            // the label goes through ValueMsg, which every locale translates
            assert!(!kind.label().is_empty(), "{kind:?}");
            assert!(crate::components::Icon::ALL.contains(&kind.icon()));
        }
        for locale in Locale::ALL {
            assert!(!Msg::from(ValueMsg::KindShaderPack).text(locale).is_empty());
        }
    }
}
