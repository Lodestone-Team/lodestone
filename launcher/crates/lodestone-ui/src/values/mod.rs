//! How values are shown: made safe and short ([`truncate`]), the game's and the
//! loaders' one-phrase names ([`product`]), and the one style each role of a value
//! has ([`role`]). The readable components (`components::readable`) and the typed
//! sentences ([`tr_spans!`](crate::tr_spans)) are built on these.
//!
//! `launcher/docs/readable-values.md` has the rules and which component to use when.

pub mod product;
pub mod role;
pub mod truncate;

pub use product::{Product, is_game_or_loader, phrase};
pub use role::{Role, role_run, role_style};
