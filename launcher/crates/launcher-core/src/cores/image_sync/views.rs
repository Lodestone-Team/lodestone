//! The server views an instance keeps (`<instance>/server-views/<id>.json`): the one it
//! applied last, which a delta or a whole view is compared with, and a few before it, which
//! it names in `have` so the server can send a delta from any of them (a player who
//! switched back, or whose applied view the server pruned). They sit beside `instance.json`,
//! outside the game folder, so they are never content.
//!
//! Each file is checked when read as a view from the server is (its id is its name, it
//! hashes to it, and it is a client view): one that isn't is deleted and not named.

use std::{
    fs,
    path::{Path, PathBuf},
};

use lodestone_image::{
    Image, Side, hex::is_sha256, store::MAX_MANIFEST_BYTES, view::check_received,
};

use super::super::player_image::MAX_HAVE;
use crate::storage;

/// The folder of an instance's kept views.
pub const VIEWS_FOLDER: &str = "server-views";
/// How many views an instance keeps, the applied one among them.
pub const KEPT_VIEWS: usize = 4;
/// The file naming the views updates were placing, one id a line, from before an update's
/// files change until the link names a view: what a crash part way leaves.
const PLACING: &str = "placing";
/// At most this many views are named in [`PLACING`] (each update that stopped part way
/// without being followed by one that finished adds one); the oldest go first.
const MAX_PLACING: usize = 8;

/// An instance's kept views.
pub struct KeptViews {
    dir: PathBuf,
}

impl KeptViews {
    pub fn open(instance_dir: &Path) -> Self {
        Self {
            dir: instance_dir.join(VIEWS_FOLDER),
        }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// The kept view `id`, checked; `None` when there is none or it is damaged (then it
    /// is deleted).
    pub fn load(&self, id: &str) -> Option<Image> {
        if !is_sha256(id) {
            return None;
        }
        let path = self.path(id);
        let bytes = read_capped(&path)?;
        let image = Image::from_json(&bytes)
            .ok()
            .filter(|image| image.id == id && check_received(image, Side::Client).is_ok());
        if image.is_none() {
            let _ = fs::remove_file(&path);
        }
        image
    }

    /// Keeps `view` (a checked client view), replacing a file of the same id.
    pub fn keep(&self, view: &Image) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        storage::write_atomic(&self.path(&view.id), &view.to_json_pretty())
    }

    /// Keeps `view` unless it is kept already.
    pub fn keep_once(&self, view: &Image) -> std::io::Result<()> {
        if self.path(&view.id).is_file() {
            return Ok(());
        }
        self.keep(view)
    }

    /// Notes that an update starts placing `view`'s files: keeps it and adds it to the views
    /// being placed, until [`Self::done_placing`]. Harmless on its own, since the link still
    /// names the view applied before. Returns the views named before, for
    /// [`Self::stopped_placing`]: a view an earlier update stopped part way placing stays
    /// named, since its files are still on disk until a later update finishes.
    pub fn begin_placing(&self, view: &Image) -> std::io::Result<Vec<String>> {
        self.keep_once(view)?;
        let before = self.placing_ids();
        let mut ids = before.clone();
        if !ids.contains(&view.id) {
            ids.push(view.id.clone());
        }
        let ids = &ids[ids.len().saturating_sub(MAX_PLACING)..];
        storage::write_atomic(&self.dir.join(PLACING), ids.join("\n").as_bytes())?;
        Ok(before)
    }

    /// The views updates were placing and didn't record (they stopped part way, or the
    /// Launcher stopped), oldest first, those still kept.
    pub fn placing(&self) -> Vec<Image> {
        self.placing_ids()
            .iter()
            .filter_map(|id| self.load(id))
            .collect()
    }

    /// An update failed having changed nothing: the views named are those named before it
    /// began (`before`, from [`Self::begin_placing`]), so a player's own copy of one of its
    /// files isn't later taken for the server's, while a view an earlier update stopped
    /// placing still counts.
    pub fn stopped_placing(&self, before: &[String]) {
        if before.is_empty() {
            self.done_placing();
        } else if let Err(err) =
            storage::write_atomic(&self.dir.join(PLACING), before.join("\n").as_bytes())
        {
            // naming one view too many only counts its files on disk as the server's
            eprintln!("Couldn't note which server views are being placed: {err}");
        }
    }

    /// The view an update placed is recorded: no view is being placed any more (the
    /// update's plan counted the files of those named).
    pub fn done_placing(&self) {
        let _ = fs::remove_file(self.dir.join(PLACING));
    }

    /// The ids [`PLACING`] names, oldest first: well-formed ones only, each once.
    fn placing_ids(&self) -> Vec<String> {
        let Some(bytes) = read_capped(&self.dir.join(PLACING)) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = Vec::new();
        for id in String::from_utf8_lossy(&bytes).lines().map(str::trim) {
            if is_sha256(id) && !ids.iter().any(|known| known == id) {
                ids.push(id.to_string());
            }
        }
        let start = ids.len().saturating_sub(MAX_PLACING);
        ids.split_off(start)
    }

    /// The ids of the kept views, newest first.
    fn ids(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut found: Vec<(std::time::SystemTime, String)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json").filter(|id| is_sha256(id))?;
                let modified = entry.metadata().ok()?.modified().ok()?;
                Some((modified, id.to_string()))
            })
            .collect();
        found.sort_by(|a, b| b.cmp(a));
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// What to send as `have`: the applied view first (when it is kept), then the others,
    /// newest first, at most [`MAX_HAVE`].
    pub fn have(&self, applied: Option<&str>) -> Vec<String> {
        let mut have: Vec<String> = Vec::new();
        if let Some(applied) = applied.filter(|id| self.path(id).is_file()) {
            have.push(applied.to_string());
        }
        for id in self.ids() {
            if have.len() >= MAX_HAVE {
                break;
            }
            if !have.contains(&id) {
                have.push(id);
            }
        }
        have
    }

    /// Deletes all but the newest [`KEPT_VIEWS`], never the applied one.
    pub fn prune(&self, applied: &str) {
        let mut kept = 1;
        for id in self.ids() {
            if id == applied {
                continue;
            }
            if kept < KEPT_VIEWS {
                kept += 1;
            } else {
                let _ = fs::remove_file(self.path(&id));
            }
        }
    }

    /// Deletes every kept view (the instance was unlinked from its server).
    pub fn clear(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn read_capped(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_MANIFEST_BYTES).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use lodestone_image::{
        CanonicalPath, Entry, Game, Kind, Loader, RULES, Source, hex::sha256_hex, view,
    };

    use super::*;

    fn client_view(mods: usize) -> Image {
        let mut image = Image::new(
            Kind::Content,
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: None,
            },
        );
        for i in 0..mods {
            let path = format!("mods/m{i}.jar");
            image.entries.push(Entry::new(
                CanonicalPath::new(&path).unwrap(),
                1,
                sha256_hex(path.as_bytes()),
                Source::Object {},
            ));
        }
        image.seal();
        view(&image, Side::Client, RULES).unwrap()
    }

    fn touch_later(views: &KeptViews, id: &str, seconds: u64) {
        let file = fs::File::options()
            .write(true)
            .open(views.path(id))
            .unwrap();
        file.set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000 + seconds),
        )
        .unwrap();
    }

    #[test]
    fn keeps_names_and_prunes_views() {
        let dir = tempfile::tempdir().unwrap();
        let views = KeptViews::open(dir.path());
        assert!(views.have(None).is_empty());
        let all: Vec<Image> = (1..=6).map(client_view).collect();
        for (i, view) in all.iter().enumerate() {
            views.keep(view).unwrap();
            touch_later(&views, &view.id, i as u64);
        }
        assert_eq!(views.load(&all[2].id).unwrap(), all[2]);
        // the applied view first, then newest first
        let have = views.have(Some(&all[1].id));
        assert_eq!(have[0], all[1].id);
        assert_eq!(have[1], all[5].id);
        assert_eq!(have.len(), 6);
        // an applied view that isn't kept isn't named
        assert_eq!(views.have(Some(&"e".repeat(64)))[0], all[5].id);

        // four are kept, the applied one whatever its age
        views.prune(&all[0].id);
        let mut kept = views.have(None);
        kept.sort();
        let mut expected = vec![
            all[0].id.clone(),
            all[3].id.clone(),
            all[4].id.clone(),
            all[5].id.clone(),
        ];
        expected.sort();
        assert_eq!(kept, expected);

        views.clear();
        assert!(views.have(None).is_empty());
    }

    /// An update that fails having changed nothing puts back the views named before it,
    /// so a view an earlier update stopped part way placing still counts; a finished one
    /// clears them all.
    #[test]
    fn a_failed_update_keeps_the_views_stopped_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let views = KeptViews::open(dir.path());
        let (two, three) = (client_view(2), client_view(3));
        assert!(views.begin_placing(&two).unwrap().is_empty());
        // the Launcher stopped part way: view 2 is still named when view 3 begins
        let before = views.begin_placing(&three).unwrap();
        assert_eq!(before, std::slice::from_ref(&two.id));
        let ids = |views: &KeptViews| -> Vec<String> {
            views.placing().into_iter().map(|view| view.id).collect()
        };
        assert_eq!(ids(&views), [two.id.clone(), three.id.clone()]);
        views.stopped_placing(&before);
        assert_eq!(ids(&views), std::slice::from_ref(&two.id));
        // beginning the same view again names it once
        views.begin_placing(&two).unwrap();
        assert_eq!(ids(&views), std::slice::from_ref(&two.id));
        views.stopped_placing(&[]);
        assert!(views.placing().is_empty());
        views.begin_placing(&two).unwrap();
        views.begin_placing(&three).unwrap();
        views.done_placing();
        assert!(views.placing().is_empty());
        // junk lines and too many ids: only the newest well-formed ones
        let many: Vec<String> = (0..20).map(|i| format!("{i:064x}")).collect();
        fs::write(
            dir.path().join(VIEWS_FOLDER).join(PLACING),
            format!("../escape\n{}\n{}", many.join("\n"), two.id),
        )
        .unwrap();
        let named = views.placing_ids();
        assert_eq!(named.len(), MAX_PLACING);
        assert_eq!(named.last(), Some(&two.id));
        assert_eq!(ids(&views), std::slice::from_ref(&two.id));
    }

    #[test]
    fn damaged_views_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let views = KeptViews::open(dir.path());
        let view = client_view(1);
        views.keep(&view).unwrap();
        // contents that aren't the view its name says
        let other = client_view(2);
        fs::write(views.path(&view.id), other.to_json_pretty()).unwrap();
        assert!(views.load(&view.id).is_none());
        assert!(!views.path(&view.id).exists());
        // not JSON
        views.keep(&view).unwrap();
        fs::write(views.path(&view.id), b"{broken").unwrap();
        assert!(views.load(&view.id).is_none());
        assert!(views.have(None).is_empty());
        // a name that isn't an id is never read
        assert!(views.load("../x").is_none());
    }
}
