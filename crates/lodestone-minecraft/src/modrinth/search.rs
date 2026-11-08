//! Search parameters, and the Modrinth `facets` built from them.

use super::types::{ModrinthProjectType, ModrinthSearchIndex};
use crate::Error;

pub const DEFAULT_SEARCH_LIMIT: u32 = 20;
pub const MAX_SEARCH_LIMIT: u32 = 100;
const MAX_QUERY_CHARS: usize = 256;
/// Modrinth does not page past this many results.
const MAX_SEARCH_OFFSET: u32 = 100_000;

/// A checked search, ready to send to Modrinth.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchParams {
    pub query: Option<String>,
    pub project_type: ModrinthProjectType,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub index: ModrinthSearchIndex,
    pub offset: u32,
    pub limit: u32,
}

fn bad_request(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/// Whether `value` is a plausible game version or loader name, such as `1.20.1`,
/// `24w14a` or `fabric`. Such values go into facets and URL parameters.
pub fn is_valid_filter_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

fn optional_filter(value: Option<String>, what: &str) -> Result<Option<String>, Error> {
    match value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        Some(v) if is_valid_filter_value(&v) => Ok(Some(v)),
        Some(_) => Err(bad_request(format!("Invalid {what}"))),
        None => Ok(None),
    }
}

fn check_query(query: Option<String>) -> Result<Option<String>, Error> {
    let query = query
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    if let Some(q) = &query {
        if q.chars().count() > MAX_QUERY_CHARS {
            return Err(bad_request(format!(
                "The search text cannot be longer than {MAX_QUERY_CHARS} characters"
            )));
        }
        if q.chars().any(|c| c.is_control()) {
            return Err(bad_request(
                "The search text cannot contain control characters",
            ));
        }
    }
    Ok(query)
}

fn check_offset(offset: Option<u32>) -> Result<u32, Error> {
    let offset = offset.unwrap_or(0);
    if offset > MAX_SEARCH_OFFSET {
        return Err(bad_request("The search offset is too large"));
    }
    Ok(offset)
}

/// The query string for Modrinth's `GET /search`.
fn search_query_pairs(
    query: &Option<String>,
    facets: &[Vec<String>],
    index: ModrinthSearchIndex,
    offset: u32,
    limit: u32,
) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    if let Some(q) = query {
        pairs.push(("query", q.clone()));
    }
    pairs.push(("facets", serde_json::to_string(facets).unwrap_or_default()));
    pairs.push(("index", index.as_str().to_string()));
    pairs.push(("offset", offset.to_string()));
    pairs.push(("limit", limit.to_string()));
    pairs
}

impl SearchParams {
    /// Checks the values a client sent. `limit` is capped at [`MAX_SEARCH_LIMIT`].
    pub fn new(
        query: Option<String>,
        project_type: ModrinthProjectType,
        game_version: Option<String>,
        loader: Option<String>,
        index: Option<ModrinthSearchIndex>,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Self, Error> {
        let query = check_query(query)?;
        let offset = check_offset(offset)?;
        Ok(Self {
            query,
            project_type,
            game_version: optional_filter(game_version, "game version")?,
            loader: optional_filter(loader, "loader")?.map(|l| l.to_lowercase()),
            index: index.unwrap_or_default(),
            offset,
            limit: limit
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT),
        })
    }

    /// The Modrinth facets: AND of the outer list, OR inside each inner list. A server-side
    /// facet is always added, so projects that only run on the client are hidden.
    pub fn facets(&self) -> Vec<Vec<String>> {
        let mut facets = vec![vec![format!("project_type:{}", self.project_type.as_str())]];
        // Modrinth reports plugins as `project_type: "mod"` with plugin loaders, so a
        // plugin search also asks for a server plugin loader
        let loaders = match (&self.loader, self.project_type) {
            (Some(loader), _) => compatible_loaders(loader),
            (None, ModrinthProjectType::Plugin) => {
                PLUGIN_LOADERS.iter().map(|l| l.to_string()).collect()
            }
            (None, _) => Vec::new(),
        };
        if !loaders.is_empty() {
            facets.push(
                loaders
                    .into_iter()
                    .map(|l| format!("categories:{l}"))
                    .collect(),
            );
        }
        if let Some(game_version) = &self.game_version {
            facets.push(vec![format!("versions:{game_version}")]);
        }
        facets.push(vec![
            "server_side:required".to_string(),
            "server_side:optional".to_string(),
        ]);
        facets
    }

    /// The query string for Modrinth's `GET /search`.
    pub fn query_pairs(&self) -> Vec<(&'static str, String)> {
        search_query_pairs(
            &self.query,
            &self.facets(),
            self.index,
            self.offset,
            self.limit,
        )
    }
}

/// What a game client's folder holds, as Modrinth project types: mods, resource packs and
/// shader packs, and the modpacks instances are made from. Lodestone Launcher browses these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientProjectType {
    Mod,
    ResourcePack,
    Shader,
    Modpack,
}

impl ClientProjectType {
    /// The Modrinth `project_type`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientProjectType::Mod => "mod",
            ClientProjectType::ResourcePack => "resourcepack",
            ClientProjectType::Shader => "shader",
            ClientProjectType::Modpack => "modpack",
        }
    }

    pub fn from_project_type(project_type: &str) -> Option<Self> {
        match project_type {
            "mod" => Some(ClientProjectType::Mod),
            "resourcepack" => Some(ClientProjectType::ResourcePack),
            "shader" => Some(ClientProjectType::Shader),
            "modpack" => Some(ClientProjectType::Modpack),
            _ => None,
        }
    }
}

/// A checked search for content that runs on a game client. Unlike [`SearchParams`], it
/// has no server-side facet; mod searches hide mods that don't run on the client instead.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientSearchParams {
    pub query: Option<String>,
    pub project_type: ClientProjectType,
    pub game_version: Option<String>,
    /// Any of these Modrinth loaders (categories). Empty for no loader filter.
    pub loaders: Vec<String>,
    pub index: ModrinthSearchIndex,
    pub offset: u32,
    pub limit: u32,
}

impl ClientSearchParams {
    /// Checks the values, as [`SearchParams::new`] does.
    pub fn new(
        query: Option<String>,
        project_type: ClientProjectType,
        game_version: Option<String>,
        loaders: Vec<String>,
        index: Option<ModrinthSearchIndex>,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Self, Error> {
        let mut checked = Vec::new();
        for loader in loaders {
            if let Some(loader) = optional_filter(Some(loader), "loader")? {
                let loader = loader.to_lowercase();
                if !checked.contains(&loader) {
                    checked.push(loader);
                }
            }
        }
        Ok(Self {
            query: check_query(query)?,
            project_type,
            game_version: optional_filter(game_version, "game version")?,
            loaders: checked,
            index: index.unwrap_or_default(),
            offset: check_offset(offset)?,
            limit: limit
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT),
        })
    }

    /// The Modrinth facets: the project type, the loaders (any of them), the game version,
    /// and for mods and modpacks a client-side facet so server-only ones are hidden.
    pub fn facets(&self) -> Vec<Vec<String>> {
        let mut facets = vec![vec![format!("project_type:{}", self.project_type.as_str())]];
        if !self.loaders.is_empty() {
            facets.push(
                self.loaders
                    .iter()
                    .map(|l| format!("categories:{l}"))
                    .collect(),
            );
        }
        if let Some(game_version) = &self.game_version {
            facets.push(vec![format!("versions:{game_version}")]);
        }
        if matches!(
            self.project_type,
            ClientProjectType::Mod | ClientProjectType::Modpack
        ) {
            facets.push(vec![
                "client_side:required".to_string(),
                "client_side:optional".to_string(),
            ]);
        }
        facets
    }

    /// The query string for Modrinth's `GET /search`.
    pub fn query_pairs(&self) -> Vec<(&'static str, String)> {
        search_query_pairs(
            &self.query,
            &self.facets(),
            self.index,
            self.offset,
            self.limit,
        )
    }
}

/// The Modrinth loaders of plugins for the servers Lodestone runs (Paper and Spigot, and
/// the Paper forks Folia and Purpur).
pub const PLUGIN_LOADERS: [&str; 5] = ["paper", "spigot", "bukkit", "folia", "purpur"];

/// The Modrinth loaders whose files run on a server with `loader`. Paper also runs Spigot
/// and Bukkit plugins, and Spigot runs Bukkit plugins.
pub fn compatible_loaders(loader: &str) -> Vec<String> {
    match loader {
        "paper" => vec!["paper".into(), "spigot".into(), "bukkit".into()],
        "spigot" => vec!["spigot".into(), "bukkit".into()],
        other => vec![other.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(
        project_type: ModrinthProjectType,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> SearchParams {
        SearchParams::new(
            Some("sodium".into()),
            project_type,
            game_version.map(String::from),
            loader.map(String::from),
            None,
            None,
            None,
        )
        .unwrap()
    }

    const SERVER_SIDE: [&str; 2] = ["server_side:required", "server_side:optional"];

    #[test]
    fn server_side_facet_is_always_present() {
        for project_type in [
            ModrinthProjectType::Mod,
            ModrinthProjectType::Plugin,
            ModrinthProjectType::Modpack,
        ] {
            for (gv, loader) in [
                (None, None),
                (Some("1.20.1"), None),
                (None, Some("fabric")),
                (Some("1.20.1"), Some("paper")),
            ] {
                let facets = params(project_type, gv, loader).facets();
                assert!(
                    facets.iter().any(|f| f == &SERVER_SIDE),
                    "{project_type:?} {gv:?} {loader:?}: {facets:?}"
                );
                assert_eq!(
                    facets[0],
                    vec![format!("project_type:{}", project_type.as_str())]
                );
            }
        }
    }

    #[test]
    fn facets_for_filters() {
        let facets = params(ModrinthProjectType::Mod, Some("1.20.1"), Some("Fabric")).facets();
        assert_eq!(
            facets,
            vec![
                vec!["project_type:mod".to_string()],
                vec!["categories:fabric".to_string()],
                vec!["versions:1.20.1".to_string()],
                SERVER_SIDE.map(String::from).to_vec(),
            ]
        );
        let facets = params(ModrinthProjectType::Plugin, None, Some("paper")).facets();
        assert_eq!(
            facets[1],
            vec!["categories:paper", "categories:spigot", "categories:bukkit"]
        );
        // a plugin search without a loader still asks for plugin loaders
        let facets = params(ModrinthProjectType::Plugin, None, None).facets();
        assert_eq!(facets[0], vec!["project_type:plugin"]);
        assert_eq!(
            facets[1],
            PLUGIN_LOADERS
                .iter()
                .map(|l| format!("categories:{l}"))
                .collect::<Vec<_>>()
        );
        // mods without a loader get no loader facet
        assert_eq!(
            params(ModrinthProjectType::Mod, None, None).facets().len(),
            2
        );
    }

    #[test]
    fn query_pairs_encode_facets_as_json() {
        let pairs = params(ModrinthProjectType::Mod, Some("1.20.1"), None).query_pairs();
        let facets = pairs.iter().find(|(k, _)| *k == "facets").unwrap();
        let parsed: Vec<Vec<String>> = serde_json::from_str(&facets.1).unwrap();
        assert_eq!(parsed.len(), 3);
        assert!(pairs.contains(&("index", "relevance".to_string())));
        assert!(pairs.contains(&("limit", "20".to_string())));
        assert!(pairs.contains(&("offset", "0".to_string())));
        assert!(pairs.contains(&("query", "sodium".to_string())));
    }

    #[test]
    fn client_search_facets() {
        let new = |project_type, gv: Option<&str>, loaders: &[&str]| {
            ClientSearchParams::new(
                Some("x".into()),
                project_type,
                gv.map(String::from),
                loaders.iter().map(|l| l.to_string()).collect(),
                None,
                None,
                None,
            )
        };
        let mods = new(
            ClientProjectType::Mod,
            Some("1.20.1"),
            &["Quilt", "fabric", "quilt"],
        )
        .unwrap()
        .facets();
        assert_eq!(
            mods,
            vec![
                vec!["project_type:mod".to_string()],
                vec![
                    "categories:quilt".to_string(),
                    "categories:fabric".to_string()
                ],
                vec!["versions:1.20.1".to_string()],
                vec![
                    "client_side:required".to_string(),
                    "client_side:optional".to_string()
                ],
            ]
        );
        // no server-side facet anywhere: shader and resource packs are client-only
        for project_type in [ClientProjectType::ResourcePack, ClientProjectType::Shader] {
            let facets = new(project_type, None, &[]).unwrap().facets();
            assert_eq!(
                facets,
                vec![vec![format!("project_type:{}", project_type.as_str())]]
            );
        }
        let shaders = new(
            ClientProjectType::Shader,
            Some("1.21.1"),
            &["iris", "optifine"],
        )
        .unwrap()
        .facets();
        assert_eq!(shaders[1], vec!["categories:iris", "categories:optifine"]);
        assert!(new(ClientProjectType::Mod, None, &["fabric,forge"]).is_err());
        assert!(new(ClientProjectType::Mod, Some("1.20\"]]"), &[]).is_err());
        assert_eq!(
            ClientProjectType::from_project_type("resourcepack"),
            Some(ClientProjectType::ResourcePack)
        );
        assert_eq!(
            ClientProjectType::from_project_type("modpack"),
            Some(ClientProjectType::Modpack)
        );
        assert_eq!(ClientProjectType::from_project_type("plugin"), None);
        let modpacks = new(ClientProjectType::Modpack, None, &["fabric"])
            .unwrap()
            .facets();
        assert_eq!(
            modpacks,
            vec![
                vec!["project_type:modpack".to_string()],
                vec!["categories:fabric".to_string()],
                vec![
                    "client_side:required".to_string(),
                    "client_side:optional".to_string()
                ],
            ]
        );
    }

    #[test]
    fn checks_client_values() {
        let new = |gv: Option<&str>, loader: Option<&str>, limit: Option<u32>| {
            SearchParams::new(
                None,
                ModrinthProjectType::Mod,
                gv.map(String::from),
                loader.map(String::from),
                Some(ModrinthSearchIndex::Downloads),
                None,
                limit,
            )
        };
        assert_eq!(new(None, None, Some(1000)).unwrap().limit, 100);
        assert_eq!(new(None, None, Some(0)).unwrap().limit, 1);
        assert_eq!(new(Some(""), Some(" "), None).unwrap().game_version, None);
        assert!(new(Some("1.20\"]]"), None, None).is_err());
        assert!(new(None, Some("fabric,forge"), None).is_err());
        assert!(new(None, Some("categories:x"), None).is_err());
        assert!(SearchParams::new(
            Some("a".repeat(257)),
            ModrinthProjectType::Mod,
            None,
            None,
            None,
            None,
            None
        )
        .is_err());
    }
}
