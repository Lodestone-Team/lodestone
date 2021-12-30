#![allow(non_snake_case)]
pub mod response_from_mojang {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct ServerVersion {
        pub id: String,
        pub r#type: String,
        pub url: String,
        pub time: String,
        pub releaseTime: String,
    }
    #[derive(Deserialize, Serialize)]
    pub struct LatestVersion {
        pub release: String,
        pub snapshot: String,
    }

    #[derive(Deserialize, Serialize)]
    pub struct VersionManifest {
        pub versions: Vec<ServerVersion>,
        pub latest: LatestVersion,
    }
}
