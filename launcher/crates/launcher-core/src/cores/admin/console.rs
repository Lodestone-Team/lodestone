//! The Console tab: work package B (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `GET /instance/{uuid}/console/buffer -> ClientEvent[] (the InstanceOutput ones)`
//! - `POST /instance/{uuid}/console, body: the command as a JSON string`
//! - the live console: `LiveStream::open(.., StreamKind::Console, ..)` (cores::live)
