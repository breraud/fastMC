use account_manager::Account;
use launcher::{LaunchAuth, MemorySettings, Resolution, VanillaLaunchConfig};
use serde::Deserialize;
use std::fs;
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;

pub enum LaunchProgress {
    Downloading(String, f32), // File, percentage
    Extracting,
    Launching,
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    libraries: Vec<Library>,
    mainClass: String,
    downloads: VersionDownloads,
    assetIndex: AssetIndexRef,
}

#[derive(Debug, Deserialize)]
struct AssetIndexRef {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionDownloads {
    client: DownloadFile,
}

#[derive(Debug, Deserialize, Clone)]
struct DownloadFile {
    url: String,
    sha1: String,
    size: u64,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Library {
    downloads: LibraryDownloads,
    name: String,
}

#[derive(Debug, Deserialize)]
struct LibraryDownloads {
    artifact: Option<DownloadFile>,
    classifiers: Option<serde_json::Value>, // Simplified for now
}

pub fn prepare_and_launch(
    account: &Account,
    access_token: &str,
    java_path: PathBuf,
    game_dir: PathBuf,
) -> Result<Command, String> {
    // 1. Setup directories
    let versions_dir = game_dir.join("versions");
    let libraries_dir = game_dir.join("libraries");
    let assets_dir = game_dir.join("assets");
    let natives_dir = game_dir.join("natives").join("1.21");

    fs::create_dir_all(&versions_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&libraries_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;

    // 2. Fetch Manifest
    let version_id = "1.21";
    let version_json_path = versions_dir
        .join(version_id)
        .join(format!("{}.json", version_id));

    println!("Checking version manifest at {:?}", version_json_path);

    let version_data: VersionData = {
        // Helper to fetch and save
        let fetch_manifest = || -> Result<VersionData, String> {
            // Step 1: Fetch the main version manifest to get the dynamic URL for 1.21
            let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
            println!("Fetching main manifest from {}", manifest_url);
            let client = reqwest::blocking::Client::new();
            let resp = client.get(manifest_url).send().map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!(
                    "Failed to fetch main manifest: Status {}",
                    resp.status()
                ));
            }
            let manifest_json: serde_json::Value = resp
                .json()
                .map_err(|e| format!("Failed to parse main manifest: {}", e))?;

            let version_url = manifest_json["versions"]
                .as_array()
                .ok_or("Invalid manifest format")?
                .iter()
                .find(|v| v["id"] == "1.21")
                .and_then(|v| v["url"].as_str())
                .ok_or("Version 1.21 not found in manifest")?
                .to_string();

            println!("Found 1.21 URL: {}", version_url);

            // Step 2: Fetch the actual version json
            let resp = client.get(&version_url).send().map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!(
                    "Failed to fetch 1.21 manifest: Status {}",
                    resp.status()
                ));
            }
            let content = resp.text().map_err(|e| e.to_string())?;

            fs::create_dir_all(versions_dir.join(version_id)).map_err(|e| e.to_string())?;
            fs::write(&version_json_path, &content).map_err(|e| e.to_string())?;

            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse downloaded manifest: {}", e))
        };

        if version_json_path.exists() {
            match fs::read_to_string(&version_json_path) {
                Ok(content) => match serde_json::from_str::<VersionData>(&content) {
                    Ok(data) => data,
                    Err(e) => {
                        println!(
                            "Local manifest corrupted ({}). Deleting and re-downloading...",
                            e
                        );
                        fs::remove_file(&version_json_path).ok();
                        fetch_manifest()?
                    }
                },
                Err(_) => fetch_manifest()?,
            }
        } else {
            fetch_manifest()?
        }
    };

    // 3. Download Client JAR
    let client_jar = versions_dir
        .join(version_id)
        .join(format!("{}.jar", version_id));
    if !client_jar.exists() {
        download_file(&version_data.downloads.client.url, &client_jar)?;
    }

    // 4. Download Libraries (Including Natives)
    let mut classpath = vec![];
    for lib in version_data.libraries {
        // Standard library
        if let Some(artifact) = lib.downloads.artifact {
            let rel_path = if let Some(p) = artifact.path {
                p
            } else {
                maven_to_path(&lib.name).to_string_lossy().to_string()
            };

            let lib_path = libraries_dir.join(&rel_path);

            if !lib_path.exists() {
                if let Some(parent) = lib_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                download_file(&artifact.url, &lib_path)?;
            }
            classpath.push(lib_path);
        }

        // Natives
        if let Some(classifiers) = lib.downloads.classifiers {
            // For Windows, we look for "natives-windows"
            if let Some(native_obj) = classifiers.get("natives-windows") {
                // Deserialize manually or assume structure
                if let Ok(file_info) = serde_json::from_value::<DownloadFile>(native_obj.clone()) {
                    // Download native jar
                    let nat_path =
                        libraries_dir.join(format!("{}-native.jar", lib.name.replace(':', "-")));
                    if !nat_path.exists() {
                        download_file(&file_info.url, &nat_path)?;
                    }

                    // Extract (basic unzip)
                    // In a real implementation we should filter META-INF
                    if let Ok(file) = fs::File::open(&nat_path) {
                        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                        for i in 0..archive.len() {
                            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
                            let outpath = natives_dir.join(file.name());

                            if file.name().contains("META-INF") {
                                continue;
                            }

                            if let Some(p) = outpath.parent() {
                                fs::create_dir_all(p).map_err(|e| e.to_string())?;
                            }
                            let mut outfile =
                                fs::File::create(&outpath).map_err(|e| e.to_string())?;
                            io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }
    }
    classpath.push(client_jar);

    // 5. Assets Index and Objects
    let asset_index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", version_data.assetIndex.id));
    if !asset_index_path.exists() {
        fs::create_dir_all(asset_index_path.parent().unwrap()).map_err(|e| e.to_string())?;
        download_file(&version_data.assetIndex.url, &asset_index_path)?;
    }

    // Process Asset Index to download actual objects
    println!("Verifying assets from index: {:?}", asset_index_path);
    let index_content = fs::read_to_string(&asset_index_path).map_err(|e| e.to_string())?;
    let index_data: serde_json::Value =
        serde_json::from_str(&index_content).map_err(|e| e.to_string())?;

    if let Some(objects) = index_data["objects"].as_object() {
        let objects_dir = assets_dir.join("objects");
        for (_name, obj) in objects {
            if let Some(hash) = obj["hash"].as_str()
                && hash.len() >= 2 {
                    let prefix = &hash[..2];
                    let object_path = objects_dir.join(prefix).join(hash);

                    if !object_path.exists() {
                        let url = format!(
                            "https://resources.download.minecraft.net/{}/{}",
                            prefix, hash
                        );
                        if let Some(parent) = object_path.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        // Use a lighter download function or silent one to avoid spamming console for 1000s of assets
                        // For now we assume standard download_file but maybe suppress log if too spammy
                        // Let's just download it.
                        // println!("Downloading asset {}", hash);
                        match download_file(&url, &object_path) {
                            Ok(_) => {}
                            Err(e) => println!("Failed to download asset {}: {}", hash, e),
                        }
                    }
                }
        }
    }

    // 6. Build Config
    let config = VanillaLaunchConfig {
        java_path,
        game_dir: game_dir.clone(),
        assets_dir,
        classpath,
        main_class: version_data.mainClass,
        version_name: version_id.to_string(),
        asset_index: Some(version_data.assetIndex.id),
        resolution: Some(Resolution {
            width: 854,
            height: 480,
        }),
        memory: Some(MemorySettings {
            min_megabytes: 1024,
            max_megabytes: 4096,
        }),
        extra_jvm_args: vec![],
        extra_game_args: vec![],
        natives_dir: Some(natives_dir),
    };

    // 7. Launch Auth
    let auth = match &account.kind {
        account_manager::AccountKind::Microsoft { uuid, username } => LaunchAuth::Microsoft {
            username: username.clone(),
            uuid: uuid.clone(),
            access_token: access_token.to_string(),
        },
        account_manager::AccountKind::Offline { username, uuid } => LaunchAuth::Offline {
            username: username.clone(),
            uuid: uuid.clone(),
        },
    };

    Ok(config.build_command(&auth))
}

fn download_file(url: &str, path: &Path) -> Result<(), String> {
    println!("Downloading {} to {:?}", url, path);
    let mut response =
        reqwest::blocking::get(url).map_err(|e| format!("Failed to GET {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed for {}: Status {}",
            url,
            response.status()
        ));
    }

    let mut file =
        fs::File::create(path).map_err(|e| format!("Failed to create file {:?}: {}", path, e))?;
    io::copy(&mut response, &mut file)
        .map_err(|e| format!("Failed to write to {:?}: {}", path, e))?;
    Ok(())
}

fn maven_to_path(maven_id: &str) -> PathBuf {
    let parts: Vec<&str> = maven_id.split(':').collect();
    let domain = parts[0].replace('.', "/");
    let name = parts[1];
    let version = parts[2];
    PathBuf::from(format!(
        "{}/{}/{}/{}-{}.jar",
        domain, name, version, name, version
    ))
}
