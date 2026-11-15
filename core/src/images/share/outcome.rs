//! How a server made from a file ended: what it didn't get, as data beside the creation's
//! English message (`docs/instance-images.md`, "M6b decisions": the structured creation
//! outcome). The `InstanceCreation` progression's end value carries it, so a client (the
//! Launcher's "Host this", the GPUI Host's "From a file") can say it in its own languages;
//! the dashboard and older clients read the English message, built here from the same data.

use lodestone_image::display_text;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::import::{Placed, Why};
use super::inspect::NameList;

/// What a server made from a file didn't get.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, TS)]
#[ts(export)]
pub struct CreationOutcome {
    /// Files whose download failed and whose bytes the file didn't carry, by path.
    pub not_downloaded: NameList,
    /// Files with no bytes in the file that this core can't download (CurseForge's, a
    /// pack's), by path.
    pub no_source: NameList,
    /// Mods deleted because their jar says they run only on clients, by name.
    pub client_jars: NameList,
    /// Files holding a key to the server, a password or a token, never placed, by path.
    pub secrets: NameList,
    /// Settings files deleted because they couldn't be checked for keys to the server (the
    /// server writes its defaults), by path.
    pub settings_removed: NameList,
    /// Scripts left out because only the owner may add them while code execution is on.
    #[ts(type = "number")]
    pub scripts: u64,
}

impl CreationOutcome {
    /// Whether the server got everything it could take.
    pub fn is_empty(&self) -> bool {
        *self == CreationOutcome::default()
    }
}

/// The outcome of placing a file's files: `plan_dropped` is what the plan left out (only
/// the reasons a person needs told about count here), `placed` what placing couldn't do.
pub fn outcome(plan_dropped: &[(String, Why)], placed: &Placed) -> CreationOutcome {
    let dropped = |why: Why| {
        NameList::of(
            plan_dropped
                .iter()
                .filter(|(_, w)| *w == why)
                .map(|(p, _)| p.as_str()),
        )
    };
    CreationOutcome {
        not_downloaded: NameList::of(&placed.left_out),
        no_source: dropped(Why::NoSource),
        client_jars: NameList::of(&placed.client_jars),
        secrets: dropped(Why::Secret),
        settings_removed: NameList::of(&placed.settings_removed),
        scripts: plan_dropped
            .iter()
            .filter(|(_, w)| *w == Why::Script)
            .count() as u64,
    }
}

/// The creation's last message in English: what the new server didn't get, or `None`
/// when it got everything. Each list shows its first 8 names.
pub fn describe(outcome: &CreationOutcome) -> Option<String> {
    let names = |list: &NameList| {
        let shown: Vec<String> = list
            .names
            .iter()
            .take(8)
            .map(|p| display_text(p).into_owned())
            .collect();
        let more = if list.count > 8 { ", …" } else { "" };
        format!("{}{more}", shown.join(", "))
    };
    let mut parts = Vec::new();
    let mut list = |list: &NameList, text: &str| {
        if list.count > 0 {
            parts.push(format!("{} {text}: {}", list.count, names(list)));
        }
    };
    list(
        &outcome.not_downloaded,
        "file(s) couldn't be downloaded and were left out",
    );
    list(
        &outcome.no_source,
        "file(s) can't be downloaded here and were left out",
    );
    list(
        &outcome.client_jars,
        "mod(s) that run only on clients were removed",
    );
    list(
        &outcome.secrets,
        "file(s) holding a key to the server, a password or a token were left out, so the server writes its defaults",
    );
    list(
        &outcome.settings_removed,
        "settings file(s) couldn't be checked for keys to the server and were left out, so the server writes its defaults",
    );
    if outcome.scripts > 0 {
        parts.push(format!(
            "{} script file(s) were left out: only the owner can add scripts, while code execution is on",
            outcome.scripts
        ));
    }
    (!parts.is_empty()).then(|| parts.join(". "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed() -> Placed {
        Placed {
            left_out: vec!["mods/a.jar".into(), "mods/b.jar".into()],
            client_jars: vec!["minimap.jar".into()],
            settings_removed: vec!["spigot.yml".into()],
        }
    }

    fn dropped() -> Vec<(String, Why)> {
        vec![
            ("mods/cf.jar".into(), Why::NoSource),
            ("forwarding.secret".into(), Why::Secret),
            ("start.sh".into(), Why::Script),
            ("plugins/x.sh".into(), Why::Script),
            ("options.txt".into(), Why::ClientFile),
        ]
    }

    #[test]
    fn the_outcome_lists_what_the_server_didnt_get() {
        let outcome = outcome(&dropped(), &placed());
        assert_eq!(
            outcome.not_downloaded,
            NameList::of(["mods/a.jar", "mods/b.jar"])
        );
        assert_eq!(outcome.no_source, NameList::of(["mods/cf.jar"]));
        assert_eq!(outcome.client_jars, NameList::of(["minimap.jar"]));
        assert_eq!(outcome.secrets, NameList::of(["forwarding.secret"]));
        assert_eq!(outcome.settings_removed, NameList::of(["spigot.yml"]));
        assert_eq!(outcome.scripts, 2);
        // a player's file left out isn't news to the owner
        assert!(!outcome.is_empty());
        assert!(super::outcome(
            &[("options.txt".into(), Why::ClientFile)],
            &Placed::default()
        )
        .is_empty());
    }

    #[test]
    fn the_english_message_is_built_from_the_outcome() {
        let outcome = outcome(&dropped(), &placed());
        assert_eq!(
            describe(&outcome).unwrap(),
            "2 file(s) couldn't be downloaded and were left out: mods/a.jar, mods/b.jar. \
             1 file(s) can't be downloaded here and were left out: mods/cf.jar. \
             1 mod(s) that run only on clients were removed: minimap.jar. \
             1 file(s) holding a key to the server, a password or a token were left out, so the server writes its defaults: forwarding.secret. \
             1 settings file(s) couldn't be checked for keys to the server and were left out, so the server writes its defaults: spigot.yml. \
             2 script file(s) were left out: only the owner can add scripts, while code execution is on"
        );
        assert_eq!(describe(&CreationOutcome::default()), None);
    }

    #[test]
    fn long_lists_show_their_first_names() {
        let placed = Placed {
            left_out: (0..10).map(|i| format!("mods/{i}.jar")).collect(),
            ..Placed::default()
        };
        let text = describe(&outcome(&[], &placed)).unwrap();
        assert!(
            text.starts_with("10 file(s) couldn't be downloaded"),
            "{text}"
        );
        assert!(text.ends_with(", …"), "{text}");
        assert_eq!(text.matches(".jar").count(), 8);
    }
}
