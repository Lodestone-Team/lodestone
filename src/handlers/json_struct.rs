pub mod response_from_mojang {
    use serde::{Serialize, Deserialize};
    
    #[derive(Deserialize, Serialize)]
    pub struct ServerVersion {
        pub id: String,
        pub r#type: String, // bruh
        pub url: String,
        pub time: String,
        pub releaseTime: String,
    }

    #[derive(Deserialize, Serialize)]
    pub struct VersionManifest {
        pub versions: Vec<ServerVersion>,
    }
}