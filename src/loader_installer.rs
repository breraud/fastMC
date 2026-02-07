use crate::game::{download_file, maven_to_path};
use crate::instance_manager::ModLoader;
use std::path::Path;
use version_manager::models::LoaderProfile;

pub async fn install_loader(
    instance_dir: &Path,
    game_version: &str,
    loader: ModLoader,
    loader_version: &str,
    java_path: Option<&Path>,
) -> Result<(), String> {
    match loader {
        ModLoader::Vanilla => Err("Cannot install Vanilla as a loader".to_string()),
        ModLoader::Fabric => install_fabric(instance_dir, game_version, loader_version).await,
        ModLoader::Quilt => install_quilt(instance_dir, game_version, loader_version).await,
        ModLoader::Forge => {
            install_forge(instance_dir, game_version, loader_version, java_path).await
        }
        ModLoader::NeoForge => {
            install_neoforge(instance_dir, game_version, loader_version, java_path).await
        }
    }
}

async fn download_loader_libraries(
    libraries_dir: &Path,
    profile: &LoaderProfile,
) -> Result<(), String> {
    for lib in &profile.libraries {
        let rel_path = maven_to_path(&lib.name);
        let lib_path = libraries_dir.join(&rel_path);

        if lib_path.exists() {
            continue;
        }

        let base_url = lib
            .url
            .as_deref()
            .unwrap_or("https://libraries.minecraft.net/");

        let url = format!("{}{}", base_url, rel_path.display());

        if let Some(parent) = lib_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create lib dir: {}", e))?;
        }

        download_file(&url, &lib_path).await?;
    }
    Ok(())
}

async fn save_loader_profile(instance_dir: &Path, profile: &LoaderProfile) -> Result<(), String> {
    let path = instance_dir.join("loader_profile.json");
    let json =
        serde_json::to_string_pretty(profile).map_err(|e| format!("Failed to serialize: {}", e))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Failed to write loader_profile.json: {}", e))?;
    Ok(())
}

// === Fabric ===

async fn install_fabric(
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
) -> Result<(), String> {
    println!(
        "Installing Fabric {} for MC {}",
        loader_version, game_version
    );

    let profile =
        version_manager::fabric::fetch_fabric_profile(game_version, loader_version).await?;

    let libraries_dir = instance_dir.join(".minecraft").join("libraries");
    tokio::fs::create_dir_all(&libraries_dir)
        .await
        .map_err(|e| e.to_string())?;

    download_loader_libraries(&libraries_dir, &profile).await?;
    save_loader_profile(instance_dir, &profile).await?;

    println!("Fabric installation complete");
    Ok(())
}

// === Quilt ===

async fn install_quilt(
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
) -> Result<(), String> {
    println!(
        "Installing Quilt {} for MC {}",
        loader_version, game_version
    );

    let profile =
        version_manager::quilt::fetch_quilt_profile(game_version, loader_version).await?;

    let libraries_dir = instance_dir.join(".minecraft").join("libraries");
    tokio::fs::create_dir_all(&libraries_dir)
        .await
        .map_err(|e| e.to_string())?;

    download_loader_libraries(&libraries_dir, &profile).await?;
    save_loader_profile(instance_dir, &profile).await?;

    println!("Quilt installation complete");
    Ok(())
}

// === Forge ===

async fn install_forge(
    instance_dir: &Path,
    game_version: &str,
    forge_version: &str,
    java_path: Option<&Path>,
) -> Result<(), String> {
    println!(
        "Installing Forge {} for MC {}",
        forge_version, game_version
    );

    let java = java_path.ok_or("Java path required for Forge installation")?;
    let libraries_dir = instance_dir.join(".minecraft").join("libraries");
    let installer_path = instance_dir.join("forge-installer.jar");

    // 1. Download installer
    version_manager::forge::download_forge_installer(game_version, forge_version, &installer_path)
        .await?;

    // 2. Extract install_profile.json, version.json, and maven/ libs
    let libraries_dir_clone = libraries_dir.clone();
    let installer_path_clone = installer_path.clone();
    let (install_profile, version_json) = tokio::task::spawn_blocking(move || {
        version_manager::forge::extract_forge_installer(&installer_path_clone, &libraries_dir_clone)
    })
    .await
    .map_err(|e| e.to_string())??;

    // 3. Download all libraries from install_profile + version_json
    tokio::fs::create_dir_all(&libraries_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Download install_profile libraries
    for lib in &install_profile.libraries {
        let lib_path = libraries_dir.join(maven_to_path(&lib.name));
        if lib_path.exists() {
            continue;
        }
        if let Some(parent) = lib_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }

        if let Some(ref downloads) = lib.downloads {
            if let Some(ref artifact) = downloads.artifact {
                if !artifact.url.is_empty() {
                    download_file(&artifact.url, &lib_path).await?;
                    continue;
                }
            }
        }
        // Fallback: try Forge maven
        let url = format!(
            "https://maven.minecraftforge.net/{}",
            maven_to_path(&lib.name).display()
        );
        let _ = download_file(&url, &lib_path).await;
    }

    // Download version_json libraries
    for lib in &version_json.libraries {
        let lib_path = libraries_dir.join(maven_to_path(&lib.name));
        if lib_path.exists() {
            continue;
        }
        if let Some(parent) = lib_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref downloads) = lib.downloads {
            if let Some(ref artifact) = downloads.artifact {
                if !artifact.url.is_empty() {
                    download_file(&artifact.url, &lib_path).await?;
                    continue;
                }
            }
        }
        let url = format!(
            "https://maven.minecraftforge.net/{}",
            maven_to_path(&lib.name).display()
        );
        let _ = download_file(&url, &lib_path).await;
    }

    // 4. Run processors (client-side only)
    run_forge_processors(
        &install_profile,
        &libraries_dir,
        instance_dir,
        game_version,
        java,
    )
    .await?;

    // 5. Build LoaderProfile from version_json
    let profile = forge_version_to_loader_profile(&version_json);
    save_loader_profile(instance_dir, &profile).await?;

    // 6. Cleanup installer JAR
    let _ = tokio::fs::remove_file(&installer_path).await;

    println!("Forge installation complete");
    Ok(())
}

// === NeoForge ===

async fn install_neoforge(
    instance_dir: &Path,
    game_version: &str,
    neoforge_version: &str,
    java_path: Option<&Path>,
) -> Result<(), String> {
    println!(
        "Installing NeoForge {} for MC {}",
        neoforge_version, game_version
    );

    let java = java_path.ok_or("Java path required for NeoForge installation")?;
    let libraries_dir = instance_dir.join(".minecraft").join("libraries");
    let installer_path = instance_dir.join("neoforge-installer.jar");

    // 1. Download installer
    version_manager::neoforge::download_neoforge_installer(neoforge_version, &installer_path)
        .await?;

    // 2. Extract — reuse Forge extraction (same format)
    let libraries_dir_clone = libraries_dir.clone();
    let installer_path_clone = installer_path.clone();
    let (install_profile, version_json) = tokio::task::spawn_blocking(move || {
        version_manager::forge::extract_forge_installer(&installer_path_clone, &libraries_dir_clone)
    })
    .await
    .map_err(|e| e.to_string())??;

    // 3. Download libraries
    tokio::fs::create_dir_all(&libraries_dir)
        .await
        .map_err(|e| e.to_string())?;

    for lib in install_profile
        .libraries
        .iter()
        .chain(version_json.libraries.iter())
    {
        let lib_path = libraries_dir.join(maven_to_path(&lib.name));
        if lib_path.exists() {
            continue;
        }
        if let Some(parent) = lib_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref downloads) = lib.downloads {
            if let Some(ref artifact) = downloads.artifact {
                if !artifact.url.is_empty() {
                    download_file(&artifact.url, &lib_path).await?;
                    continue;
                }
            }
        }
        // Fallback: NeoForge maven
        let url = format!(
            "https://maven.neoforged.net/releases/{}",
            maven_to_path(&lib.name).display()
        );
        let _ = download_file(&url, &lib_path).await;
    }

    // 4. Run processors
    run_forge_processors(
        &install_profile,
        &libraries_dir,
        instance_dir,
        game_version,
        java,
    )
    .await?;

    // 5. Build LoaderProfile
    let profile = forge_version_to_loader_profile(&version_json);
    save_loader_profile(instance_dir, &profile).await?;

    // 6. Cleanup
    let _ = tokio::fs::remove_file(&installer_path).await;

    println!("NeoForge installation complete");
    Ok(())
}

// === Forge processor pipeline ===

async fn run_forge_processors(
    install_profile: &version_manager::models::ForgeInstallProfile,
    libraries_dir: &Path,
    instance_dir: &Path,
    game_version: &str,
    java_path: &Path,
) -> Result<(), String> {
    let game_dir = instance_dir.join(".minecraft");
    let versions_dir = game_dir.join("versions");
    let client_jar = versions_dir
        .join(game_version)
        .join(format!("{}.jar", game_version));

    // Build data map
    let mut data_map = std::collections::HashMap::new();
    data_map.insert(
        "MINECRAFT_JAR".to_string(),
        client_jar.to_string_lossy().to_string(),
    );
    data_map.insert("SIDE".to_string(), "client".to_string());
    data_map.insert(
        "ROOT".to_string(),
        game_dir.to_string_lossy().to_string(),
    );
    data_map.insert(
        "LIBRARY_DIR".to_string(),
        libraries_dir.to_string_lossy().to_string(),
    );

    // Add install_profile data entries
    for (key, entry) in &install_profile.data {
        let value = &entry.client;
        let resolved = if value.starts_with('[') && value.ends_with(']') {
            // Maven coordinate reference -> resolve to library path
            let coord = &value[1..value.len() - 1];
            libraries_dir
                .join(maven_to_path(coord))
                .to_string_lossy()
                .to_string()
        } else if value.starts_with('/') {
            // Path inside installer JAR (already extracted)
            libraries_dir
                .join("forge_extracted")
                .join(value.trim_start_matches('/'))
                .to_string_lossy()
                .to_string()
        } else {
            value.clone()
        };
        data_map.insert(key.clone(), resolved);
    }

    for processor in &install_profile.processors {
        // Skip server-side-only processors
        if let Some(ref sides) = processor.sides {
            if !sides.iter().any(|s| s == "client") {
                continue;
            }
        }

        // Build classpath for this processor
        let mut cp_entries = Vec::new();
        let processor_jar_path = libraries_dir.join(maven_to_path(&processor.jar));
        cp_entries.push(processor_jar_path.to_string_lossy().to_string());

        for cp_entry in &processor.classpath {
            let path = libraries_dir.join(maven_to_path(cp_entry));
            cp_entries.push(path.to_string_lossy().to_string());
        }

        let classpath = cp_entries.join(":");

        // Get main class from processor JAR
        let jar_for_main = processor_jar_path.clone();
        let main_class = tokio::task::spawn_blocking(move || {
            version_manager::forge::extract_jar_main_class(&jar_for_main)
        })
        .await
        .map_err(|e| e.to_string())??;

        // Resolve processor args
        let mut resolved_args = Vec::new();
        for arg in &processor.args {
            let resolved = resolve_forge_token(arg, &data_map, libraries_dir);
            resolved_args.push(resolved);
        }

        println!(
            "Running processor: {} {}",
            main_class,
            resolved_args.join(" ")
        );

        let java_owned = java_path.to_path_buf();
        let output = tokio::process::Command::new(&java_owned)
            .arg("-cp")
            .arg(&classpath)
            .arg(&main_class)
            .args(&resolved_args)
            .output()
            .await
            .map_err(|e| format!("Failed to run processor {}: {}", main_class, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "Forge processor {} failed:\nstdout: {}\nstderr: {}",
                main_class, stdout, stderr
            ));
        }
    }

    Ok(())
}

fn resolve_forge_token(
    token: &str,
    data_map: &std::collections::HashMap<String, String>,
    libraries_dir: &Path,
) -> String {
    if token.starts_with('{') && token.ends_with('}') {
        let key = &token[1..token.len() - 1];
        data_map.get(key).cloned().unwrap_or_else(|| token.to_string())
    } else if token.starts_with('[') && token.ends_with(']') {
        let coord = &token[1..token.len() - 1];
        libraries_dir
            .join(maven_to_path(coord))
            .to_string_lossy()
            .to_string()
    } else {
        token.to_string()
    }
}

fn forge_version_to_loader_profile(
    version_json: &version_manager::models::ForgeVersionJson,
) -> LoaderProfile {
    let libraries = version_json
        .libraries
        .iter()
        .map(|lib| version_manager::models::LoaderLibrary {
            name: lib.name.clone(),
            url: lib
                .downloads
                .as_ref()
                .and_then(|d| d.artifact.as_ref())
                .map(|a| {
                    // Extract base URL from full artifact URL
                    let path = maven_to_path(&lib.name);
                    let path_str = path.to_string_lossy();
                    a.url
                        .strip_suffix(&*path_str)
                        .unwrap_or("https://maven.minecraftforge.net/")
                        .to_string()
                }),
        })
        .collect();

    let mut jvm_args = Vec::new();
    let mut game_args = Vec::new();

    if let Some(ref args) = version_json.arguments {
        if let Some(ref jvm) = args.jvm {
            for arg in jvm {
                if let Some(s) = arg.as_str() {
                    jvm_args.push(s.to_string());
                }
            }
        }
        if let Some(ref game) = args.game {
            for arg in game {
                if let Some(s) = arg.as_str() {
                    game_args.push(s.to_string());
                }
            }
        }
    }

    LoaderProfile {
        main_class: version_json.main_class.clone(),
        libraries,
        jvm_args,
        game_args,
    }
}
