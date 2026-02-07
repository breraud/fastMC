use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModLoader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl fmt::Display for ModLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModLoader::Vanilla => write!(f, "Vanilla"),
            ModLoader::Fabric => write!(f, "Fabric"),
            ModLoader::Forge => write!(f, "Forge"),
            ModLoader::NeoForge => write!(f, "NeoForge"),
            ModLoader::Quilt => write!(f, "Quilt"),
        }
    }
}

pub const ALL_LOADERS: [ModLoader; 5] = [
    ModLoader::Vanilla,
    ModLoader::Fabric,
    ModLoader::Quilt,
    ModLoader::Forge,
    ModLoader::NeoForge,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetadata {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created: u64,
    pub last_played: u64,
    pub total_time: u64,

    // Components
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,

    // Java Overrides (None = inherit from global)
    pub java_path: Option<String>,
    #[serde(default)]
    pub min_memory_mb: Option<u32>,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    pub jvm_args: Option<Vec<String>>,
    #[serde(default)]
    pub auto_discover: Option<bool>,

    #[serde(default)]
    pub loader_installed: bool,

    // Legacy field: read but never written back
    #[serde(default, skip_serializing)]
    memory_mb: Option<u32>,
}

impl InstanceMetadata {
    /// Migrate legacy fields to new format.
    pub fn migrate(&mut self) {
        if let Some(mem) = self.memory_mb.take() {
            if self.min_memory_mb.is_none() {
                self.min_memory_mb = Some(mem);
            }
            if self.max_memory_mb.is_none() {
                self.max_memory_mb = Some(mem);
            }
        }
    }
}

impl Default for InstanceMetadata {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New Instance".to_string(),
            icon: None,
            created: current_timestamp(),
            last_played: 0,
            total_time: 0,
            game_version: "1.21".to_string(),
            loader: ModLoader::Vanilla,
            loader_version: None,
            java_path: None,
            min_memory_mb: None,
            max_memory_mb: None,
            jvm_args: None,
            auto_discover: None,
            loader_installed: false,
            memory_mb: None,
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone)]
pub struct InstanceManager {
    base_dir: PathBuf,
}

impl InstanceManager {
    pub fn new() -> Self {
        let dirs = directories::ProjectDirs::from("com", "fastmc", "fastmc").unwrap();
        let base_dir = dirs.data_local_dir().join("instances");
        Self { base_dir }
    }

    pub fn init(&self) -> std::io::Result<()> {
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir)?;
        }
        Ok(())
    }

    pub fn list_instances(&self) -> Vec<InstanceMetadata> {
        let mut instances = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let json_path = entry.path().join("instance.json");
                    if json_path.exists() {
                        if let Ok(content) = fs::read_to_string(&json_path) {
                            if let Ok(mut meta) =
                                serde_json::from_str::<InstanceMetadata>(&content)
                            {
                                meta.migrate();
                                instances.push(meta);
                            }
                        }
                    }
                }
            }
        }

        // Sort by last played (descending), then created
        instances.sort_by(|a, b| {
            b.last_played
                .cmp(&a.last_played)
                .then(b.created.cmp(&a.created))
        });

        instances
    }

    pub fn create_instance(
        &self,
        name: String,
        version: String,
    ) -> std::io::Result<InstanceMetadata> {
        self.init()?;

        // Generate safe directory name
        let id = Uuid::new_v4().to_string();
        let instance_dir = self.base_dir.join(&id);
        fs::create_dir_all(&instance_dir)?;
        fs::create_dir_all(instance_dir.join(".minecraft"))?;

        let metadata = InstanceMetadata {
            id: id.clone(),
            name,
            game_version: version,
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(instance_dir.join("instance.json"), json)?;

        Ok(metadata)
    }

    pub fn delete_instance(&self, id: &str) -> std::io::Result<()> {
        let instance_dir = self.base_dir.join(id);
        if instance_dir.exists() {
            fs::remove_dir_all(instance_dir)?;
        }
        Ok(())
    }

    pub fn save_instance(&self, metadata: &InstanceMetadata) -> io::Result<()> {
        let instance_dir = self.base_dir.join(&metadata.id);
        let json = serde_json::to_string_pretty(metadata)?;
        fs::write(instance_dir.join("instance.json"), json)?;
        Ok(())
    }

    pub fn load_instance(&self, id: &str) -> io::Result<InstanceMetadata> {
        let json_path = self.base_dir.join(id).join("instance.json");
        let content = fs::read_to_string(&json_path)?;
        let mut meta: InstanceMetadata = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        meta.migrate();
        Ok(meta)
    }
}
