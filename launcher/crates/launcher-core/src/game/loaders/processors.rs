//! The processors of a Forge (1.13 and later) or NeoForge installer: small Java
//! programs that split, rename and patch the Minecraft jar into the jars the loader
//! launches. This is what the official installer does after "Install client", without
//! its window, and what other launchers (MultiMC, Prism, ATLauncher) do too.
//!
//! Each processor is `java -cp <its jar and classpath> <Main-Class> <args>`, with:
//! - `[group:artifact:version]` arguments as paths in the libraries directory;
//! - `{KEY}` replaced by the `data` entry `KEY` (a `[maven]` path, a `'literal'`, or a
//!   file inside the installer such as `/data/client.lzma`, extracted first) or by
//!   one of `SIDE`, `MINECRAFT_JAR`, `MINECRAFT_VERSION`, `ROOT`, `INSTALLER` and
//!   `LIBRARY_DIR`;
//! - `outputs`: files whose SHA-1 is checked after the run. A processor whose outputs
//!   are all there with the right SHA-1 is skipped, like the official installer does.

use std::{
    collections::BTreeMap,
    io::{BufRead as _, BufReader, Read as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use super::{
    LoaderError,
    forge::{InstallProfile, Processor},
};
use crate::game::{
    download::{CancelToken, file_matches},
    libraries::{maven_path, safe_relative_path},
    platform::Os,
};

/// The `{...}` values a processor's arguments use.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values {
    values: BTreeMap<String, String>,
    /// The values that are Maven paths: the files the processors make.
    artifacts: Vec<PathBuf>,
}

/// What the built-in values point at.
#[derive(Clone, Debug)]
pub struct Builtins<'a> {
    pub minecraft_jar: &'a Path,
    pub minecraft_version: &'a str,
    /// The data directory, `{ROOT}`.
    pub root: &'a Path,
    pub installer: &'a Path,
    pub libraries_dir: &'a Path,
}

fn invalid(why: impl std::fmt::Display) -> LoaderError {
    LoaderError::Invalid(format!("the installer's processors are invalid: {why}"))
}

/// The path of a Maven artifact in the libraries directory.
pub fn artifact_path(libraries_dir: &Path, coordinate: &str) -> Result<PathBuf, LoaderError> {
    let path = maven_path(coordinate, None).map_err(invalid)?;
    Ok(libraries_dir.join(safe_relative_path(&path).map_err(invalid)?))
}

fn path_string(path: &Path) -> Result<String, LoaderError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| invalid(format!("{} is not valid UTF-8", path.display())))
}

impl Values {
    /// The values of `profile`'s `data`, with the installer's own files (`/data/...`)
    /// at the paths `extracted` gives them.
    pub fn new(
        profile: &InstallProfile,
        builtins: &Builtins<'_>,
        mut extracted: impl FnMut(&str) -> Result<PathBuf, LoaderError>,
    ) -> Result<Self, LoaderError> {
        let mut values = BTreeMap::new();
        let mut artifacts = Vec::new();
        for (key, entry) in &profile.data {
            let value = &entry.client;
            let resolved = if let Some(coordinate) =
                value.strip_prefix('[').and_then(|v| v.strip_suffix(']'))
            {
                let path = artifact_path(builtins.libraries_dir, coordinate)?;
                let text = path_string(&path)?;
                artifacts.push(path);
                text
            } else if let Some(literal) =
                value.strip_prefix('\'').and_then(|v| v.strip_suffix('\''))
            {
                literal.to_string()
            } else if value.starts_with('/') {
                path_string(&extracted(value)?)?
            } else {
                value.clone()
            };
            values.insert(key.clone(), resolved);
        }
        for (key, value) in [
            ("SIDE", "client".to_string()),
            ("MINECRAFT_JAR", path_string(builtins.minecraft_jar)?),
            ("MINECRAFT_VERSION", builtins.minecraft_version.to_string()),
            ("ROOT", path_string(builtins.root)?),
            ("INSTALLER", path_string(builtins.installer)?),
            ("LIBRARY_DIR", path_string(builtins.libraries_dir)?),
        ] {
            values.insert(key.to_string(), value);
        }
        Ok(Self { values, artifacts })
    }

    /// The files named by `[maven]` data entries, which the processors make.
    pub fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }

    /// `arg` with its placeholders replaced: a whole-argument `[maven]` coordinate
    /// becomes its path, a `'literal'` its text, and each `{KEY}` its value. A `\`
    /// escapes the next character.
    pub fn substitute(&self, arg: &str, libraries_dir: &Path) -> Result<String, LoaderError> {
        if let Some(coordinate) = arg.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return path_string(&artifact_path(libraries_dir, coordinate)?);
        }
        if let Some(literal) = arg.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            return Ok(literal.to_string());
        }
        let mut out = String::with_capacity(arg.len());
        let mut chars = arg.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.extend(chars.next()),
                '{' => {
                    let key: String = chars.by_ref().take_while(|c| *c != '}').collect();
                    let value = self
                        .values
                        .get(&key)
                        .ok_or_else(|| invalid(format!("unknown value {{{key}}}")))?;
                    out.push_str(value);
                }
                c => out.push(c),
            }
        }
        Ok(out)
    }
}

/// A processor ready to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub jar: PathBuf,
    /// The jar, then its classpath.
    pub classpath: Vec<PathBuf>,
    pub args: Vec<String>,
    /// The files it makes, with their SHA-1.
    pub outputs: Vec<(PathBuf, String)>,
}

impl Planned {
    /// Whether every output is already there with its SHA-1. Processors without
    /// outputs always run.
    pub fn is_done(&self) -> bool {
        !self.outputs.is_empty()
            && self
                .outputs
                .iter()
                .all(|(path, sha1)| file_matches(path, sha1))
    }
}

/// The client's processors of `profile`, with their arguments filled in.
pub fn plan(
    profile: &InstallProfile,
    values: &Values,
    libraries_dir: &Path,
) -> Result<Vec<Planned>, LoaderError> {
    profile
        .client_processors()
        .map(|processor| plan_one(processor, values, libraries_dir))
        .collect()
}

fn plan_one(
    processor: &Processor,
    values: &Values,
    libraries_dir: &Path,
) -> Result<Planned, LoaderError> {
    let jar = artifact_path(libraries_dir, &processor.jar)?;
    let mut classpath = vec![jar.clone()];
    for coordinate in &processor.classpath {
        classpath.push(artifact_path(libraries_dir, coordinate)?);
    }
    let args = processor
        .args
        .iter()
        .map(|arg| values.substitute(arg, libraries_dir))
        .collect::<Result<_, _>>()?;
    let outputs = processor
        .outputs
        .iter()
        .map(|(path, sha1)| {
            Ok((
                PathBuf::from(values.substitute(path, libraries_dir)?),
                values.substitute(sha1, libraries_dir)?,
            ))
        })
        .collect::<Result<_, LoaderError>>()?;
    Ok(Planned {
        jar,
        classpath,
        args,
        outputs,
    })
}

/// The `Main-Class` of a jar's `META-INF/MANIFEST.MF`.
pub fn main_class(jar: &Path) -> Result<String, LoaderError> {
    let file = std::fs::File::open(jar)
        .map_err(|err| invalid(format!("couldn't open {}: {err}", jar.display())))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| invalid(format!("{} is not a jar: {err}", jar.display())))?;
    let mut manifest = String::new();
    archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|err| invalid(format!("{} has no manifest: {err}", jar.display())))?
        .take(1024 * 1024)
        .read_to_string(&mut manifest)
        .map_err(|err| {
            invalid(format!(
                "couldn't read the manifest of {}: {err}",
                jar.display()
            ))
        })?;
    parse_main_class(&manifest)
        .ok_or_else(|| invalid(format!("{} has no Main-Class", jar.display())))
}

/// The `Main-Class` of a manifest, whose long lines continue on lines starting with a
/// space.
pub fn parse_main_class(manifest: &str) -> Option<String> {
    let mut joined: Vec<String> = Vec::new();
    for line in manifest.lines() {
        let line = line.trim_end_matches('\r');
        match (line.strip_prefix(' '), joined.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => joined.push(line.to_string()),
        }
    }
    joined
        .iter()
        .find_map(|line| line.strip_prefix("Main-Class:"))
        .map(|class| class.trim().to_string())
        .filter(|class| !class.is_empty())
}

/// `paths` as the `-cp` of a Java on `os`. On Windows the separator is `;`, since
/// every absolute path has a `:` after its drive letter.
fn classpath(paths: &[PathBuf], os: Os) -> Result<String, LoaderError> {
    Ok(paths
        .iter()
        .map(|path| path_string(path))
        .collect::<Result<Vec<_>, _>>()?
        .join(os.classpath_separator()))
}

/// Runs one processor with `java` in `cwd`, passing each line it prints to `on_line`.
/// Checks its outputs afterwards.
pub fn run(
    java: &Path,
    processor: &Planned,
    cwd: &Path,
    cancel: &CancelToken,
    on_line: &(dyn Fn(String) + Sync),
) -> Result<(), LoaderError> {
    let main = main_class(&processor.jar)?;
    let classpath = classpath(&processor.classpath, Os::current())?;
    let mut child = Command::new(java)
        .arg("-cp")
        .arg(classpath)
        .arg(&main)
        .args(&processor.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| LoaderError::Invalid(format!("couldn't start {}: {err}", java.display())))?;
    let status = std::thread::scope(|scope| {
        let pipes: Vec<Box<dyn std::io::Read + Send>> = [
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .collect();
        for pipe in pipes {
            scope.spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    on_line(line);
                }
            });
        }
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if cancel.is_cancelled() => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(None);
                }
                // A processor takes seconds; checking for cancel ten times a second
                // costs nothing.
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(err) => return Err(Some(err)),
            }
        }
    });
    let status = match status {
        Ok(status) => status,
        Err(None) => return Err(LoaderError::Cancelled),
        Err(Some(err)) => {
            return Err(LoaderError::Invalid(format!(
                "waiting for the processor {main}: {err}"
            )));
        }
    };
    if !status.success() {
        return Err(LoaderError::Invalid(format!(
            "the installer's processor {main} failed ({status})"
        )));
    }
    for (path, sha1) in &processor.outputs {
        if !file_matches(path, sha1) {
            return Err(LoaderError::Invalid(format!(
                "the processor {main} made {} with the wrong SHA-1",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::loaders::tests::loader_fixture;

    fn builtins<'a>(libraries: &'a Path) -> Builtins<'a> {
        Builtins {
            minecraft_jar: Path::new("/data/versions/1.20.1/1.20.1.jar"),
            minecraft_version: "1.20.1",
            root: Path::new("/data"),
            installer: Path::new("/data/libraries/forge-installer.jar"),
            libraries_dir: libraries,
        }
    }

    #[test]
    fn plans_forge_1_20_1() {
        let profile =
            InstallProfile::parse(&loader_fixture("forge/1.20.1-47.4.28/install_profile.json"))
                .unwrap();
        let libraries = Path::new("/data/libraries");
        let mut extracted = Vec::new();
        let values = Values::new(&profile, &builtins(libraries), |name| {
            extracted.push(name.to_string());
            Ok(PathBuf::from("/tmp/work").join(name.trim_start_matches('/')))
        })
        .unwrap();
        assert_eq!(extracted, ["/data/client.lzma"]);
        let planned = plan(&profile, &values, libraries).unwrap();
        assert_eq!(planned.len(), 6);
        let mcp = &planned[0];
        assert_eq!(
            mcp.jar,
            Path::new(
                "/data/libraries/net/minecraftforge/installertools/1.4.1/installertools-1.4.1.jar"
            )
        );
        assert_eq!(mcp.classpath.len(), 8);
        assert_eq!(
            mcp.args,
            [
                "--task",
                "MCP_DATA",
                "--input",
                "/data/libraries/de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412.zip",
                "--output",
                "/data/libraries/de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt",
                "--key",
                "mappings",
            ]
        );
        assert_eq!(planned[1].args[5], "client", "{{SIDE}}");
        let split = &planned[3];
        assert_eq!(split.args[1], "/data/versions/1.20.1/1.20.1.jar");
        assert_eq!(
            split.outputs,
            [
                (
                    PathBuf::from(
                        "/data/libraries/net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412-extra.jar"
                    ),
                    "8c5a95cbce940cfdb304376ae9fea47968d02587".to_string()
                ),
                (
                    PathBuf::from(
                        "/data/libraries/net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412-slim.jar"
                    ),
                    "de86b035d2da0f78940796bb95c39a932ed84834".to_string()
                ),
            ]
        );
        let patch = &planned[5];
        assert_eq!(patch.args[5], "/tmp/work/data/client.lzma");
        assert_eq!(
            patch.args[3],
            "/data/libraries/net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar"
        );
        assert!(values.artifacts().iter().any(|p| {
            p.ends_with("net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar")
        }));
        assert!(!planned[0].is_done(), "no outputs, so it always runs");
    }

    #[test]
    fn plans_neoforge() {
        let profile =
            InstallProfile::parse(&loader_fixture("neoforge/21.1.209/install_profile.json"))
                .unwrap();
        let libraries = Path::new("/data/libraries");
        let values = Values::new(&profile, &builtins(libraries), |name| {
            Ok(PathBuf::from("/w").join(name.trim_start_matches('/')))
        })
        .unwrap();
        let planned = plan(&profile, &values, libraries).unwrap();
        assert_eq!(planned.len(), 6);
        assert!(planned[5].jar.ends_with("binarypatcher-2.1.2-fatjar.jar"));
    }

    #[test]
    fn substitution() {
        let profile: InstallProfile = serde_json::from_value(serde_json::json!({
            "data": {"KEY": {"client": "'lit'", "server": ""}, "PATH": {"client": "[a.b:c:1:d@txt]", "server": ""}}
        }))
        .unwrap();
        let libraries = Path::new("/l");
        let values = Values::new(&profile, &builtins(libraries), |_| unreachable!()).unwrap();
        assert_eq!(values.substitute("{KEY}", libraries).unwrap(), "lit");
        assert_eq!(
            values.substitute("x={KEY}/{SIDE}", libraries).unwrap(),
            "x=lit/client"
        );
        assert_eq!(values.substitute("\\{KEY}", libraries).unwrap(), "{KEY}");
        assert_eq!(
            values.substitute("{PATH}", libraries).unwrap(),
            "/l/a/b/c/1/c-1-d.txt"
        );
        assert_eq!(values.substitute("'{PATH}'", libraries).unwrap(), "{PATH}");
        assert!(values.substitute("{NOPE}", libraries).is_err());
        assert!(values.substitute("[../../x:y:1]", libraries).is_err());
    }

    #[test]
    fn classpaths() {
        let windows = [
            PathBuf::from(r"C:\Users\a\libraries\installertools.jar"),
            PathBuf::from(r"C:\Users\a\libraries\jopt-simple.jar"),
        ];
        assert_eq!(
            classpath(&windows, Os::Windows).unwrap(),
            r"C:\Users\a\libraries\installertools.jar;C:\Users\a\libraries\jopt-simple.jar"
        );
        let unix = [PathBuf::from("/l/a.jar"), PathBuf::from("/l/b.jar")];
        assert_eq!(classpath(&unix, Os::MacOs).unwrap(), "/l/a.jar:/l/b.jar");
        assert_eq!(classpath(&unix, Os::Linux).unwrap(), "/l/a.jar:/l/b.jar");
    }

    #[test]
    fn manifests() {
        assert_eq!(
            parse_main_class(
                "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.binarypatcher.Con\r\n sole\r\n"
            ),
            Some("net.minecraftforge.binarypatcher.Console".into())
        );
        assert_eq!(parse_main_class("Manifest-Version: 1.0\n"), None);
    }
}
