//! Checking what a server sent before anything uses it ("Server to player": what a player
//! checks). A hostile server or friend can send anything, and `lodestone_image::apply`
//! alone checks only a delta's result's id, so every view, whole or made from a delta,
//! must be exactly the server's client view (`lodestone_image::view::check_received`:
//! its side, known rules, its hash, and its own view giving it back), with the id the
//! server named, and something this Launcher installs: a content image (no worlds), on a
//! base it can get, of a bounded number of files.
//!
//! Which paths a view may place is checked again when its files are worked out
//! ([`super::placed`]), since the client view's rules let through paths a server must
//! never write on a player's machine (`options.txt`, `servers.dat`, worlds).

use lodestone_image::{
    Base, Delta, Image, Kind, Side, ViewError, apply, display_text, view::check_received,
};

use super::SyncError;
use crate::images::scope::MAX_PACK_FILES;

/// Checks a view received from a server, whole or made from a delta, against the id the
/// server named for it (`X-Lodestone-View`). The cheap checks come first, so a huge
/// hostile answer is refused before its view is worked out again.
pub fn check_view(image: &Image, expected: &str) -> Result<(), SyncError> {
    if image.entries.len() > MAX_PACK_FILES {
        return Err(SyncError::Refused(format!(
            "it has more than {MAX_PACK_FILES} files"
        )));
    }
    if image.kind != Kind::Content
        || !image.worlds.is_empty()
        || image.entries.iter().any(|entry| entry.world.is_some())
    {
        return Err(SyncError::Refused(
            "it holds worlds or other data, not only mods and configs".into(),
        ));
    }
    if image.id != expected {
        return Err(SyncError::Refused(
            "its view isn't the one it said is current".into(),
        ));
    }
    match check_received(image, Side::Client) {
        Ok(()) => {}
        Err(ViewError::UnsupportedRules(_)) => return Err(SyncError::UpdateLodestone),
        Err(err) => {
            return Err(SyncError::Refused(format!(
                "it isn't the server's client view ({})",
                display_text(&err.to_string())
            )));
        }
    }
    // after the view check, which says whether the base is a known kind at all
    if matches!(image.base, Some(Base::CurseforgePack { .. })) {
        return Err(SyncError::CurseForgeBase);
    }
    Ok(())
}

/// Applies a delta from the server to the kept view its `from` names, and checks the
/// result as a whole view. A delta that would make a view of too many files is refused
/// before it is applied.
pub fn apply_delta(kept: &Image, delta: &Delta, expected: &str) -> Result<Image, SyncError> {
    let entries = kept
        .entries
        .len()
        .saturating_add(delta.added.len())
        .saturating_sub(delta.removed.len());
    if entries > MAX_PACK_FILES {
        return Err(SyncError::Refused(format!(
            "it has more than {MAX_PACK_FILES} files"
        )));
    }
    let view = apply(kept, delta).map_err(|err| {
        SyncError::Refused(format!(
            "its changes don't apply ({})",
            display_text(&err.to_string())
        ))
    })?;
    check_view(&view, expected)?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use lodestone_image::{
        CanonicalPath, Entry, Game, Loader, RULES, Source, World, diff, hex::sha256_hex, view,
    };

    use super::*;

    fn entry(path: &str) -> Entry {
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            3,
            sha256_hex(path.as_bytes()),
            Source::Object {},
        )
    }

    fn client_view(change: impl FnOnce(&mut Image)) -> Image {
        let mut image = Image::new(
            Kind::Content,
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
        );
        image.entries.push(entry("mods/a.jar"));
        change(&mut image);
        image.seal();
        view(&image, Side::Client, RULES).unwrap()
    }

    /// A delta from `from` to whatever `change` makes of it, as a hostile server would
    /// send it: it applies cleanly, so only the checks refuse it.
    fn hostile(from: &Image, change: impl FnOnce(&mut Image)) -> (Delta, String) {
        let mut target = from.clone();
        change(&mut target);
        target.seal();
        let delta = diff(from, &target);
        let delta = Delta::from_json(&serde_json::to_vec(&delta).unwrap()).unwrap();
        (delta, target.id)
    }

    #[test]
    fn a_servers_view_and_deltas_to_it_pass() {
        let first = client_view(|_| {});
        check_view(&first, &first.id).unwrap();
        let second = client_view(|image| image.entries.push(entry("mods/b.jar")));
        let delta = Delta::from_json(&serde_json::to_vec(&diff(&first, &second)).unwrap()).unwrap();
        assert_eq!(
            apply_delta(&first, &delta, &second.id).unwrap().id,
            second.id
        );
    }

    #[test]
    fn deltas_that_add_what_a_client_view_never_carries_are_refused() {
        let first = client_view(|_| {});
        for path in [
            "server.properties",
            ".lodestone_config",
            ".lodestone_modpack/pack.json",
            "mods/.lodestone_client_mods.json",
            "config/unshared.toml",
            "mods/off.jar.disabled",
            "plugins/LuckPerms.jar",
            "bukkit.yml",
        ] {
            let (delta, to) = hostile(&first, |t| t.entries.push(entry(path)));
            assert!(
                matches!(apply_delta(&first, &delta, &to), Err(SyncError::Refused(_))),
                "{path}"
            );
        }
        // the side, and rules this Launcher doesn't know
        let (delta, to) = hostile(&first, |t| t.side = Some(Side::Server));
        assert!(matches!(
            apply_delta(&first, &delta, &to),
            Err(SyncError::Refused(_))
        ));
        let (delta, to) = hostile(&first, |t| t.rules = RULES + 1);
        assert!(matches!(
            apply_delta(&first, &delta, &to),
            Err(SyncError::UpdateLodestone)
        ));
        // a delta for another view
        let other = client_view(|image| image.entries.push(entry("mods/c.jar")));
        let (delta, to) = hostile(&other, |t| t.entries.push(entry("mods/d.jar")));
        assert!(matches!(
            apply_delta(&first, &delta, &to),
            Err(SyncError::Refused(_))
        ));
    }

    #[test]
    fn views_that_lie_or_carry_data_are_refused() {
        let view = client_view(|_| {});
        // the id the server named isn't the view's
        assert!(matches!(
            check_view(&view, &"0".repeat(64)),
            Err(SyncError::Refused(_))
        ));
        // a data image with a world, sent as a view
        let data = client_view(|image| {
            image.kind = Kind::Data;
            image.worlds.push(World {
                name: "world".into(),
                level_name: None,
                data_version: None,
                game_version: None,
                size: None,
                last_played: None,
                quiesced: None,
            });
            image.entries.push(entry("saves/world/level.dat"));
        });
        assert!(matches!(
            check_view(&data, &data.id),
            Err(SyncError::Refused(_))
        ));
        let curseforge = client_view(|image| {
            image.base = Some(Base::CurseforgePack {
                project: 1,
                file: 2,
            })
        });
        assert!(matches!(
            check_view(&curseforge, &curseforge.id),
            Err(SyncError::CurseForgeBase)
        ));
        // an image that isn't a view at all
        let mut image = view.clone();
        image.side = None;
        image.seal();
        assert!(matches!(
            check_view(&image, &image.id),
            Err(SyncError::Refused(_))
        ));
        // a view whose contents don't hash to its id
        let mut forged = view.clone();
        forged.entries.push(entry("mods/z.jar"));
        assert!(matches!(
            check_view(&forged, &forged.id),
            Err(SyncError::Refused(_))
        ));
    }

    #[test]
    fn views_of_too_many_files_are_refused() {
        let many = client_view(|image| {
            image.entries = (0..=MAX_PACK_FILES)
                .map(|i| entry(&format!("mods/m{i}.jar")))
                .collect();
        });
        assert!(matches!(
            check_view(&many, &many.id),
            Err(SyncError::Refused(message)) if message.contains("100000")
        ));
    }
}
