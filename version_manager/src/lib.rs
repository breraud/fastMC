pub mod fabric;
pub mod models;
pub mod vanilla;

pub use fabric::*;
pub use models::*;
pub use vanilla::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_vanilla() {
        let versions = vanilla::fetch_vanilla_versions().await;
        assert!(versions.is_ok());
        let versions = versions.unwrap();
        assert!(!versions.is_empty());
        println!("Found {} vanilla versions", versions.len());
        println!("Latest: {:?}", versions.first());
    }

    #[tokio::test]
    async fn test_fetch_fabric() {
        let loaders = fabric::fetch_fabric_loaders().await;
        assert!(loaders.is_ok());
        let loaders = loaders.unwrap();
        assert!(!loaders.is_empty());
        println!("Found {} fabric loaders", loaders.len());
    }
}
