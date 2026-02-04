use crate::models::{FabricGameVersion, FabricLoaderVersion};
use reqwest::Error;

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
