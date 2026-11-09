//! Skills for coding agents (Claude Code, Codex and the like): the guides to
//! `lodestone-launcher-cli` and to making skins, kept in the launcher's data folder
//! (`<data>/skills/<name>/SKILL.md`) so an agent on this computer can read them, and the
//! prompt Settings > Developer copies to set an agent up.
//!
//! The files are compiled in and written at startup when they differ, so they always
//! match the running build.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use launcher_core::{paths::DataDir, update::Build};

/// Each skill's folder name and its `SKILL.md`.
pub const SKILLS: &[(&str, &str)] = &[
    (
        "lodestone-launcher",
        include_str!("../../lodestone-launcher-cli/SKILL.md"),
    ),
    (
        "create-skin",
        include_str!("../../../skills/create-skin/SKILL.md"),
    ),
];

/// The repository the command-line tool is built from.
const REPO: &str = "https://github.com/Lodestone-Team/lodestone";

/// `<data>/skills`.
pub fn dir(data: &DataDir) -> PathBuf {
    data.root().join("skills")
}

/// Writes each skill that is missing or out of date. Returns how many were written.
pub fn install(data: &DataDir) -> io::Result<usize> {
    install_into(&dir(data))
}

fn install_into(dir: &Path) -> io::Result<usize> {
    let mut written = 0;
    for (name, text) in SKILLS {
        let path = dir.join(name).join("SKILL.md");
        if fs::read_to_string(&path).is_ok_and(|current| current == *text) {
            continue;
        }
        fs::create_dir_all(path.parent().expect("a skill's file is in its folder"))?;
        launcher_core::storage::write_atomic(&path, text.as_bytes())?;
        written += 1;
    }
    Ok(written)
}

/// The prompt Settings > Developer copies: set up `lodestone-launcher-cli` from this
/// build's commit, read the skills, check the connection, and start debugging.
/// `custom_data_dir` is the data folder when `LODESTONE_LAUNCHER_DATA_DIR` chose it.
///
/// It is in English whatever the launcher's language: it's for the agent.
pub fn prompt(skills: &Path, build: Option<Build>, custom_data_dir: Option<&Path>) -> String {
    let skills = skills.display();
    let source = match build.and_then(|build| build.commit) {
        Some(commit) => format!("--rev {commit}"),
        None => "--branch main".to_string(),
    };
    let mut steps = vec![
        format!(
            "Read the skill that explains the tool: \"{skills}/lodestone-launcher/SKILL.md\". If you \
             can install skills, add the folders in \"{skills}\" to yours (for Claude Code, copy \
             them into ~/.claude/skills/). \"{skills}/create-skin/SKILL.md\" covers making \
             launcher skins."
        ),
        format!(
            "Install the command-line tool, lodestone-launcher-cli, built from the same source as \
             my launcher. It needs Rust (https://rustup.rs):\n   \
             cargo install --locked --git {REPO} {source} lodestone-launcher-cli"
        ),
    ];
    if let Some(dir) = custom_data_dir {
        steps.push(format!(
            "This launcher keeps its data in \"{}\", so run the tool with \
             LODESTONE_LAUNCHER_DATA_DIR set to that folder.",
            dir.display()
        ));
    }
    steps.push(
        "Check that it connects: `lodestone-launcher-cli ping`, then `lodestone-launcher-cli \
         status`. If it says the launcher isn't running, ask me to open it, or to turn on \
         Settings › Developer › Command line › Allow command-line connections."
            .to_string(),
    );
    steps.push(
        "Start debugging: watch what the launcher does with `lodestone-launcher-cli trace watch` \
         (and `play logs` for the game's output), ask me what went wrong, and reproduce it."
            .to_string(),
    );
    let mut text = String::from(
        "Help me debug Lodestone Launcher, the Minecraft launcher running on this computer. You \
         can control it and read its debug traces with its command-line tool.\n\n",
    );
    for (index, step) in steps.iter().enumerate() {
        text.push_str(&format!("{}. {step}\n", index + 1));
    }
    text.push_str(
        "\nIf I also run a Lodestone Core server, its own tool is lodestone_cli: \
         https://github.com/Lodestone-Team/lodestone_cli/releases",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_are_written_once_and_kept_up_to_date() {
        let dir = std::env::temp_dir().join(format!(
            "lodestone-agent-skills-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(install_into(&dir).unwrap(), SKILLS.len());
        assert_eq!(install_into(&dir).unwrap(), 0, "nothing changed");
        let cli = dir.join("lodestone-launcher/SKILL.md");
        std::fs::write(&cli, "an older build's guide").unwrap();
        assert_eq!(install_into(&dir).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&cli).unwrap(), SKILLS[0].1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_skill_has_its_name_in_its_front_matter() {
        for (name, text) in SKILLS {
            assert!(text.starts_with("---\n"), "{name}");
            assert!(
                text.lines().any(|line| line.starts_with("name: ")),
                "{name} has a name"
            );
        }
    }

    #[test]
    fn the_prompt_installs_this_builds_commit() {
        let skills = Path::new("/data/skills");
        let published = Build {
            number: 3500,
            commit: Some("abc1234def"),
            dev: false,
        };
        let text = prompt(skills, Some(published), None);
        assert!(
            text.contains("--rev abc1234def lodestone-launcher-cli"),
            "{text}"
        );
        assert!(text.contains("\"/data/skills/lodestone-launcher/SKILL.md\""));
        assert!(!text.contains("LODESTONE_LAUNCHER_DATA_DIR"));

        let dev = prompt(skills, None, Some(Path::new("/tmp/dev data")));
        assert!(dev.contains("--branch main"), "{dev}");
        assert!(dev.contains("LODESTONE_LAUNCHER_DATA_DIR"));
        assert!(dev.contains("\"/tmp/dev data\""));
    }
}
