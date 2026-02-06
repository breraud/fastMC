use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallSource {
    JavaHome,
    PathEntry,
    SystemLocation,
    UserProvided,
}

#[derive(Debug, Clone)]
pub struct JavaInstallation {
    pub id: Uuid,
    pub path: PathBuf,
    pub version: Option<String>,
    pub vendor: Option<String>,
    pub source: InstallSource,
}

#[derive(Debug, Clone)]
pub struct JavaDetectionConfig {
    pub auto_discover: bool,
    pub preferred_path: Option<PathBuf>,
}

impl Default for JavaDetectionConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            preferred_path: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum JavaError {
    #[error("java binary not found at {0}")]
    BinaryMissing(String),
    #[error("failed to inspect java at {path:?}: {error}")]
    Inspect { path: PathBuf, error: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default, Clone)]
pub struct DetectionSummary {
    pub installations: Vec<JavaInstallation>,
    pub errors: Vec<String>,
}

pub fn detect_installations(config: &JavaDetectionConfig) -> DetectionSummary {
    let mut summary = DetectionSummary::default();
    let candidates = candidate_binaries(config);
    let mut seen = HashSet::new();

    for (candidate, source) in candidates {
        let normalized = normalize_java_path(&candidate);
        let key = normalized.to_string_lossy().into_owned();
        if !seen.insert(key) {
            continue;
        }

        if !normalized.exists() {
            if matches!(source, InstallSource::UserProvided) {
                summary
                    .errors
                    .push(JavaError::BinaryMissing(normalized.display().to_string()).to_string());
            }
            continue;
        }

        match inspect_binary(&normalized, source) {
            Ok(installation) => summary.installations.push(installation),
            Err(err) => summary.errors.push(err.to_string()),
        }
    }

    summary
}

impl DetectionSummary {
    pub fn select_for_version(&self, target_version: &str) -> Result<PathBuf, String> {
        // 1. Precise Match Logic
        // Legacy (1.0 - 1.16.5) -> Java 8
        // 1.17 -> Java 16/17
        // 1.18+ -> Java 17+
        // 1.20.5+ -> Java 21+

        // Basic parsing of target version to determine requirement
        // Basic parsing of target version to determine requirement
        let is_version_1_x = target_version.starts_with("1.");
        let parts: Vec<&str> = if is_version_1_x {
            target_version.split('.').collect()
        } else {
            Vec::new()
        };
        
        let minor = if parts.len() >= 2 {
            parts[1].parse::<i32>().unwrap_or(0)
        } else {
            0
        };
        
        let patch = if parts.len() >= 3 {
            // Strip any non-numeric suffixes from patch (e.g., "1.2.3-pre") 
            parts[2].chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<i32>()
                .unwrap_or(0)
        } else {
            0
        };

        let requires_java21 = is_version_1_x 
            && (minor >= 21 || (minor == 20 && patch >= 5));

        let requires_java17 = is_version_1_x 
            && !requires_java21 
            && minor >= 17;

        let requires_java8 = is_version_1_x 
            && !requires_java21 
            && !requires_java17;

        // Helper to check if a version string matches requirement
        let matches_req = |v: &str| {
            if requires_java8 {
                v.starts_with("1.8") || v.starts_with("8")
            } else if requires_java21 {
                v.starts_with("21") || v.starts_with("22")
            } else if requires_java17 {
                v.starts_with("17") || v.starts_with("16")
            } else {
                // Fallback for unknown newer versions
                v.starts_with("21")
            }
        };

        // Priority 1: User Provided Path
        if let Some(user_install) = self
            .installations
            .iter()
            .find(|i| matches!(i.source, InstallSource::UserProvided))
        {
            // Validate compatibility if version metadata is available
            if let Some(v) = &user_install.version {
                if matches_req(v) {
                    return Ok(user_install.path.clone());
                }
                // If it DOESN'T match, we fail immediately with a descriptive error.
                // This handles the case where user selected Java 21 for 1.8.
                return Err(format!(
                    "Selected Java version ({}) is incompatible with Minecraft {}. Required: {}",
                    v,
                    target_version,
                    if requires_java8 {
                        "Java 8"
                    } else if requires_java17 {
                        "Java 16/17"
                    } else {
                        "Java 21+"
                    }
                ));
            }

            // If version inspection failed (None) but path is valid, we trust the user.
            // Warning: this risks a ClassCastException if they chose wrong, but it's better than blocking a valid unrecognized JVM.
            return Ok(user_install.path.clone());
        }

        // Priority 2: Best Auto-Detected Match
        let best_match = self.installations.iter().find(|i| {
            let v = i.version.as_deref().unwrap_or("");
            matches_req(v)
        });

        if let Some(install) = best_match {
            return Ok(install.path.clone());
        }

        // Fallbacks
        if requires_java8 {
            // Try to find ANY 8
            if let Some(install) = self.installations.iter().find(|i| {
                i.version
                    .as_deref()
                    .map(|v| v.starts_with("1.8") || v.starts_with("8"))
                    .unwrap_or(false)
            }) {
                return Ok(install.path.clone());
            }

            // If user has a forced path and we found NO other match, maybe just try the user path?
            if let Some(user_install) = self
                .installations
                .iter()
                .find(|i| matches!(i.source, InstallSource::UserProvided))
            {
                return Ok(user_install.path.clone());
            }

            // ERROR: Targeted legacy but no Java 8 found
            return Err("Java 8 is required for this version. Please install it or configure a Java path in settings.".to_string());
        }

        // Final Fallback: use whatever looks newest or just the first one
        Ok(self
            .installations
            .iter()
            .max_by_key(|i| {
                i.version
                    .as_ref()
                    .and_then(|v| v.split(|c: char| !c.is_numeric()).next())
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0)
            })
            .map(|i| i.path.clone())
            .unwrap_or_else(|| PathBuf::from("java")))
    }
}

fn candidate_binaries(config: &JavaDetectionConfig) -> Vec<(PathBuf, InstallSource)> {
    let mut candidates = Vec::new();

    if let Some(path) = &config.preferred_path {
        candidates.push((path.clone(), InstallSource::UserProvided));
    }

    if config.auto_discover {
        if let Some(java_home) = env::var_os("JAVA_HOME") {
            candidates.push((PathBuf::from(java_home), InstallSource::JavaHome));
        }

        if let Some(path_var) = env::var_os("PATH") {
            for entry in env::split_paths(&path_var) {
                candidates.push((entry, InstallSource::PathEntry));
            }
        }

        candidates.extend(platform_candidates());
    }

    candidates
        .into_iter()
        .map(|(path, source)| (ensure_binary(path), source))
        .collect()
}

fn ensure_binary(mut path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path = path.join("bin");
    }

    let name = if cfg!(windows) { "java.exe" } else { "java" };
    let direct = path.join(name);
    if direct.exists() {
        return direct;
    }

    // Some PATH entries already point directly to the binary.
    path
}

fn normalize_java_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn inspect_binary(path: &Path, source: InstallSource) -> Result<JavaInstallation, JavaError> {
    if !path.exists() {
        return Err(JavaError::BinaryMissing(path.display().to_string()));
    }

    let output = Command::new(path)
        .arg("-version")
        .output()
        .map_err(|error| JavaError::Inspect {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;

    let metadata = parse_java_metadata(&output.stderr, &output.stdout);
    let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, path.to_string_lossy().as_bytes());

    Ok(JavaInstallation {
        id,
        path: path.to_path_buf(),
        version: metadata.version,
        vendor: metadata.vendor,
        source,
    })
}

struct JavaMetadata {
    version: Option<String>,
    vendor: Option<String>,
}

fn parse_java_metadata(stderr: &[u8], stdout: &[u8]) -> JavaMetadata {
    let mut version = None;
    let mut vendor = None;

    for line in stderr
        .split(|b| *b == b'\n')
        .chain(stdout.split(|b| *b == b'\n'))
    {
        let line = String::from_utf8_lossy(line);
        let lower = line.to_lowercase();

        if version.is_none()
            && let Some(idx) = line.find("version \"")
        {
            let tail = &line[idx + 9..];
            if let Some(end) = tail.find('"') {
                version = Some(tail[..end].to_string());
            }
        }

        if vendor.is_none() {
            vendor = match () {
                _ if lower.contains("openjdk") => Some("OpenJDK".to_string()),
                _ if lower.contains("temurin") => Some("Temurin".to_string()),
                _ if lower.contains("corretto") => Some("Amazon Corretto".to_string()),
                _ if lower.contains("oracle") => Some("Oracle".to_string()),
                _ => None,
            };
        }

        if version.is_some() && vendor.is_some() {
            break;
        }
    }

    if version.is_none() {
        // Fallback: grab the first token that looks like a version (handles some older Java 8 outputs).
        for line in stderr
            .split(|b| *b == b'\n')
            .chain(stdout.split(|b| *b == b'\n'))
        {
            let line = String::from_utf8_lossy(line);
            for token in line.split_whitespace() {
                if let Some(v) = strip_version_like(token) {
                    version = Some(v);
                    break;
                }
            }
            if version.is_some() {
                break;
            }
        }
    }

    JavaMetadata { version, vendor }
}

fn strip_version_like(token: &str) -> Option<String> {
    let mut digits = false;
    let mut has_dot = false;
    let mut cleaned = String::new();

    for ch in token.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '_' {
            if ch == '.' {
                has_dot = true;
            }
            if ch.is_ascii_digit() {
                digits = true;
            }
            cleaned.push(ch);
        }
    }

    if digits && has_dot {
        Some(cleaned)
    } else {
        None
    }
}

fn platform_candidates() -> Vec<(PathBuf, InstallSource)> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let base = Path::new("/Library/Java/JavaVirtualMachines");
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                paths.push((
                    entry
                        .path()
                        .join("Contents")
                        .join("Home")
                        .join("bin")
                        .join("java"),
                    InstallSource::SystemLocation,
                ));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let candidates = [
            env::var_os("ProgramFiles"),
            env::var_os("ProgramFiles(x86)"),
            env::var_os("ProgramData"),
        ];

        for root in candidates.into_iter().flatten() {
            let java_root = PathBuf::from(root).join("Java");
            if let Ok(entries) = fs::read_dir(java_root) {
                for entry in entries.flatten() {
                    paths.push((
                        entry.path().join("bin").join("java.exe"),
                        InstallSource::SystemLocation,
                    ));
                }
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let search_roots = [
            "/usr/lib/jvm",
            "/usr/lib64/jvm",
            "/usr/lib/java",
            "/usr/local/lib/jvm",
            "/opt/java",
            "/usr/java",
        ];

        for root in search_roots {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    paths.push((
                        entry.path().join("bin").join("java"),
                        InstallSource::SystemLocation,
                    ));
                }
            }
        }

        paths.push((
            PathBuf::from("/usr/bin/java"),
            InstallSource::SystemLocation,
        ));
        paths.push((
            PathBuf::from("/usr/local/bin/java"),
            InstallSource::SystemLocation,
        ));
    }

    paths
}
