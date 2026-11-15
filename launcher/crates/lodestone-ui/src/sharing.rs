//! What the apps say about moving a game to another computer as a `.lodeimg` file, so the
//! GPUI Host's "From a file" and the Launcher's "Host this" say it alike: what a file
//! holds before a server is made from it ([`summary_lines`] and the helpers beside it,
//! from the core's [`ImageSummary`]), and what the server didn't get once made
//! ([`outcome_lines`], from the creation's [`CreationOutcome`]). Each app lays the lines
//! out with its own components.

use launcher_core::cores::admin::images::{
    CreationOutcome, ImageMaker, ImageSummary, NameList, NotTaken,
};
use launcher_core::cores::admin::instances::NAME_FORBIDDEN;

use crate::i18n::{CommonPlural, SharingMsg as M, SharingPlural as P, format, t, tr, trn};

/// A server's name from the file: its label without control and invisible characters,
/// else the file's name without `.lodeimg`, at most 64 characters, with the characters a
/// server's name can't have replaced (an unnamed backup's label has the time, `14:10`).
pub fn default_name(summary: &ImageSummary) -> String {
    let label = summary.label.as_deref().unwrap_or_default();
    let label = visible(label);
    let name = if label.trim().is_empty() {
        let file = visible(&summary.file_name);
        file.strip_suffix(".lodeimg").unwrap_or(&file).to_string()
    } else {
        label
    };
    server_name(&name)
}

/// `name` as a server's name the core takes: `:` becomes `.`, the other characters a
/// name can't have `-`, at most 64 characters.
pub fn server_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            ':' => '.',
            c if NAME_FORBIDDEN.contains(&c) => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect()
}

/// `text` without the invisible characters the core writes visibly (`\u{…}`) and without
/// control characters.
fn visible(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\\u{") {
        out.push_str(&rest[..at]);
        rest = rest[at..].split_once('}').map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out.chars().filter(|c| !c.is_control()).collect()
}

/// The note on the server made: a Bukkit-family server the file names (Spigot,
/// Purpur…) is made as Paper, said only while Paper is the server picked.
pub fn server_note(summary: &ImageSummary, picked: Option<&str>) -> Option<String> {
    let flavour = summary.flavour.as_deref()?;
    let paper = picked.or(summary.server.as_deref()) == Some("Paper");
    let relative = ["spigot", "bukkit", "purpur", "folia"]
        .iter()
        .any(|f| flavour.eq_ignore_ascii_case(f));
    (paper && relative).then(|| tr!(M::ImageMadeAsPaper, flavour = capitalized(flavour)))
}

fn capitalized(text: &str) -> String {
    let mut text = text.to_string();
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

/// Why no server can be made from the file, when none can.
pub fn unsupported(summary: &ImageSummary) -> Option<String> {
    summary.unsupported.as_ref()?;
    Some(tr!(
        M::ImageUnsupported,
        loader = capitalized(&summary.loader)
    ))
}

/// The world picked when the summary opens: the file's own choice when it names one it
/// has (a Host's level, a Launcher's last played world).
pub fn default_world(summary: &ImageSummary) -> Option<String> {
    summary
        .default_world
        .clone()
        .filter(|w| summary.worlds.iter().any(|world| &world.folder == w))
        .or_else(|| summary.worlds.first().map(|w| w.folder.clone()))
}

/// The game in a line: "Fabric 0.16.10 · Minecraft 1.21.1", "Paper · Minecraft 1.21.1".
pub fn game_line(summary: &ImageSummary) -> String {
    let software = summary
        .server
        .clone()
        .or_else(|| summary.flavour.clone())
        .unwrap_or_else(|| summary.loader.clone());
    let software = match (&summary.loader_version, summary.loader.as_str()) {
        (Some(version), loader) if loader != "vanilla" => format!("{software} {version}"),
        _ => software,
    };
    format!("{software} · Minecraft {}", summary.minecraft)
}

/// The server software to ask for: the one picked, only when the file's game allows a
/// choice and it is one of them; otherwise the core makes the file's own.
pub fn chosen_server(summary: &ImageSummary, picked: Option<&str>) -> Option<String> {
    let picked = picked?;
    (summary.server_choices.len() > 1 && summary.server_choices.iter().any(|c| c == picked))
        .then(|| picked.to_string())
}

/// The server software a caller typed (the control API, the CLI) as the file's
/// choices spell it, whatever its case; `None` when it is none of them.
pub fn server_choice<'a>(summary: &'a ImageSummary, typed: &str) -> Option<&'a str> {
    let typed = typed.trim();
    summary
        .server_choices
        .iter()
        .find(|choice| choice.eq_ignore_ascii_case(typed))
        .map(String::as_str)
}

/// What the server won't take from the file, as a list.
pub fn not_taken(what: &[NotTaken]) -> String {
    what.iter()
        .filter_map(|n| {
            Some(t(match n {
                NotTaken::ServerSoftware => M::NotTakenSoftware,
                NotTaken::Eula => M::NotTakenEula,
                NotTaken::ClientFiles => M::NotTakenClientFiles,
                NotTaken::Operators => M::NotTakenOperators,
                NotTaken::Scripts => M::NotTakenScripts,
                NotTaken::Unknown => return None,
            }))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a server can be made from the file `summary` describes by someone who may
/// (`may_write_global`) or may not change global files: not when Lodestone makes no server
/// for it, nor when it carries protected files the person may not add, which the core
/// always refuses whatever world is picked. No room for the default world doesn't stop
/// it: a smaller world may fit, and the core refuses clearly when it doesn't.
pub fn can_create(summary: &ImageSummary, may_write_global: bool) -> bool {
    summary.unsupported.is_none() && (may_write_global || !summary.needs_global_files)
}

/// The trust warning of a file whose server takes mods or plugins, which run on it: how
/// many, as the Launcher's import says it, and the ones whose bytes the file carries
/// (from whoever made it, not a download). A file with no mods that still carries
/// protected files of its own (jars, programs) gets a warning too.
pub fn trust_warning(summary: &ImageSummary) -> Option<String> {
    let count = summary.server_mods.max(summary.carried_mods.count);
    if count == 0 {
        return summary
            .needs_global_files
            .then(|| t(M::ImageProtectedWarning).to_string());
    }
    let mut text = trn!(P::ImageModsWarning, count);
    if !summary.carried_mods.is_empty() {
        text.push(' ');
        text.push_str(&tr!(M::ImageCarried, names = summary.carried_mods.joined()));
        if !text.ends_with(['.', '…', '。']) {
            text.push('.');
        }
    }
    Some(text)
}

/// How bad a [`SummaryLines`] alert is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Nothing can be made as things stand.
    Error,
    /// Worth reading before making it.
    Warning,
}

/// What the person has picked so far, which some lines follow.
#[derive(Clone, Copy, Debug, Default)]
pub struct Picked<'a> {
    /// The world (its folder) that becomes the server's; `None` for none.
    pub world: Option<&'a str>,
    /// The server software, one of the summary's choices; `None` for the file's own.
    pub server: Option<&'a str>,
    /// The file's operators are kept (their tickbox then says so itself).
    pub keep_operators: bool,
    /// The person may change global files (what a file's jars need).
    pub may_write_global: bool,
    /// The file is the person's own game, sent from this computer (the Launcher's "Host
    /// this"): who made it and the warning to trust its sender say nothing then.
    pub own_file: bool,
}

/// A file's summary as lines of text: notes under the title (the game, who made it),
/// alerts to read before creating, and details (what is downloaded, left out, the disk).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SummaryLines {
    pub notes: Vec<String>,
    pub alerts: Vec<(Level, String)>,
    pub details: Vec<String>,
}

/// What `summary` says, with `picked` chosen.
pub fn summary_lines(summary: &ImageSummary, picked: Picked<'_>) -> SummaryLines {
    let mut lines = SummaryLines::default();
    // the game line names the server picked
    let with_server = ImageSummary {
        server: picked
            .server
            .map(str::to_string)
            .or_else(|| summary.server.clone()),
        ..summary.clone()
    };
    lines.notes.push(format!(
        "{} · {} · {}",
        game_line(&with_server),
        trn!(CommonPlural::Files, summary.files),
        format::bytes(summary.total_bytes)
    ));
    if !picked.own_file {
        lines.notes.push(match summary.made_by {
            ImageMaker::Host => tr!(M::ImageMadeByHost, version = summary.producer_version),
            ImageMaker::Launcher => {
                tr!(M::ImageMadeByLauncher, version = summary.producer_version)
            }
            ImageMaker::Unknown => t(M::ImageMadeByOther).to_string(),
        });
    }
    if let Some(note) = server_note(summary, picked.server) {
        lines.notes.push(note);
    }
    if let Some(base) = &summary.base {
        lines
            .notes
            .push(launcher_core::images::display_text(base).into_owned());
    }

    if let Some(why) = unsupported(summary) {
        lines.alerts.push((Level::Error, why));
    }
    if let Some(warning) = trust_warning(summary).filter(|_| !picked.own_file) {
        lines.alerts.push((Level::Warning, warning));
    }
    if !summary.not_downloadable.is_empty() {
        lines.alerts.push((
            Level::Warning,
            tr!(
                M::ImageNotDownloadable,
                names = summary.not_downloadable.joined()
            ),
        ));
    }
    if summary.offline_mode {
        lines
            .alerts
            .push((Level::Warning, t(M::ImageOfflineMode).to_string()));
    }
    if summary.needs_global_files && !picked.may_write_global {
        lines
            .alerts
            .push((Level::Error, t(M::ImageNeedsGlobal).to_string()));
    }

    if summary.download_bytes > 0 {
        let size = format::bytes(summary.download_bytes);
        lines
            .details
            .push(if summary.mods_from_modrinth.is_empty() {
                tr!(M::ImageDownloads, size = size)
            } else {
                tr!(
                    M::ImageDownloadsMods,
                    size = size,
                    names = summary.mods_from_modrinth.joined()
                )
            });
    }
    for (list, message) in [
        (&summary.client_only, M::ImageClientOnly),
        (&summary.left_out, M::ImageTakenOut),
        (&summary.withheld, M::ImageWithheld),
        (&summary.blanked, M::ImageBlanked),
        (&summary.secrets_not_taken, M::ImageSecretsNotTaken),
    ] {
        if !list.is_empty() {
            lines.details.push(tr!(message, names = list.joined()));
        }
    }
    let what = not_taken(
        &summary
            .not_taken
            .iter()
            .filter(|n| !(picked.keep_operators && **n == NotTaken::Operators))
            .copied()
            .collect::<Vec<_>>(),
    );
    if !what.is_empty() {
        lines.details.push(tr!(M::ImageNotTaken, what = what));
    }
    // for the world picked: the core checks again
    if let Some(free) = summary.disk_free {
        lines.details.push(tr!(
            M::ImageDisk,
            needed = format::bytes(
                summary
                    .needed_bytes_for(picked.world)
                    .saturating_add(ImageSummary::SPARE_BYTES)
            ),
            free = format::bytes(free)
        ));
    }
    if !summary.has_room_for(picked.world) {
        lines
            .alerts
            .push((Level::Warning, t(M::ImageNoRoom).to_string()));
    }
    lines
}

/// What a server made from a file didn't get, a line per kind (empty when it got
/// everything it could take).
pub fn outcome_lines(outcome: &CreationOutcome) -> Vec<String> {
    let mut lines = Vec::new();
    let mut list = |names: &NameList, message: P| {
        if !names.is_empty() {
            lines.push(trn!(message, names.count, names = names.joined()));
        }
    };
    list(&outcome.not_downloaded, P::OutcomeNotDownloaded);
    list(&outcome.no_source, P::OutcomeNoSource);
    list(&outcome.client_jars, P::OutcomeClientJars);
    list(&outcome.secrets, P::OutcomeSecrets);
    list(&outcome.settings_removed, P::OutcomeSettingsRemoved);
    if outcome.scripts > 0 {
        lines.push(trn!(P::OutcomeScripts, outcome.scripts));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::images::ImageWorld;

    fn summary(json: serde_json::Value) -> ImageSummary {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn a_server_is_named_after_the_file() {
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "Survival \\u{202E}SMP", "file_name": "x.lodeimg"
            }))),
            "Survival SMP"
        );
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "  ", "file_name": "Island.lodeimg"
            }))),
            "Island"
        );
        let long = "a".repeat(100);
        assert_eq!(
            default_name(&summary(serde_json::json!({ "label": long })))
                .chars()
                .count(),
            64
        );
        // an unnamed backup's label has the time, and a name may have any character: the
        // name made is one the core takes
        let name = default_name(&summary(serde_json::json!({
            "label": "Survival – 2026-11-12 14:10 UTC"
        })));
        assert_eq!(name, "Survival – 2026-11-12 14.10 UTC");
        let name = default_name(&summary(serde_json::json!({
            "label": "Before 1/2 \"big\" update? <a|b> *\\"
        })));
        assert!(!name.contains(NAME_FORBIDDEN), "{name}");
        assert_eq!(server_name("My: world/1"), "My. world-1");
    }

    #[test]
    fn the_files_own_world_is_picked() {
        let world = |folder: &str| ImageWorld {
            folder: folder.into(),
            ..Default::default()
        };
        let mut s = ImageSummary {
            worlds: vec![world("A"), world("B")],
            default_world: Some("B".into()),
            ..Default::default()
        };
        assert_eq!(default_world(&s).as_deref(), Some("B"));
        s.default_world = Some("gone".into());
        assert_eq!(default_world(&s).as_deref(), Some("A"));
        s.worlds.clear();
        assert_eq!(default_world(&s), None);
    }

    #[test]
    fn server_software_is_asked_for_only_when_there_is_a_choice() {
        let vanilla = summary(serde_json::json!({
            "server": "Vanilla", "server_choices": ["Vanilla", "Paper"]
        }));
        assert_eq!(
            chosen_server(&vanilla, Some("Paper")).as_deref(),
            Some("Paper")
        );
        assert_eq!(chosen_server(&vanilla, Some("Fabric")), None);
        assert_eq!(chosen_server(&vanilla, None), None);
        let fabric = summary(serde_json::json!({
            "server": "Fabric", "server_choices": ["Fabric"]
        }));
        assert_eq!(chosen_server(&fabric, Some("Fabric")), None);
    }

    #[test]
    fn a_typed_server_software_is_matched_whatever_its_case() {
        let vanilla = summary(serde_json::json!({
            "server": "Vanilla", "server_choices": ["Vanilla", "Paper"]
        }));
        assert_eq!(server_choice(&vanilla, "paper"), Some("Paper"));
        assert_eq!(server_choice(&vanilla, " VANILLA "), Some("Vanilla"));
        assert_eq!(server_choice(&vanilla, "Spigot"), None);
        let none = summary(serde_json::json!({ "server_choices": [] }));
        assert_eq!(server_choice(&none, "Paper"), None);
    }

    #[test]
    fn notes_on_the_server_are_the_apps_own_words() {
        crate::i18n::force(crate::i18n::Locale::En);
        let purpur = summary(serde_json::json!({
            "loader": "vanilla", "flavour": "purpur", "server": "Paper",
            "server_choices": ["Vanilla", "Paper"],
            "server_note": "the core's English"
        }));
        assert_eq!(
            server_note(&purpur, None).as_deref(),
            Some("Lodestone makes Purpur servers as Paper servers, which run the same plugins.")
        );
        assert_eq!(
            server_note(&purpur, Some("Vanilla")),
            None,
            "not once Vanilla is picked"
        );
        let paper = summary(serde_json::json!({ "flavour": "paper", "server": "Paper" }));
        assert_eq!(server_note(&paper, None), None);
        let quilt = summary(serde_json::json!({
            "loader": "quilt", "unsupported": "Lodestone can't make Quilt servers"
        }));
        assert_eq!(
            unsupported(&quilt).as_deref(),
            Some("Lodestone can’t make Quilt servers.")
        );
        assert_eq!(unsupported(&paper), None);
    }

    #[test]
    fn mods_from_anywhere_get_the_trust_warning() {
        crate::i18n::force(crate::i18n::Locale::En);
        let downloaded = summary(serde_json::json!({
            "server_mods": 40,
            "mods_from_modrinth": {"names": ["lithium.jar"], "count": 40}
        }));
        assert_eq!(
            trust_warning(&downloaded).as_deref(),
            Some(
                "It has 40 mods or plugins: programs that run on the server. Only use files from people you trust."
            )
        );
        let carried = summary(serde_json::json!({
            "server_mods": 1, "carried_mods": {"names": ["own.jar"], "count": 1}
        }));
        assert_eq!(
            trust_warning(&carried).as_deref(),
            Some(
                "It has 1 mod or plugin: a program that runs on the server. Only use files from people you trust. These are in the file itself, not downloaded from Modrinth: own.jar."
            )
        );
        assert_eq!(trust_warning(&summary(serde_json::json!({}))), None);
        // no mods, but protected files of its own: warned too
        let programs = summary(serde_json::json!({ "needs_global_files": true }));
        assert_eq!(
            trust_warning(&programs).as_deref(),
            Some(t(M::ImageProtectedWarning))
        );
    }

    #[test]
    fn a_file_the_core_would_refuse_cant_be_created() {
        let fine = summary(serde_json::json!({ "server": "Fabric" }));
        assert!(can_create(&fine, false));
        let jars = summary(serde_json::json!({ "server": "Fabric", "needs_global_files": true }));
        assert!(can_create(&jars, true));
        assert!(!can_create(&jars, false));
        let quilt = summary(serde_json::json!({ "unsupported": "Quilt" }));
        assert!(!can_create(&quilt, true));
    }

    #[test]
    fn the_game_reads_in_one_line() {
        crate::i18n::force(crate::i18n::Locale::En);
        let fabric = summary(serde_json::json!({
            "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10",
            "server": "Fabric"
        }));
        assert_eq!(game_line(&fabric), "Fabric 0.16.10 · Minecraft 1.21.1");
        let paper = summary(serde_json::json!({
            "minecraft": "1.21.3", "loader": "vanilla", "flavour": "purpur", "server": "Paper"
        }));
        assert_eq!(game_line(&paper), "Paper · Minecraft 1.21.3");
        assert_eq!(
            not_taken(&[NotTaken::Eula, NotTaken::Unknown, NotTaken::Operators]),
            "the EULA, the operators"
        );
    }

    #[test]
    fn the_summary_follows_what_is_picked() {
        crate::i18n::force(crate::i18n::Locale::En);
        let file = summary(serde_json::json!({
            "made_by": "launcher", "producer_version": "1.4.0", "minecraft": "1.21.1",
            "loader": "fabric", "loader_version": "0.16.10", "server": "Fabric",
            "files": 3, "total_bytes": 2048,
            "client_only": {"names": ["sodium.jar"], "count": 1},
            "not_downloadable": {"names": ["mods/jade.jar"], "count": 1},
            "not_taken": ["client_files", "operators"], "needs_global_files": true,
            "worlds": [{"folder": "Big", "size": 20000}, {"folder": "Small", "size": 5}],
            "default_world": "Big", "needed_bytes": 50000,
            "disk_free": (1u64 << 30) + 20000
        }));
        let lines = summary_lines(
            &file,
            Picked {
                world: Some("Big"),
                ..Default::default()
            },
        );
        assert!(lines.notes[0].starts_with("Fabric 0.16.10 · Minecraft 1.21.1 · 3 files"));
        assert_eq!(lines.notes[1], "Made with Lodestone Launcher 1.4.0");
        assert!(
            lines
                .alerts
                .contains(&(Level::Error, t(M::ImageNeedsGlobal).to_string()))
        );
        assert!(
            lines
                .alerts
                .contains(&(Level::Warning, t(M::ImageNoRoom).to_string()))
        );
        assert!(
            lines
                .details
                .contains(&"Mods only players run, left out: sodium.jar".to_string())
        );
        assert!(
            lines.details.contains(
                &"Not taken from the file: a player’s own files, the operators".to_string()
            )
        );
        // a smaller world fits, someone who may change global files may create, and kept
        // operators aren't "not taken"
        let lines = summary_lines(
            &file,
            Picked {
                world: Some("Small"),
                keep_operators: true,
                may_write_global: true,
                ..Default::default()
            },
        );
        assert!(!lines.alerts.iter().any(|(level, _)| *level == Level::Error));
        assert!(
            !lines
                .alerts
                .contains(&(Level::Warning, t(M::ImageNoRoom).to_string()))
        );
        assert!(
            lines
                .details
                .contains(&"Not taken from the file: a player’s own files".to_string())
        );
        // the person's own game: no maker, no warning to trust the sender
        let own = summary_lines(
            &summary(serde_json::json!({
                "made_by": "launcher", "producer_version": "1.4.0", "server_mods": 3
            })),
            Picked {
                own_file: true,
                may_write_global: true,
                ..Default::default()
            },
        );
        assert_eq!(own.notes.len(), 1, "{own:?}");
        assert!(own.alerts.is_empty(), "{own:?}");
    }

    #[test]
    fn the_outcome_says_what_was_left_out() {
        crate::i18n::force(crate::i18n::Locale::En);
        let outcome: CreationOutcome = serde_json::from_value(serde_json::json!({
            "not_downloaded": {"names": ["mods/a.jar"], "count": 1},
            "no_source": {"names": ["mods/cf.jar", "mods/cf2.jar"], "count": 70},
            "scripts": 2
        }))
        .unwrap();
        assert_eq!(
            outcome_lines(&outcome),
            [
                "1 file couldn’t be downloaded and was left out: mods/a.jar",
                "70 files can’t be downloaded by Lodestone servers and were left out: mods/cf.jar, mods/cf2.jar, …",
                "2 scripts were left out: only the owner can add scripts, while code execution is on.",
            ]
        );
        assert!(outcome_lines(&CreationOutcome::default()).is_empty());
    }
}
