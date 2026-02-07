use crate::models::{FabricGameVersion, FabricLoaderVersion, LoaderLibrary, LoaderProfile};
use reqwest::Error;
use serde::Deserialize;

const FABRIC_LOADER_URL: &str = "https://meta.fabricmc.net/v2/versions/loader";
const FABRIC_GAME_URL: &str = "https://meta.fabricmc.net/v2/versions/game";

pub async fn fetch_fabric_loaders() -> Result<Vec<FabricLoaderVersion>, Error> {
    let client = reqwest::Client::new();
    let response = client.get(FABRIC_LOADER_URL).send().await?;
    let loaders: Vec<FabricLoaderVersion> = response.json().await?;
    Ok(loaders)
}

pub async fn fetch_fabric_game_versions() -> Result<Vec<FabricGameVersion>, Error> {
    let client = reqwest::Client::new();
    let response = client.get(FABRIC_GAME_URL).send().await?;
    let versions: Vec<FabricGameVersion> = response.json().await?;
    Ok(versions)
}

pub async fn fetch_compatible_loaders(
    game_version: &str,
) -> Result<Vec<FabricLoaderVersion>, Error> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}",
        game_version
    );
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let loaders: Vec<FabricLoaderVersion> = response.json().await?;
    Ok(loaders)
}

#[derive(Debug, Deserialize)]
struct FabricProfileJson {
    #[serde(rename = "mainClass")]
    main_class: String,
    libraries: Vec<FabricProfileLib>,
    arguments: Option<FabricProfileArguments>,
}

#[derive(Debug, Deserialize)]
struct FabricProfileLib {
    name: String,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FabricProfileArguments {
    jvm: Option<Vec<String>>,
    game: Option<Vec<String>>,
}

pub async fn fetch_fabric_profile(
    game_version: &str,
    loader_version: &str,
) -> Result<LoaderProfile, String> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        game_version, loader_version
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Fabric profile: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Fabric profile API returned {}",
            response.status()
        ));
    }

    let profile: FabricProfileJson = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Fabric profile: {}", e))?;

    Ok(LoaderProfile {
        main_class: profile.main_class,
        libraries: profile
            .libraries
            .into_iter()
            .map(|lib| LoaderLibrary {
                name: lib.name,
                url: lib.url.or_else(|| Some("https://maven.fabricmc.net/".to_string())),
            })
            .collect(),
        jvm_args: profile
            .arguments
            .as_ref()
            .and_then(|a| a.jvm.clone())
            .unwrap_or_default(),
        game_args: profile
            .arguments
            .as_ref()
            .and_then(|a| a.game.clone())
            .unwrap_or_default(),
    })
}
