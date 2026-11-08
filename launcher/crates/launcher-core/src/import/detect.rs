//! Finding other launchers' instances: in each launcher's default folder on macOS or
//! Windows, or in a folder the user picks (a launcher's data folder or one instance's
//! folder). MultiMC on Windows and portable installs keep their data beside the app, so
//! they're only found when picked. Only reads.

use std::path::{Path, PathBuf};

use super::{
    Found, Source, atlauncher, curseforge, gdlauncher, modrinth_app, prism, subfolders, vanilla,
};

/// One launcher's instances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launcher {
    pub source: Source,
    /// The launcher's data folder (CurseForge: its `Instances` folder).
    pub root: PathBuf,
    pub instances: Vec<Found>,
    /// Instances or files that couldn't be read.
    pub problems: Vec<String>,
}

impl Launcher {
    pub(crate) fn new(source: Source, root: &Path) -> Self {
        Self {
            source,
            root: root.to_path_buf(),
            instances: Vec::new(),
            problems: Vec::new(),
        }
    }
}

/// Where to look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    pub home: PathBuf,
    /// Where apps keep the user's data: `~/Library/Application Support` on macOS, the
    /// roaming `AppData` folder (`%APPDATA%`) on Windows.
    pub app_data: PathBuf,
    /// The user's Documents folder, wherever it was moved (into OneDrive, say).
    pub documents: PathBuf,
    /// Folders of macOS app bundles.
    pub applications: Vec<PathBuf>,
}

impl Locations {
    /// This computer's, from the system's known folders; on macOS the app bundles in
    /// `/Applications` and `~/Applications` too.
    pub fn current() -> Option<Self> {
        let home = dirs::home_dir()?;
        let applications = if cfg!(target_os = "macos") {
            vec![PathBuf::from("/Applications"), home.join("Applications")]
        } else {
            Vec::new()
        };
        Some(Self {
            app_data: dirs::data_dir()?,
            documents: dirs::document_dir().unwrap_or_else(|| home.join("Documents")),
            applications,
            home,
        })
    }

    /// Every default folder, by launcher, whether it exists or not.
    pub fn candidates(&self) -> Vec<(Source, PathBuf)> {
        let data = |name: &str| self.app_data.join(name);
        let curseforge = |base: &Path| base.join("curseforge").join("minecraft").join("Instances");
        let mut list = vec![
            // The official launcher's folder is `minecraft` on macOS, `.minecraft` on Windows.
            (Source::Vanilla, data("minecraft")),
            (Source::Vanilla, data(".minecraft")),
            (Source::Prism, data("PrismLauncher")),
            (Source::Prism, data("PolyMC")),
            (Source::MultiMc, data("MultiMC")),
            (Source::ModrinthApp, data("ModrinthApp")),
            (Source::ModrinthApp, data("com.modrinth.theseus")),
            // Where ATLauncher's Windows installer puts it; its data is beside it.
            (Source::AtLauncher, data("ATLauncher")),
            (Source::CurseForge, curseforge(&self.documents)),
            (Source::CurseForge, curseforge(&self.home)),
            (Source::GdLauncher, data("gdlauncher_next")),
            (Source::GdLauncherCarbon, data("gdlauncher_carbon")),
        ];
        for apps in &self.applications {
            list.push((Source::MultiMc, apps.join("MultiMC.app/Data")));
            list.push((
                Source::AtLauncher,
                apps.join("ATLauncher.app/Contents/Java"),
            ));
        }
        list
    }
}

/// Whether `root` looks like `source`'s data folder.
pub fn is_root(source: Source, root: &Path) -> bool {
    if !root.is_dir() {
        return false;
    }
    match source {
        Source::Vanilla => root.join(vanilla::PROFILES_FILE).is_file(),
        Source::Prism | Source::MultiMc => {
            let prism_cfg = ["prismlauncher.cfg", "polymc.cfg"]
                .iter()
                .any(|f| root.join(f).is_file());
            let multimc_cfg = root.join("multimc.cfg").is_file();
            let has_instances = || {
                subfolders(&root.join("instances"))
                    .iter()
                    .any(|d| d.join("instance.cfg").is_file())
            };
            match source {
                // A MultiMC folder that Prism hasn't taken over.
                Source::MultiMc => multimc_cfg && !prism_cfg,
                _ => prism_cfg || (!multimc_cfg && has_instances()),
            }
        }
        Source::ModrinthApp => {
            root.join(modrinth_app::DATABASE).is_file()
                || subfolders(&root.join("profiles"))
                    .iter()
                    .any(|d| d.join("profile.json").is_file())
        }
        Source::AtLauncher => subfolders(&root.join("instances"))
            .iter()
            .any(|d| atlauncher::is_instance(d)),
        Source::CurseForge => subfolders(root)
            .iter()
            .any(|d| d.join(curseforge::INSTANCE_FILE).is_file()),
        Source::GdLauncher => {
            root.join("override.data").is_file()
                || subfolders(&root.join("instances"))
                    .iter()
                    .any(|d| gdlauncher::is_legacy_instance(d))
        }
        Source::GdLauncherCarbon => {
            root.join("runtime_path_override").is_file() || root.join("data/instances").is_dir()
        }
    }
}

/// Reads `source`'s data folder `root`.
pub fn read_root(source: Source, root: &Path) -> Launcher {
    match source {
        Source::Vanilla => vanilla::read(root),
        Source::Prism | Source::MultiMc => prism::read(root, source),
        Source::ModrinthApp => modrinth_app::read(root),
        Source::AtLauncher => atlauncher::read(root),
        Source::CurseForge => curseforge::read(root),
        Source::GdLauncher => gdlauncher::read_legacy(root),
        Source::GdLauncherCarbon => gdlauncher::read_carbon(root),
    }
}

/// The launchers installed in their default places, with their instances. Launchers whose
/// folder exists are listed even with no instances.
pub fn detect(locations: &Locations) -> Vec<Launcher> {
    let mut found: Vec<Launcher> = Vec::new();
    for (source, root) in locations.candidates() {
        if !is_root(source, &root) {
            continue;
        }
        let canonical = std::fs::canonicalize(&root).unwrap_or(root.clone());
        if found
            .iter()
            .any(|l| std::fs::canonicalize(&l.root).unwrap_or(l.root.clone()) == canonical)
        {
            continue;
        }
        found.push(read_root(source, &root));
    }
    found
}

/// Reads a folder the user picked: a launcher's data folder, an app bundle
/// (`MultiMC.app`, `ATLauncher.app`), CurseForge's `minecraft` or `Instances` folder, or a
/// single instance's folder.
pub fn read_folder(path: &Path) -> Result<Vec<Launcher>, String> {
    let mut roots: Vec<PathBuf> = vec![path.to_path_buf()];
    for inner in ["Data", "Contents/Java", "Instances", "minecraft/Instances"] {
        roots.push(path.join(inner));
    }
    let mut launchers = Vec::new();
    for root in &roots {
        for source in Source::ALL {
            if is_root(source, root) {
                let launcher = read_root(source, root);
                if !launcher.instances.is_empty() || !launcher.problems.is_empty() {
                    launchers.push(launcher);
                }
            }
        }
        if !launchers.is_empty() {
            return Ok(launchers);
        }
    }
    let single = |source: Source, found: Result<Found, String>| -> Result<Vec<Launcher>, String> {
        let mut launcher = Launcher::new(source, path);
        launcher.instances.push(found?);
        Ok(vec![launcher])
    };
    if path.join("instance.cfg").is_file() {
        let icons = path
            .parent()
            .and_then(Path::parent)
            .map(|data| data.join("icons"));
        let source = if path.join(".minecraft").exists() && !path.join("minecraft").exists() {
            Source::MultiMc
        } else {
            Source::Prism
        };
        return single(source, prism::read_instance(path, source, icons.as_deref()));
    }
    if path.join(curseforge::INSTANCE_FILE).is_file() {
        return single(Source::CurseForge, curseforge::read_instance(path));
    }
    if atlauncher::is_instance(path) {
        return single(Source::AtLauncher, atlauncher::read_instance(path));
    }
    if gdlauncher::is_carbon_instance(path) {
        return single(
            Source::GdLauncherCarbon,
            gdlauncher::read_carbon_instance(path),
        );
    }
    if gdlauncher::is_legacy_instance(path) {
        return single(Source::GdLauncher, gdlauncher::read_legacy_instance(path));
    }
    if path.join("profile.json").is_file() {
        return single(
            Source::ModrinthApp,
            modrinth_app::read_profile_json(path, &[path]),
        );
    }
    Err(format!(
        "{} isn't a launcher's folder or an instance Lodestone Launcher can read.",
        path.display()
    ))
}
