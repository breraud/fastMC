use iced::widget::{image as iced_image, svg};
use std::collections::HashMap;
use std::path::Path;
// Use external image crate, distinct from iced's image module/widget
use image as image_crate;

#[derive(Debug, Clone)]
pub struct AssetStore {
    pub icons: HashMap<String, svg::Handle>,
    pub images: HashMap<String, iced_image::Handle>,
}

impl Default for AssetStore {
    fn default() -> Self {
        Self {
            icons: HashMap::new(),
            images: HashMap::new(),
        }
    }
}

impl AssetStore {
    pub async fn load() -> Self {
        let mut store = Self::default();
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let assets_dir = manifest_dir.join("assets");

        // Load SVGs
        let svgs = vec![
            "play.svg",
            "server.svg",
            "package.svg",
            "coffee.svg",
            "settings.svg",
        ];

        for name in svgs {
            let path = assets_dir.join("svg").join(name);
            if path.exists() {
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    store
                        .icons
                        .insert(name.to_string(), svg::Handle::from_memory(bytes));
                } else {
                    eprintln!("Warning: Failed to read icon: {:?}", path);
                }
            } else {
                eprintln!("Warning: Icon not found: {:?}", path);
            }
        }

        // Load Images (e.g. default instance background)
        let images = vec![
            "instances_images/default.jpg",
            "favicon.png",
            "favicon_noblur.png",
            "wide_logo.png",
        ];

        for rel_path in images {
            let path = assets_dir.join(rel_path);
            if path.exists() {
                let key = rel_path.to_string();
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    // Decode image to RGBA8 to ensure it's ready for GPU
                    // This happens in the background task
                    match image_crate::load_from_memory(&bytes) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let width = rgba.width();
                            let height = rgba.height();
                            let pixels = rgba.into_raw();

                            let handle = iced_image::Handle::from_rgba(width, height, pixels);
                            store.images.insert(key, handle);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to decode image {:?}: {}", path, e);
                        }
                    }
                } else {
                    eprintln!("Warning: Failed to read image: {:?}", path);
                }
            } else {
                eprintln!("Warning: Image not found: {:?}", path);
            }
        }

        store
    }

    pub fn get_icon(&self, name: &str) -> Option<svg::Handle> {
        self.icons.get(name).cloned()
    }

    pub fn get_image(&self, name: &str) -> Option<iced_image::Handle> {
        self.images.get(name).cloned()
    }
}
