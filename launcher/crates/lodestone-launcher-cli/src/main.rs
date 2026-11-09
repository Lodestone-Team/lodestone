//! `lodestone-launcher-cli`: controls a running Lodestone Launcher over its control
//! socket. See the README and SKILL.md next to this crate.

mod cli;
mod output;
mod plan;
mod run;
mod schema;

fn main() {
    std::process::exit(run::main());
}
