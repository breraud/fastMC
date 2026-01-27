use crate::models::{VersionManifestV2, VanillaVersion};
use reqwest::Error;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

pub async fn fetch_vanilla_versions() -> Result<Vec<VanillaVersion>, Error> {
    let client = reqwest::Client::new();
    let response = client.get(MANIFEST_URL).send().await?;
    let manifest: VersionManifestV2 = response.json().await?;
    Ok(manifest.versions)
}

pub async fn fetch_manifest() -> Result<VersionManifestV2, Error> {
    let client = reqwest::Client::new();
    let response = client.get(MANIFEST_URL).send().await?;
    let manifest: VersionManifestV2 = response.json().await?;
    Ok(manifest)
}
