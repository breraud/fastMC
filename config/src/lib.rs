use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;
use thiserror::Error;

const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config directory unavailable")]
    ConfigDirMissing,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("persist error: {0}")]
    Persist(#[from] tempfile::PersistError),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfilesConfig {
    /// Name of the default profile/instance to select on launch.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Optional override for where instances/profiles are stored.
    #[serde(default)]
    pub instances_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaConfig {
    /// If set, use this Java binary; otherwise attempt discovery.
    #[serde(default)]
    pub java_path: Option<String>,
    /// Whether to auto-discover Java installations.
    #[serde(default = "default_true")]
    pub auto_discover: bool,
    /// Requested minimum RAM (in megabytes) for the JVM.
    #[serde(default = "default_min_memory_mb")]
    pub min_memory_mb: u32,
    /// Requested maximum RAM (in megabytes) for the JVM.
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u32,
    /// Extra JVM arguments to append during launch.
    #[serde(default)]
    pub extra_jvm_args: Vec<String>,
}

impl Default for JavaConfig {
    fn default() -> Self {
        Self {
            java_path: None,
            auto_discover: true,
            min_memory_mb: default_min_memory_mb(),
            max_memory_mb: default_max_memory_mb(),
            extra_jvm_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountsConfig {
    /// Optional Microsoft client ID for device-code auth.
    #[serde(default)]
    pub microsoft_client_id: Option<String>,
    /// Permit offline account creation.
    #[serde(default = "default_true")]
    pub allow_offline: bool,
    /// Optional custom storage path for accounts; defaults to app data dir.
    #[serde(default)]
    pub store_path: Option<String>,
}

impl Default for AccountsConfig {
    fn default() -> Self {
        Self {
            microsoft_client_id: None,
            allow_offline: true,
            store_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastmcConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub profiles: ProfilesConfig,
    #[serde(default)]
    pub java: JavaConfig,
    #[serde(default)]
    pub accounts: AccountsConfig,
}

impl Default for FastmcConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            profiles: ProfilesConfig::default(),
            java: JavaConfig::default(),
            accounts: AccountsConfig::default(),
        }
    }
}

impl FastmcConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let mut config: FastmcConfig = serde_json::from_str(&content)?;
        migrate(&mut config);
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = config_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut tmp = NamedTempFile::new_in(path.parent().ok_or(ConfigError::ConfigDirMissing)?)?;
        tmp.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;
        tmp.persist(path)?;
        Ok(())
    }
}

fn config_file() -> Result<PathBuf, ConfigError> {
    let dirs = ProjectDirs::from("com", "fastmc", "fastmc").ok_or(ConfigError::ConfigDirMissing)?;
    Ok(dirs.config_dir().join("config.json"))
}

fn migrate(config: &mut FastmcConfig) {
    if config.version < CONFIG_VERSION {
        config.version = CONFIG_VERSION;
    }
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

fn default_min_memory_mb() -> u32 {
    1024
}

fn default_max_memory_mb() -> u32 {
    4096
}

fn default_true() -> bool {
    true
}
