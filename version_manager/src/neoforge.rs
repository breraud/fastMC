use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct NeoForgeMavenVersions {
    versions: Vec<String>,
}

pub async fn fetch_neoforge_versions(game_version: &str) -> Result<Vec<String>, String> {
    let url =
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch NeoForge versions: {}", e))?;

    let data: NeoForgeMavenVersions = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse NeoForge versions: {}", e))?;

    // NeoForge versions use MC version without the leading "1." as prefix
    // e.g. MC 1.21.4 -> NeoForge prefix "21.4."
    let prefix = game_version
        .strip_prefix("1.")
        .unwrap_or(game_version);
    let prefix_dot = format!("{}.", prefix);

    let mut versions: Vec<String> = data
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix_dot))
        .collect();

    versions.reverse(); // newest first
    Ok(versions)
}

pub async fn download_neoforge_installer(
    neoforge_version: &str,
    dest: &Path,
) -> Result<(), String> {
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
        v = neoforge_version
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download NeoForge installer: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "NeoForge installer download failed: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read NeoForge installer bytes: {}", e))?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create installer dir: {}", e))?;
    }

    std::fs::write(dest, &bytes)
        .map_err(|e| format!("Failed to write NeoForge installer: {}", e))?;
    Ok(())
}
