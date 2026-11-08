//! The Modrinth types the core sends to clients, from [`lodestone_minecraft::modrinth::types`].
//! Their TypeScript bindings are exported into `bindings/` from here.

pub use lodestone_minecraft::modrinth::types::*;

#[cfg(test)]
crate::export_shared_bindings! {
    export_bindings_modrinthprojecttype => ModrinthProjectType,
    export_bindings_modrinthsearchindex => ModrinthSearchIndex,
    export_bindings_modrinthsearchresponse => ModrinthSearchResponse,
    export_bindings_modrinthsearchhit => ModrinthSearchHit,
    export_bindings_modrinthgalleryimage => ModrinthGalleryImage,
    export_bindings_modrinthproject => ModrinthProject,
    export_bindings_modrinthversiontype => ModrinthVersionType,
    export_bindings_modrinthdependencytype => ModrinthDependencyType,
    export_bindings_modrinthversionfile => ModrinthVersionFile,
    export_bindings_modrinthdependency => ModrinthDependency,
    export_bindings_modrinthversion => ModrinthVersion,
    export_bindings_modrinthurltarget => ModrinthUrlTarget,
}
