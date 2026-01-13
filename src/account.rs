#![allow(dead_code)]

use directories::ProjectDirs;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use microsoft_auth::MinecraftSession;

#[derive(Debug)]
pub enum AccountError {
    Io(io::Error),
    Json(serde_json::Error),
    Http(reqwest::Error),
    ConfigDirMissing,
}

impl From<io::Error> for AccountError {
    fn from(value: io::Error) -> Self {
        AccountError::Io(value)
    }
}

impl From<serde_json::Error> for AccountError {
    fn from(value: serde_json::Error) -> Self {
        AccountError::Json(value)
    }
}

impl From<reqwest::Error> for AccountError {
    fn from(value: reqwest::Error) -> Self {
        AccountError::Http(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountKind {
    Offline {
        username: String,
        #[serde(default)]
        uuid: String,
    },
    Microsoft {
        uuid: String,
        username: String,
        access_token: String,
        refresh_token: String,
        expires_at: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub display_name: String,
    pub kind: AccountKind,
    /// Optional path to a cached 64x64 head render.
    pub skin_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub active: Option<Uuid>,
    pub accounts: Vec<Account>,
}

impl AccountStore {
    pub fn load() -> Result<Self, AccountError> {
        let path = accounts_file()?;
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let mut store: Self = serde_json::from_str(&content)?;
            store.ensure_offline_uuids();
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), AccountError> {
        let path = accounts_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn add_offline(&mut self, username: String) -> Result<&Account, AccountError> {
        if let Some(idx) = self.accounts.iter().position(|acc| {
            matches!(&acc.kind, AccountKind::Offline { username: name, .. } if name == &username)
        }) {
            let account_id = self.accounts[idx].id;
            self.active = Some(account_id);
            self.save()?;
            return Ok(&self.accounts[idx]);
        }

        let offline_uuid = offline_uuid(&username);
        let account = Account {
            id: Uuid::new_v4(),
            display_name: username.clone(),
            kind: AccountKind::Offline {
                username,
                uuid: offline_uuid.to_string(),
            },
            skin_path: None,
        };
        self.accounts.push(account);
        let last = self.accounts.last().unwrap().id;
        self.active = Some(last);
        self.save()?;
        Ok(self.accounts.last().unwrap())
    }

    pub fn upsert_microsoft(
        &mut self,
        session: &MinecraftSession,
    ) -> Result<&Account, AccountError> {
        let minecraft_profile = &session.profile;
        let expires_at = session.expires_at;
        let skin_path =
            cache_skin_head(&minecraft_profile.id)?.map(|path| path.to_string_lossy().to_string());

        if let Some(idx) = self.accounts.iter().position(|acc| {
            matches!(
                &acc.kind,
                AccountKind::Microsoft { uuid, .. } if uuid == &minecraft_profile.id
            )
        }) {
            {
                let account = self.accounts.get_mut(idx).expect("valid index");
                account.display_name = minecraft_profile.name.clone();
                account.skin_path = skin_path.clone();
                account.kind = AccountKind::Microsoft {
                    uuid: minecraft_profile.id.clone(),
                    username: minecraft_profile.name.clone(),
                    access_token: session.access_token.clone(),
                    refresh_token: session.refresh_token.clone(),
                    expires_at,
                };
            }

            let account_id = self.accounts[idx].id;
            self.active = Some(account_id);
            self.save()?;
            return Ok(&self.accounts[idx]);
        }

        let account = Account {
            id: Uuid::new_v4(),
            display_name: minecraft_profile.name.clone(),
            skin_path,
            kind: AccountKind::Microsoft {
                uuid: minecraft_profile.id.clone(),
                username: minecraft_profile.name.clone(),
                access_token: session.access_token.clone(),
                refresh_token: session.refresh_token.clone(),
                expires_at,
            },
        };

        self.accounts.push(account);
        let last_index = self.accounts.len() - 1;
        let last_id = self.accounts[last_index].id;
        self.active = Some(last_id);
        self.save()?;
        Ok(&self.accounts[last_index])
    }

    fn ensure_offline_uuids(&mut self) {
        for account in &mut self.accounts {
            if let AccountKind::Offline { username, uuid } = &mut account.kind
                && uuid.is_empty()
            {
                *uuid = offline_uuid(username).to_string();
            }
        }
    }
}

fn cache_skin_head(uuid: &str) -> Result<Option<PathBuf>, AccountError> {
    let cache_dir = skin_cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    let url = format!(
        "https://crafatar.com/avatars/{}?size=64&overlay",
        uuid.replace('-', "")
    );

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;

    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response.bytes()?;
    let dest = cache_dir.join(format!("{}.png", uuid));
    fs::write(&dest, bytes)?;
    Ok(Some(dest))
}

fn skin_cache_dir() -> Result<PathBuf, AccountError> {
    data_dir().map(|root| root.join("skins"))
}

fn accounts_file() -> Result<PathBuf, AccountError> {
    data_dir().map(|root| root.join("accounts.json"))
}

fn data_dir() -> Result<PathBuf, AccountError> {
    let dirs =
        ProjectDirs::from("com", "fastmc", "fastmc").ok_or(AccountError::ConfigDirMissing)?;
    Ok(dirs.data_dir().to_path_buf())
}

fn offline_uuid(username: &str) -> Uuid {
    use md5::{Digest, Md5};

    let input = format!("OfflinePlayer:{username}");
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // set to version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // set to RFC 4122 variant
    Uuid::from_bytes(bytes)
}
