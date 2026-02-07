use crate::models::{LoaderLibrary, LoaderProfile, QuiltLoaderVersion};
use serde::Deserialize;

const QUILT_META_BASE: &str = "https://meta.quiltmc.org/v3";

pub async fn fetch_quilt_loaders() -> Result<Vec<QuiltLoaderVersion>, String> {
    let url = format!("{}/versions/loader", QUILT_META_BASE);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Quilt loaders: {}", e))?;
    let loaders: Vec<QuiltLoaderVersion> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Quilt loaders: {}", e))?;
    Ok(loaders)
}

#[derive(Debug, Deserialize)]
struct QuiltProfileJson {
    #[serde(rename = "mainClass")]
    main_class: String,
    libraries: Vec<QuiltProfileLib>,
    arguments: Option<QuiltProfileArguments>,
}

#[derive(Debug, Deserialize)]
struct QuiltProfileLib {
    name: String,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QuiltProfileArguments {
    jvm: Option<Vec<String>>,
    game: Option<Vec<String>>,
}

pub async fn fetch_quilt_profile(
    game_version: &str,
    loader_version: &str,
) -> Result<LoaderProfile, String> {
    let url = format!(
        "{}/versions/loader/{}/{}/profile/json",
        QUILT_META_BASE, game_version, loader_version
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Quilt profile: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Quilt profile API returned {}", response.status()));
    }

    let profile: QuiltProfileJson = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Quilt profile: {}", e))?;

    Ok(LoaderProfile {
        main_class: profile.main_class,
        libraries: profile
            .libraries
            .into_iter()
            .map(|lib| LoaderLibrary {
                name: lib.name,
                url: lib
                    .url
                    .or_else(|| Some("https://maven.quiltmc.org/repository/release/".to_string())),
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
