//! Installing and launching vanilla Minecraft.
//!
//! The parts, in the order a launch uses them:
//! - [`platform`]: the OS, the Java's architecture, Rosetta and memory;
//! - [`version`] and [`rules`]: a version's details file and its `rules`;
//! - [`libraries`], [`assets`], [`runtime`]: what to download, and where it goes;
//! - [`mojang`] and [`download`]: the URL policy and the download engine;
//! - [`install`]: all of the above for one version;
//! - [`natives`], [`launch`], [`process`]: the natives, the command line, the process;
//! - [`log4j`] and [`crash`]: the game's output and what went wrong.

pub mod assets;
pub mod crash;
pub mod download;
pub mod install;
pub mod launch;
pub mod libraries;
pub mod log4j;
pub mod mojang;
pub mod natives;
pub mod platform;
pub mod process;
pub mod rules;
pub mod runtime;
pub mod version;
