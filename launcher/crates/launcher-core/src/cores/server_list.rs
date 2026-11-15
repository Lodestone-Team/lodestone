//! A friend's server in an instance's Minecraft server list, `minecraft/servers.dat`, so
//! the game's Multiplayer screen shows it under its name at its usual address
//! (`127.0.0.1:<its game port>`, see [`super::friends`]).
//!
//! `servers.dat` is uncompressed NBT: a root compound with a `servers` list of
//! compounds (`name`, `ip`, and `icon`, `acceptTextures`, `hidden` that the game adds).
//! The launcher only touches the entry it added, which it finds by the address it wrote
//! ([`Listed`]). Everything else in the file stays as it was, in order.
//!
//! - The first join with an instance adds the entry, unless the list already has that
//!   address (one the user added: it is left alone).
//! - Later joins bring its address and name up to date. A name the user changed in the
//!   game stays.
//! - An entry the user deleted in the game stays deleted: it isn't added again.
//! - Removing the friend's server removes the entry.
//!
//! The game rewrites `servers.dat` when it saves the list, so callers never write it
//! while the instance's game runs.

use std::{fs, io, path::Path};

use lodestone_minecraft::nbt::{self, Compound, NbtError, NbtString, Root, Tag};
use serde::{Deserialize, Serialize};

/// The game's server list, in the game folder.
pub const SERVERS_DAT: &str = "servers.dat";

/// A server list is never this large (icons are small PNGs); bigger files are left alone.
const MAX_BYTES: u64 = 16 * 1024 * 1024;

const COMPOUND: u8 = 10;

/// An entry the launcher added to an instance's server list.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Listed {
    pub instance_id: String,
    /// The name and address the launcher wrote last.
    pub name: String,
    pub ip: String,
    /// The user deleted the entry in the game, so it isn't added again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

/// What [`upsert`] did to the list.
#[derive(Debug, PartialEq)]
pub enum Upserted {
    /// The list changed: the new file, and what to remember.
    Changed(Vec<u8>, Listed),
    /// Nothing to write. `Some` is what to remember now (the user deleted the entry);
    /// `None` keeps what was remembered.
    Unchanged(Option<Listed>),
}

fn invalid(err: NbtError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err)
}

/// The list's entries, as a list of compounds (an empty list may have any type).
fn entries(root: &mut Root) -> Result<&mut Vec<Tag>, NbtError> {
    if root.value.get("servers").is_none() {
        root.value.set("servers", Tag::List(COMPOUND, Vec::new()));
    }
    match root.value.get_mut("servers") {
        Some(Tag::List(kind, items)) if *kind == COMPOUND || items.is_empty() => {
            *kind = COMPOUND;
            Ok(items)
        }
        _ => Err(NbtError::NotNbt),
    }
}

fn read(bytes: Option<&[u8]>) -> Result<Root, NbtError> {
    match bytes {
        None | Some([]) => Ok(Root::default()),
        Some(bytes) => nbt::read(&nbt::unpack(bytes, MAX_BYTES)?),
    }
}

fn find(items: &[Tag], ip: &str) -> Option<usize> {
    items.iter().position(|item| {
        item.as_compound()
            .and_then(|entry| entry.string("ip"))
            .is_some_and(|entry_ip| entry_ip == ip)
    })
}

/// Adds the server `name` at `ip` to the list in `bytes` (`None`: there is no file
/// yet), or brings the entry the launcher added before (`previous`) up to date.
pub fn upsert(
    bytes: Option<&[u8]>,
    previous: Option<&Listed>,
    instance_id: &str,
    name: &str,
    ip: &str,
) -> Result<Upserted, NbtError> {
    if previous.is_some_and(|p| p.deleted) {
        return Ok(Upserted::Unchanged(None));
    }
    let mut root = read(bytes)?;
    let items = entries(&mut root)?;
    let listed = Listed {
        instance_id: instance_id.to_string(),
        name: name.to_string(),
        ip: ip.to_string(),
        deleted: false,
    };
    match previous {
        Some(previous) => {
            let Some(index) = find(items, &previous.ip) else {
                // deleted in the game: leave it so
                return Ok(Upserted::Unchanged(Some(Listed {
                    deleted: true,
                    ..previous.clone()
                })));
            };
            let Some(entry) = items[index].as_compound_mut() else {
                return Err(NbtError::NotNbt);
            };
            // a name the user changed in the game stays theirs
            let renamed = entry
                .string("name")
                .is_some_and(|shown| shown != previous.name);
            let new_name = !renamed && previous.name != name;
            let new_ip = previous.ip != ip;
            if !new_name && !new_ip {
                return Ok(Upserted::Unchanged(None));
            }
            if new_ip {
                entry.set("ip", Tag::String(NbtString::from(ip)));
            }
            if new_name {
                entry.set("name", Tag::String(NbtString::from(name)));
            }
            // a renamed entry remembers the launcher's old name, so later joins still
            // see the user renamed it
            let remembered = Listed {
                name: if renamed {
                    previous.name.clone()
                } else {
                    name.to_string()
                },
                ..listed
            };
            Ok(Upserted::Changed(nbt::write(&root), remembered))
        }
        None => {
            // the user listed this address already: theirs, left alone
            if find(items, ip).is_some() {
                return Ok(Upserted::Unchanged(None));
            }
            items.push(Tag::Compound(Compound(vec![
                (NbtString::from("name"), Tag::String(NbtString::from(name))),
                (NbtString::from("ip"), Tag::String(NbtString::from(ip))),
                (NbtString::from("hidden"), Tag::Byte(0)),
            ])));
            Ok(Upserted::Changed(nbt::write(&root), listed))
        }
    }
}

/// Takes the entry the launcher added out of the list in `bytes`. `None` when it isn't
/// there (deleted in the game).
pub fn remove(bytes: &[u8], listed: &Listed) -> Result<Option<Vec<u8>>, NbtError> {
    if listed.deleted {
        return Ok(None);
    }
    let mut root = read(Some(bytes))?;
    let items = entries(&mut root)?;
    let Some(index) = find(items, &listed.ip) else {
        return Ok(None);
    };
    items.remove(index);
    Ok(Some(nbt::write(&root)))
}

fn read_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_BYTES => Err(invalid(NbtError::TooLarge(MAX_BYTES))),
        Ok(_) => fs::read(path).map(Some),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// [`upsert`] on the instance's `servers.dat` in `game_dir`. Returns what to remember
/// as the instance's entry now, when that changed. Never call it while the instance's
/// game runs.
pub fn list_server(
    game_dir: &Path,
    previous: Option<&Listed>,
    instance_id: &str,
    name: &str,
    ip: &str,
) -> io::Result<Option<Listed>> {
    let path = game_dir.join(SERVERS_DAT);
    let bytes = read_file(&path)?;
    match upsert(bytes.as_deref(), previous, instance_id, name, ip).map_err(invalid)? {
        Upserted::Changed(bytes, listed) => {
            crate::storage::write_atomic(&path, &bytes)?;
            Ok(Some(listed))
        }
        Upserted::Unchanged(listed) => Ok(listed),
    }
}

/// [`remove`] on the instance's `servers.dat` in `game_dir`. Never call it while the
/// instance's game runs.
pub fn unlist_server(game_dir: &Path, listed: &Listed) -> io::Result<()> {
    let path = game_dir.join(SERVERS_DAT);
    let Some(bytes) = read_file(&path)? else {
        return Ok(());
    };
    if let Some(bytes) = remove(&bytes, listed).map_err(invalid)? {
        crate::storage::write_atomic(&path, &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The element type of an empty list, as the game writes it.
    const END: u8 = 0;

    fn text(value: &str) -> Tag {
        Tag::String(NbtString::from(value))
    }

    /// A list as the game writes it, with the user's own servers.
    fn users_list() -> Vec<u8> {
        let server = |name: &str, ip: &str, extra: Vec<(NbtString, Tag)>| {
            let mut fields = vec![
                (NbtString::from("name"), text(name)),
                (NbtString::from("ip"), text(ip)),
            ];
            fields.extend(extra);
            Tag::Compound(Compound(fields))
        };
        nbt::write(&Root {
            name: NbtString::default(),
            value: Compound(vec![(
                NbtString::from("servers"),
                Tag::List(
                    COMPOUND,
                    vec![
                        server(
                            "Hypixel",
                            "mc.hypixel.net",
                            vec![
                                (NbtString::from("icon"), text("iVBORw0KGgo=")),
                                (NbtString::from("acceptTextures"), Tag::Byte(1)),
                                (NbtString::from("hidden"), Tag::Byte(0)),
                            ],
                        ),
                        server("Home", "192.168.1.20", vec![]),
                    ],
                ),
            )]),
        })
    }

    /// The (name, ip) of each entry.
    fn shown(bytes: &[u8]) -> Vec<(String, String)> {
        let root = nbt::read(bytes).unwrap();
        let Some(Tag::List(_, items)) = root.value.get("servers") else {
            panic!("no servers list");
        };
        items
            .iter()
            .map(|item| {
                let entry = item.as_compound().unwrap();
                (
                    entry.string("name").unwrap_or_default(),
                    entry.string("ip").unwrap_or_default(),
                )
            })
            .collect()
    }

    fn changed(result: Upserted) -> (Vec<u8>, Listed) {
        match result {
            Upserted::Changed(bytes, listed) => (bytes, listed),
            other => panic!("{other:?}"),
        }
    }

    fn pair(name: &str, ip: &str) -> (String, String) {
        (name.to_string(), ip.to_string())
    }

    #[test]
    fn adds_then_updates_without_touching_the_users_servers() {
        let before = users_list();
        let (bytes, listed) =
            changed(upsert(Some(&before), None, "inst", "Alex's SMP", "127.0.0.1:31234").unwrap());
        assert_eq!(
            listed,
            Listed {
                instance_id: "inst".into(),
                name: "Alex's SMP".into(),
                ip: "127.0.0.1:31234".into(),
                deleted: false,
            }
        );
        assert_eq!(
            shown(&bytes),
            [
                pair("Hypixel", "mc.hypixel.net"),
                pair("Home", "192.168.1.20"),
                pair("Alex's SMP", "127.0.0.1:31234"),
            ]
        );
        // the user's entries are as they were, icon and all
        let (old, new) = (nbt::read(&before).unwrap(), nbt::read(&bytes).unwrap());
        let (Some(Tag::List(_, old)), Some(Tag::List(_, new))) =
            (old.value.get("servers"), new.value.get("servers"))
        else {
            panic!("no servers lists");
        };
        assert_eq!(old[..], new[..2]);

        // joining again: nothing to write, and no second entry
        assert_eq!(
            upsert(
                Some(&bytes),
                Some(&listed),
                "inst",
                "Alex's SMP",
                "127.0.0.1:31234"
            )
            .unwrap(),
            Upserted::Unchanged(None)
        );

        // the host renamed the server, and the port changed: the same entry, updated
        let (bytes, listed) = changed(
            upsert(
                Some(&bytes),
                Some(&listed),
                "inst",
                "Alex's SMP 2",
                "127.0.0.1:31240",
            )
            .unwrap(),
        );
        assert_eq!(shown(&bytes).len(), 3);
        assert_eq!(shown(&bytes)[2], pair("Alex's SMP 2", "127.0.0.1:31240"));
        assert_eq!(
            (listed.name.as_str(), listed.ip.as_str()),
            ("Alex's SMP 2", "127.0.0.1:31240")
        );

        // removing it gives back the user's list exactly
        let after = remove(&bytes, &listed).unwrap().unwrap();
        assert_eq!(after, before);
        assert_eq!(remove(&after, &listed).unwrap(), None);
    }

    #[test]
    fn a_name_the_user_changed_stays() {
        let (bytes, listed) =
            changed(upsert(None, None, "inst", "SMP", "127.0.0.1:31234").unwrap());
        // renamed in the game
        let mut root = nbt::read(&bytes).unwrap();
        let Some(Tag::List(_, items)) = root.value.get_mut("servers") else {
            panic!("no servers list");
        };
        items[0]
            .as_compound_mut()
            .unwrap()
            .set("name", text("Alex"));
        let bytes = nbt::write(&root);
        assert_eq!(
            upsert(
                Some(&bytes),
                Some(&listed),
                "inst",
                "SMP 2",
                "127.0.0.1:31234"
            )
            .unwrap(),
            Upserted::Unchanged(None)
        );
        // a new address still reaches it; the user's name stays
        let (bytes, again) = changed(
            upsert(
                Some(&bytes),
                Some(&listed),
                "inst",
                "SMP 2",
                "127.0.0.1:31240",
            )
            .unwrap(),
        );
        assert_eq!(shown(&bytes), [pair("Alex", "127.0.0.1:31240")]);
        assert_eq!(again.name, "SMP");
    }

    #[test]
    fn a_deleted_entry_stays_deleted() {
        let (_, listed) = changed(upsert(None, None, "inst", "SMP", "127.0.0.1:31234").unwrap());
        // the user deleted it in the game
        let users = users_list();
        let Upserted::Unchanged(Some(deleted)) = upsert(
            Some(&users),
            Some(&listed),
            "inst",
            "SMP",
            "127.0.0.1:31234",
        )
        .unwrap() else {
            panic!("the deletion is remembered");
        };
        assert!(deleted.deleted);
        assert_eq!(
            upsert(
                Some(&users),
                Some(&deleted),
                "inst",
                "SMP",
                "127.0.0.1:31234"
            )
            .unwrap(),
            Upserted::Unchanged(None)
        );
        assert_eq!(remove(&users, &deleted).unwrap(), None);
    }

    #[test]
    fn an_address_the_user_listed_is_theirs() {
        let users = users_list();
        assert_eq!(
            upsert(Some(&users), None, "inst", "SMP", "192.168.1.20").unwrap(),
            Upserted::Unchanged(None)
        );
    }

    #[test]
    fn a_new_list_and_an_empty_one() {
        let (bytes, _) = changed(upsert(None, None, "inst", "SMP", "127.0.0.1:31234").unwrap());
        assert_eq!(shown(&bytes), [pair("SMP", "127.0.0.1:31234")]);
        // the game writes an empty list with the end type
        let empty = nbt::write(&Root {
            name: NbtString::default(),
            value: Compound(vec![(NbtString::from("servers"), Tag::List(END, vec![]))]),
        });
        let (bytes, _) =
            changed(upsert(Some(&empty), None, "inst", "SMP", "127.0.0.1:31234").unwrap());
        assert_eq!(shown(&bytes).len(), 1);
        // a damaged file is left alone
        assert!(upsert(Some(b"not nbt"), None, "inst", "SMP", "127.0.0.1:1").is_err());
    }

    #[test]
    fn the_file_in_the_game_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SERVERS_DAT);
        // no file yet
        let listed = list_server(dir.path(), None, "inst", "SMP", "127.0.0.1:31234")
            .unwrap()
            .unwrap();
        assert_eq!(shown(&fs::read(&path).unwrap()).len(), 1);
        assert_eq!(
            list_server(dir.path(), Some(&listed), "inst", "SMP", "127.0.0.1:31234").unwrap(),
            None
        );
        unlist_server(dir.path(), &listed).unwrap();
        assert!(shown(&fs::read(&path).unwrap()).is_empty());
        // a damaged file stays as it was
        fs::write(&path, b"damaged").unwrap();
        assert!(list_server(dir.path(), None, "inst", "SMP", "127.0.0.1:31234").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        // an instance without a game folder yet gets one
        let fresh = dir.path().join("new/minecraft");
        list_server(&fresh, None, "inst", "SMP", "127.0.0.1:31234").unwrap();
        assert!(fresh.join(SERVERS_DAT).exists());
        unlist_server(&dir.path().join("gone"), &listed).unwrap();
    }
}
