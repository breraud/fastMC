use account_manager::Account;
use launcher::{LaunchAuth, MemorySettings, Resolution, VanillaLaunchConfig};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;


#[allow(dead_code)]
pub enum LaunchProgress {
    Downloading(String, f32), // File, percentage
    Extracting,
    Launching,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    libraries: Vec<Library>,
    #[serde(rename = "mainClass")]
    main_class: String,
    downloads: VersionDownloads,
    #[serde(rename = "assetIndex")]
    asset_index: AssetIndexRef,
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
    #[allow(dead_code)]
    sha1: String,
    #[allow(dead_code)]
    size: u64,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Library {
    downloads: LibraryDownloads,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct LibraryDownloads {
    artifact: Option<DownloadFile>,
    classifiers: Option<serde_json::Value>, 
}

pub async fn prepare_and_launch(
    account: &Account,
    access_token: &str,
    java_path: PathBuf,
    game_dir: PathBuf,
    version_id: &str,
) -> Result<Command, String> {
    // 1. Setup directories
    let versions_dir = game_dir.join("versions");
    let libraries_dir = game_dir.join("libraries");
    let assets_dir = game_dir.join("assets");
    let natives_dir = game_dir.join("natives").join(version_id);

    fs::create_dir_all(&versions_dir).await.map_err(|e| e.to_string())?;
    fs::create_dir_all(&libraries_dir).await.map_err(|e| e.to_string())?;
    fs::create_dir_all(&assets_dir).await.map_err(|e| e.to_string())?;
    fs::create_dir_all(&natives_dir).await.map_err(|e| e.to_string())?;

    // 2. Fetch Manifest
    let version_json_path = versions_dir
        .join(version_id)
        .join(format!("{}.json", version_id));

    println!("Checking version manifest at {:?}", version_json_path);

    // We can't use a closure easily with async recursion/await inside without BoxFuture.
    // So we'll just inline the logic or use a loop.
    let version_data: VersionData = if version_json_path.exists() {
         let content = fs::read_to_string(&version_json_path).await.map_err(|e| e.to_string())?;
         match serde_json::from_str::<VersionData>(&content) {
            Ok(data) => data,
            Err(_) => {
                println!("Local manifest corrupted. Re-downloading...");
                fetch_manifest(version_id, &versions_dir, &version_json_path).await?
            }
         }
    } else {
        fetch_manifest(version_id, &versions_dir, &version_json_path).await?
    };

    // 3. Download Client JAR
    let client_jar = versions_dir
        .join(version_id)
        .join(format!("{}.jar", version_id));
    if !client_jar.exists() {
        download_file(&version_data.downloads.client.url, &client_jar).await?;
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
                    fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
                }
                download_file(&artifact.url, &lib_path).await?;
            }
            classpath.push(lib_path);
        }

        // Natives
        if let Some(classifiers) = lib.downloads.classifiers {
            let os_classifier = if cfg!(target_os = "windows") {
                "natives-windows"
            } else if cfg!(target_os = "macos") {
                "natives-macos"
            } else if cfg!(target_os = "linux") {
                "natives-linux"
            } else {
                "natives-unknown"
            };

            if let Some(native_obj) = classifiers.get(os_classifier) {
                if let Ok(file_info) = serde_json::from_value::<DownloadFile>(native_obj.clone()) {
                    let nat_path = libraries_dir.join(format!("{}-{}.jar", lib.name.replace(':', "-"), os_classifier));
                    
                    if !nat_path.exists() {
                        download_file(&file_info.url, &nat_path).await?;
                    }

                    // Extract (Synchronous - handled in blocking task)
                    let nat_path_clone = nat_path.clone();
                    let natives_dir_clone = natives_dir.clone();
                    
                    tokio::task::spawn_blocking(move || {
                        if let Ok(file) = std::fs::File::open(&nat_path_clone) {
                            if let Ok(mut archive) = zip::ZipArchive::new(file) {
                                for i in 0..archive.len() {
                                    if let Ok(mut file) = archive.by_index(i) {
                                        if file.name().contains("META-INF") { continue; }
                                        let outpath = natives_dir_clone.join(file.name());
                                        if let Some(p) = outpath.parent() {
                                            std::fs::create_dir_all(p).ok();
                                        }
                                        if let Ok(mut outfile) = std::fs::File::create(&outpath) {
                                            std::io::copy(&mut file, &mut outfile).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }).await.map_err(|e| e.to_string())?;
                }
            }
        }
    }
    classpath.push(client_jar);

    // 5. Assets Index and Objects
    let asset_index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", version_data.asset_index.id));
    if !asset_index_path.exists() {
        if let Some(parent) = asset_index_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
        }
        download_file(&version_data.asset_index.url, &asset_index_path).await?;
    }

    println!("Verifying assets from index: {:?}", asset_index_path);
    let index_content = fs::read_to_string(&asset_index_path).await.map_err(|e| e.to_string())?;
    let index_data: serde_json::Value =
        serde_json::from_str(&index_content).map_err(|e| e.to_string())?;

    if let Some(objects) = index_data["objects"].as_object() {
        let objects_dir = assets_dir.join("objects");
        // For performance, we should parallelize this. But strict sequential for now to avoid complexity.
        // Or simple concurrency.
        for (_name, obj) in objects {
            if let Some(hash) = obj["hash"].as_str()
                && hash.len() >= 2
            {
                let prefix = &hash[..2];
                let object_path = objects_dir.join(prefix).join(hash);

                if !object_path.exists() {
                    let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);
                    if let Some(parent) = object_path.parent() {
                        fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
                    }
                     match download_file(&url, &object_path).await {
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
        main_class: version_data.main_class,
        version_name: version_id.to_string(),
        asset_index: Some(version_data.asset_index.id),
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

async fn fetch_manifest(version_id: &str, versions_dir: &Path, json_path: &Path) -> Result<VersionData, String> {
    let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    println!("Fetching main manifest from {}", manifest_url);
    
    // Async client
    let client = reqwest::Client::new();
    let resp = client.get(manifest_url).send().await.map_err(|e| e.to_string())?;
    
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch manifest: {}", resp.status()));
    }
    
    let manifest_json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    
    let version_url = manifest_json["versions"]
        .as_array()
        .ok_or("Invalid manifest format")?
        .iter()
        .find(|v| v["id"] == version_id)
        .and_then(|v| v["url"].as_str())
        .ok_or(format!("Version {} not found", version_id))?
        .to_string();

    println!("Found {} URL: {}", version_id, version_url);

    let resp = client.get(&version_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
         return Err(format!("Failed to fetch version json: {}", resp.status()));
    }
    let content = resp.text().await.map_err(|e| e.to_string())?;

    fs::create_dir_all(versions_dir.join(version_id)).await.map_err(|e| e.to_string())?;
    fs::write(json_path, &content).await.map_err(|e| e.to_string())?;

    serde_json::from_str(&content).map_err(|e| e.to_string())
}

async fn download_file(url: &str, path: &Path) -> Result<(), String> {
    println!("Downloading {} to {:?}", url, path);
    // Use reqwest async
    let resp = reqwest::get(url).await.map_err(|e| format!("Failed to GET {}: {}", url, e))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    
    fs::write(path, bytes).await.map_err(|e| format!("Write failed: {}", e))?;
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
