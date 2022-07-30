use std::{path::Path, str::FromStr};

use rocket::serde::json::serde_json;

use crate::{
    traits::{t_resource::DownloadReport, Error, ErrorInner},
    util::download_resource,
};

use super::{Instance, Flavour};

async fn get_vanilla_jar_url(version: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let response_text = client
        .get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let response: serde_json::Value = serde_json::from_str(&response_text).ok()?;

    let url = response
        .get("versions")?
        .as_array()?
        .iter()
        .find(|version_json| {
            version_json
                .get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .eq(version)
        })?
        .get("url")?
        .as_str()?;
    let response: serde_json::Value =
        serde_json::from_str(&client.get(url).send().await.ok()?.text().await.ok()?).ok()?;
    if response["downloads"]["server"]["url"] == serde_json::Value::Null {
        return None;
    }

    return Some(
        response["downloads"]["server"]["url"]
            .to_string()
            .replace("\"", ""),
    );
}

async fn get_fabric_jar_url(
    version: &str,
    fabric_loader_version: Option<&str>,
    fabric_installer_version: Option<&str>,
) -> Option<String> {
    let mut loader_version = String::new();
    let mut installer_version = String::new();
    let client = reqwest::Client::new();

    if let (Some(l), Some(i)) = (fabric_loader_version, fabric_installer_version) {
        loader_version = l.to_string();
        installer_version = i.to_string();
        return Some(format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
            version, loader_version, installer_version
        ));
    }

    if fabric_loader_version.is_none() {
        loader_version = serde_json::Value::from_str(
            client
                .get(format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}",
                    version
                ))
                .send()
                .await
                .ok()?
                .text()
                .await
                .ok()?
                .as_str(),
        )
        .ok()?
        .as_array()?
        .iter()
        .filter(|v| {
            v.get("loader")
                .unwrap()
                .get("stable")
                .unwrap()
                .as_bool()
                .unwrap()
                && v.get("intermediary")
                    .unwrap()
                    .get("stable")
                    .unwrap()
                    .as_bool()
                    .unwrap()
        })
        .max_by(|a, b| {
            let a_version = a
                .get("loader")
                .unwrap()
                .get("version")
                .unwrap()
                .as_str()
                .unwrap()
                .split('.')
                .collect::<Vec<&str>>();
            let b_version = b
                .get("loader")
                .unwrap()
                .get("version")
                .unwrap()
                .as_str()
                .unwrap()
                .split('.')
                .collect::<Vec<&str>>();
            for (a_part, b_part) in a_version.iter().zip(b_version.iter()) {
                if a_part.parse::<i32>().unwrap() > b_part.parse::<i32>().unwrap() {
                    return std::cmp::Ordering::Greater;
                } else if a_part.parse::<i32>().unwrap() < b_part.parse::<i32>().unwrap() {
                    return std::cmp::Ordering::Less;
                }
            }
            return std::cmp::Ordering::Equal;
        })?
        .get("loader")?
        .get("version")?
        .as_str()?
        .to_string();
    }

    if fabric_installer_version.is_none() {
        installer_version = serde_json::Value::from_str(
            client
                .get("https://meta.fabricmc.net/v2/versions/installer")
                .send()
                .await
                .ok()?
                .text()
                .await
                .ok()?
                .as_str(),
        )
        .ok()?
        .as_array()?
        .iter()
        .filter(|v| v.get("stable").unwrap().as_bool().unwrap())
        .max_by(|a, b| {
            // sort the version string in the form of "1.2.3"
            let a_version = a
                .get("loader")
                .unwrap()
                .get("version")
                .unwrap()
                .as_str()
                .unwrap()
                .split('.')
                .collect::<Vec<&str>>();
            let b_version = b
                .get("loader")
                .unwrap()
                .get("version")
                .unwrap()
                .as_str()
                .unwrap()
                .split('.')
                .collect::<Vec<&str>>();
            for (a_part, b_part) in a_version.iter().zip(b_version.iter()) {
                if a_part.parse::<i32>().unwrap() > b_part.parse::<i32>().unwrap() {
                    return std::cmp::Ordering::Greater;
                } else if a_part.parse::<i32>().unwrap() < b_part.parse::<i32>().unwrap() {
                    return std::cmp::Ordering::Less;
                }
            }
            return std::cmp::Ordering::Equal;
        })?
        .get("version")?
        .as_str()?
        .to_string();
    }
    return Some(format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
        version, loader_version, installer_version
    ));
}

async fn get_jre_url(version: &str) -> Option<(String, u64)> {
    let client = reqwest::Client::new();
    let os = std::env::consts::OS;
    let arch = if std::env::consts::ARCH == "x86_64" {
        "x64"
    } else {
        std::env::consts::ARCH
    };

    let major_java_version = serde_json::Value::from_str(
        client
            .get(
                serde_json::Value::from_str(
                    client
                        .get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
                        .send()
                        .await
                        .ok()?
                        .text()
                        .await
                        .ok()?
                        .as_str(),
                )
                .ok()?
                .get("versions")?
                .as_array()?
                .iter()
                .find(|v| v.get("id").unwrap().as_str().unwrap().eq(version))?
                .get("url")?
                .as_str()?,
            )
            .send()
            .await
            .ok()?
            .text()
            .await
            .ok()?
            .as_str(),
    )
    .ok()?
    .get("javaVersion")?
    .get("majorVersion")?
    .as_u64()?;

    return Some((
        format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/eclipse",
            major_java_version, os, arch
        ),
        major_java_version,
    ));
}

async fn get_list_of_versions(flavour : Flavour) -> Vec<String> {
    match flavour {
        Flavour::Vanilla => todo!(),
        Flavour::Fabric => todo!(),
        Flavour::Paper => todo!(),
        Flavour::Spigot => unimplemented!(),
    }
}

async fn download_dependencies(
    version: &str,
    flavour: Flavour,
    fabric_loader_version: Option<&str>,
    fabric_installer_version: Option<&str>,
    path_to_instance: &Path,
    path_to_runtimes: &Path, // TODO: add paper support
) -> Result<Vec<DownloadReport>, Error> {
    if let Some((url, jre_version)) = get_jre_url(version).await {
        let path_to_jre = path_to_runtimes.join(format!("jre_{}", jre_version));
        if !path_to_jre.exists() {
            std::fs::create_dir_all(&path_to_jre).unwrap();
            download_resource("jre", &path_to_jre, Some(url.as_str())).await?;
        }
        // unzip jre
        if std::env::consts::OS == "windows" {
            // handle the file as a .zip file
            todo!()
        }
        if std::env::consts::OS == "linux" {
            // handle the file as a .tar.gz file
            todo!()
        }
    }

    match flavour {
        Flavour::Vanilla => {
            download_resource(
                get_vanilla_jar_url(version)
                    .await
                    .ok_or(Error {
                        inner: ErrorInner::VersionNotFound,
                        detail: "".to_string(),
                    })?
                    .as_str(),
                path_to_instance,
                None,
            )
            .await?
        }
        Flavour::Fabric => {
            download_resource(
                get_fabric_jar_url(version, fabric_loader_version, fabric_installer_version)
                    .await
                    .ok_or(Error {
                        inner: ErrorInner::VersionNotFound,
                        detail: "".to_string(),
                    })?
                    .as_str(),
                path_to_instance,
                None,
            )
            .await?
        }
        Flavour::Paper => todo!(),
        Flavour::Spigot => unimplemented!(),
    };
    todo!()
}
mod tests {
    use rocket::tokio;

    #[tokio::test]
    async fn test_get_vanilla_jar_url() {
        assert_eq!(super::get_vanilla_jar_url("1.18.2").await, Some("https://launcher.mojang.com/v1/objects/c8f83c5655308435b3dcf03c06d9fe8740a77469/server.jar".to_string()));
        assert_eq!(super::get_vanilla_jar_url("21w44a").await, Some("https://launcher.mojang.com/v1/objects/ae583fd57a8c07f2d6fbadce1ce1e1379bf4b32d/server.jar".to_string()));
        assert_eq!(super::get_vanilla_jar_url("1.8.4").await, Some("https://launcher.mojang.com/v1/objects/dd4b5eba1c79500390e0b0f45162fa70d38f8a3d/server.jar".to_string()));

        assert_eq!(
            super::get_vanilla_jar_url("1.8.4asdasd").await,
            None
        );
    }
    #[tokio::test]
    async fn test_get_jre_url() {
        assert_eq!(super::get_jre_url("1.18.2").await, Some(("https://api.adoptium.net/v3/binary/latest/17/ga/linux/x64/jre/hotspot/normal/eclipse".to_string(), 17)));
        assert_eq!(super::get_jre_url("21w44a").await, Some(("https://api.adoptium.net/v3/binary/latest/16/ga/linux/x64/jre/hotspot/normal/eclipse".to_string(), 16)));
        assert_eq!(super::get_jre_url("1.8.4").await, Some(("https://api.adoptium.net/v3/binary/latest/8/ga/linux/x64/jre/hotspot/normal/eclipse".to_string(), 8)));

        assert_eq!(super::get_jre_url("1.8.4asdasd").await, None);
    }

    /// Test subject to fail if fabric updates their installer or loader
    #[tokio::test]
    async fn test_get_fabric_url() {
        assert_eq!(
            super::get_fabric_jar_url("1.19", Some("0.14.8"), Some("0.11.0")).await,
            Some(
                "https://meta.fabricmc.net/v2/versions/loader/1.19/0.14.8/0.11.0/server/jar"
                    .to_string()
            )
        );
        assert_eq!(
            super::get_fabric_jar_url("21w44a", None, None).await,
            Some(
                "https://meta.fabricmc.net/v2/versions/loader/21w44a/0.14.8/0.11.0/server/jar"
                    .to_string()
            )
        );
    }
}