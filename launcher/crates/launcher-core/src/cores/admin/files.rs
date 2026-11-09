//! The Files tab (paths are URL-safe Base64 of the instance-relative path): work package B (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `GET /instance/{uuid}/fs/{b64}/ls -> ClientFile[]`
//! - `GET /instance/{uuid}/fs/{b64}/read -> raw text; PUT .../write (raw body)`
//! - `PUT .../new, PUT .../mkdir, DELETE .../rm, DELETE .../rmdir`
//! - `PUT /instance/{uuid}/fs/{b64src}/move/{b64dest}; PUT /instance/{uuid}/fs/cpr`
//! - `PUT /instance/{uuid}/fs/zip (ZipRequest); PUT .../{b64}/unzip (UnzipOption)`
//! - `PUT .../{b64dir}/upload (multipart, field `file`); GET .../{b64}/url -> key for GET /file/{key}`
