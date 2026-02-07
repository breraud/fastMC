use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VersionType {
    #[serde(rename = "release")]
    Release,
    #[serde(rename = "snapshot")]
    Snapshot,
    #[serde(rename = "old_beta")]
    OldBeta,
    #[serde(rename = "old_alpha")]
    OldAlpha,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanillaVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: VersionType, // "release", "snapshot", etc. - keeping as string for broader compatibility or strictly enum if we custom deserialize
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifestV2 {
    pub latest: LatestVersions,
    pub versions: Vec<VanillaVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLoaderVersion {
    pub separator: String,
    pub build: i32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

// Minimal struct for Fabric Game Versions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricGameVersion {
    pub version: String,
    pub stable: bool,
}

// === Loader Profile (unified, used at launch time by all loaders) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderProfile {
    pub main_class: String,
    pub libraries: Vec<LoaderLibrary>,
    #[serde(default)]
    pub jvm_args: Vec<String>,
    #[serde(default)]
    pub game_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderLibrary {
    pub name: String,
    pub url: Option<String>,
}

// === Quilt ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuiltLoaderVersion {
    pub version: String,
}

// === Forge / NeoForge types ===

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeInstallProfile {
    pub processors: Vec<ForgeProcessor>,
    pub libraries: Vec<ForgeLibEntry>,
    pub data: HashMap<String, ForgeDataEntry>,
    pub minecraft: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeProcessor {
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
    #[serde(default)]
    pub sides: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeDataEntry {
    pub client: String,
    pub server: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeLibEntry {
    pub name: String,
    pub downloads: Option<ForgeLibDownloads>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeLibDownloads {
    pub artifact: Option<ForgeArtifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeArtifact {
    pub url: String,
    pub path: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeVersionJson {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub libraries: Vec<ForgeLibEntry>,
    pub arguments: Option<ForgeArguments>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeArguments {
    pub game: Option<Vec<serde_json::Value>>,
    pub jvm: Option<Vec<serde_json::Value>>,
}
