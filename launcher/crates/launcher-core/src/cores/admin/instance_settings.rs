//! An instance's Settings tab: work package C (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `GET /instance/{uuid}/settings -> ConfigurableManifest`
//! - `PUT /instance/{uuid}/settings/{section}/{setting}, body: ConfigurableValue | null`
//! - `PUT /instance/{uuid}/auto_start, PUT /instance/{uuid}/restart_on_crash (JSON bool)`
//! - `GET/PUT /instance/{uuid}/public_listing (InstancePublicListing, PublicScopeChange)`
//! - `GET/PUT /instance/{uuid}/resource_pack, POST .../resource_pack/hash`
