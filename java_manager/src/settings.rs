use std::path::PathBuf;

use fastmc_config::{JavaConfig, JavaInstallationRecord};

use crate::detection::JavaDetectionConfig;

#[derive(Debug, Clone)]
pub struct JavaLaunchSettings {
    pub java_path: Option<PathBuf>,
    pub auto_discover: bool,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub detected_installations: Vec<JavaInstallationRecord>,
}

impl JavaLaunchSettings {
    pub fn memory_bounds(&self, total_memory_mb: Option<u64>) -> (u32, u32) {
        let mut max = self.max_memory_mb.max(self.min_memory_mb);
        if let Some(total) = total_memory_mb {
            let available = total.min(u64::from(u32::MAX)) as u32;
            max = max.min(available);
        }

        let min = self.min_memory_mb.min(max);
        (min, max)
    }

    pub fn jvm_args(&self, total_memory_mb: Option<u64>) -> Vec<String> {
        let (min, max) = self.memory_bounds(total_memory_mb);
        let mut args = vec![format!("-Xms{}M", min), format!("-Xmx{}M", max)];
        args.extend(self.extra_jvm_args.clone());
        args
    }

    pub fn detection_config(&self) -> JavaDetectionConfig {
        JavaDetectionConfig {
            auto_discover: self.auto_discover,
            preferred_path: self.java_path.clone(),
        }
    }

    pub fn to_config(&self) -> JavaConfig {
        JavaConfig {
            java_path: self
                .java_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            auto_discover: self.auto_discover,
            min_memory_mb: self.min_memory_mb,
            max_memory_mb: self.max_memory_mb,
            extra_jvm_args: self.extra_jvm_args.clone(),
            detected_installations: self.detected_installations.clone(),
        }
    }
}

impl From<&JavaConfig> for JavaLaunchSettings {
    fn from(config: &JavaConfig) -> Self {
        JavaLaunchSettings {
            java_path: config.java_path.as_ref().map(PathBuf::from),
            auto_discover: config.auto_discover,
            min_memory_mb: config.min_memory_mb,
            max_memory_mb: config.max_memory_mb,
            extra_jvm_args: config.extra_jvm_args.clone(),
            detected_installations: config.detected_installations.clone(),
        }
    }
}
