//! `lodestone-launcher-cli`: controls a running Lodestone Launcher over its control
//! socket. See the README and SKILL.md next to this crate.
//!
//! The socket is Unix-only for now: on Windows the commands parse, `schema` works, and
//! everything else fails with `unsupported_platform`, so the printing and asking that
//! only the Unix transport uses go unused there.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

mod cli;
mod output;
mod plan;
mod run;
mod schema;

fn main() {
    std::process::exit(run::main());
}
