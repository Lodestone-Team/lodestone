//! Mods, plugins, Modrinth and modpacks: work package A (modpacks, create flow) and C (the Mods tab) (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `C: GET /instance/{uuid}/mods[?check_updates=true] -> InstalledModList`
//! - `C: POST .../mods/install, POST .../mods/upload[?overwrite=true], PUT .../mods/{file}/enable|disable, DELETE .../mods/{file}`
//! - `C: GET .../mods/client_pack -> ClientPackDownload`
//! - `A and C: GET /modrinth/search, /modrinth/project/{id}, /modrinth/project/{id}/versions, /modrinth/version/{id}, /modrinth/resolve`
//! - `A: POST /modpack/inspect, POST /modpack/upload (multipart)`
