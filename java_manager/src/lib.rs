pub mod detection;
pub mod settings;

pub use detection::{
    DetectionSummary, InstallSource, JavaDetectionConfig, JavaError, JavaInstallation,
    detect_installations,
};
pub use settings::JavaLaunchSettings;
