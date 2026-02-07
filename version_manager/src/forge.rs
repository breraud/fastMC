use crate::models::{ForgeInstallProfile, ForgeVersionJson};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct ForgePromotions {
    promos: std::collections::HashMap<String, String>,
}

pub async fn fetch_forge_versions(game_version: &str) -> Result<Vec<String>, String> {
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Forge promotions: {}", e))?;
    let promos: ForgePromotions = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Forge promotions: {}", e))?;

    let mut versions = Vec::new();
    let prefix = format!("{}-", game_version);
    for key in promos.promos.keys() {
        if key.starts_with(&prefix) {
            if let Some(suffix) = key.strip_prefix(&prefix) {
                let label = suffix.to_string(); // e.g. "recommended", "latest"
                if let Some(forge_ver) = promos.promos.get(key) {
                    versions.push(format!("{} ({})", forge_ver, label));
                }
            }
        }
    }

    // Also try to fetch from Maven metadata for full list
    let maven_url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml"
    );
    if let Ok(resp) = client.get(&maven_url).send().await {
        if let Ok(text) = resp.text().await {
            // Simple XML parsing — extract versions matching game_version
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("<version>") && trimmed.ends_with("</version>") {
                    let ver = trimmed
                        .trim_start_matches("<version>")
                        .trim_end_matches("</version>");
                    if ver.starts_with(&prefix) {
                        let forge_part = ver.strip_prefix(&prefix).unwrap_or(ver);
                        if !versions.iter().any(|v: &String| v.starts_with(forge_part)) {
                            versions.push(forge_part.to_string());
                        }
                    }
                }
            }
        }
    }

    if versions.is_empty() {
        return Err(format!("No Forge versions found for {}", game_version));
    }

    versions.sort();
    versions.reverse();
    Ok(versions)
}

pub async fn download_forge_installer(
    game_version: &str,
    forge_version: &str,
    dest: &Path,
) -> Result<(), String> {
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{game}-{forge}/forge-{game}-{forge}-installer.jar",
        game = game_version,
        forge = forge_version
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download Forge installer: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Forge installer download failed: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read Forge installer bytes: {}", e))?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create installer dir: {}", e))?;
    }

    std::fs::write(dest, &bytes).map_err(|e| format!("Failed to write installer: {}", e))?;
    Ok(())
}

pub fn extract_forge_installer(
    installer_jar: &Path,
    libraries_dir: &Path,
) -> Result<(ForgeInstallProfile, ForgeVersionJson), String> {
    let file =
        std::fs::File::open(installer_jar).map_err(|e| format!("Cannot open installer: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid installer JAR: {}", e))?;

    // Extract install_profile.json
    let install_profile: ForgeInstallProfile = {
        let mut entry = archive
            .by_name("install_profile.json")
            .map_err(|e| format!("Missing install_profile.json: {}", e))?;
        let mut buf = String::new();
        entry
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read install_profile.json: {}", e))?;
        serde_json::from_str(&buf)
            .map_err(|e| format!("Failed to parse install_profile.json: {}", e))?
    };

    // Extract version.json
    let version_json: ForgeVersionJson = {
        let mut entry = archive
            .by_name("version.json")
            .map_err(|e| format!("Missing version.json: {}", e))?;
        let mut buf = String::new();
        entry
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read version.json: {}", e))?;
        serde_json::from_str(&buf)
            .map_err(|e| format!("Failed to parse version.json: {}", e))?
    };

    // Extract maven/ directory contents to libraries
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = entry.name().to_string();
        if name.starts_with("maven/") && !entry.is_dir() {
            let rel = name.strip_prefix("maven/").unwrap_or(&name);
            let dest = libraries_dir.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create lib dir: {}", e))?;
            }
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            std::fs::write(&dest, &buf)
                .map_err(|e| format!("Failed to write {}: {}", dest.display(), e))?;
        }
    }

    // Also extract data entries that reference paths inside the JAR (start with /)
    // These get extracted to a temp location relative to libraries_dir
    for (_key, entry) in &install_profile.data {
        let client_val = &entry.client;
        if client_val.starts_with('/') {
            let jar_path = client_val.trim_start_matches('/');
            if let Ok(mut zip_entry) = archive.by_name(jar_path) {
                let dest = libraries_dir.join("forge_extracted").join(jar_path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut buf = Vec::new();
                zip_entry.read_to_end(&mut buf).ok();
                std::fs::write(&dest, &buf).ok();
            }
        }
    }

    Ok((install_profile, version_json))
}

pub fn extract_jar_main_class(jar_path: &Path) -> Result<String, String> {
    let file =
        std::fs::File::open(jar_path).map_err(|e| format!("Cannot open JAR {}: {}", jar_path.display(), e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid JAR {}: {}", jar_path.display(), e))?;

    let mut entry = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|e| format!("No MANIFEST.MF in {}: {}", jar_path.display(), e))?;

    let mut manifest = String::new();
    entry
        .read_to_string(&mut manifest)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;

    for line in manifest.lines() {
        if let Some(rest) = line.strip_prefix("Main-Class:") {
            return Ok(rest.trim().to_string());
        }
    }

    Err(format!("No Main-Class in {}", jar_path.display()))
}
