use directories::ProjectDirs;
use keyring::{Entry, Error as KeyringError};
use microsoft_auth::{DeviceCodeInfo, MicrosoftAuthenticator, MicrosoftTokens};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use uuid::Uuid;

const SERVICE_NAME: &str = "fastmc";

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("config directory unavailable")]
    ConfigDirMissing,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("keyring error: {0}")]
    Keyring(#[from] KeyringError),
    #[error("auth error: {0}")]
    Auth(#[from] microsoft_auth::AuthError),
    #[error("missing xbox user hash")]
    MissingUserHash,
    #[error("minecraft profile unavailable: {0}")]
    ProfileUnavailable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftSecrets {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountKind {
    Offline { username: String, uuid: String },
    Microsoft { uuid: String, username: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub display_name: String,
    pub kind: AccountKind,
    /// Optional path to a cached 64x64 head render.
    pub skin_path: Option<String>,
    #[serde(default)]
    pub requires_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub active: Option<Uuid>,
    pub accounts: Vec<Account>,
}

#[derive(Clone)]
pub struct AccountService {
    store: AccountStore,
    auth: MicrosoftAuthenticator,
    game: MicrosoftGameClient,
}

impl AccountService {
    pub fn new(client_id: impl Into<String>) -> Result<Self, AccountError> {
        Ok(Self {
            store: AccountStore::load()?,
            auth: MicrosoftAuthenticator::new(client_id),
            game: MicrosoftGameClient::new()?,
        })
    }

    pub fn accounts(&self) -> &AccountStore {
        &self.store
    }

    pub fn set_active(&mut self, account_id: Uuid) -> Result<(), AccountError> {
        if self.store.accounts.iter().any(|a| a.id == account_id) {
            self.store.active = Some(account_id);
            self.store.save()
        } else {
            Ok(())
        }
    }

    pub fn remove_account(&mut self, account_id: Uuid) -> Result<(), AccountError> {
        if let Some(pos) = self.store.accounts.iter().position(|a| a.id == account_id) {
            if matches!(self.store.accounts[pos].kind, AccountKind::Microsoft { .. }) {
                self.store.clear_microsoft_tokens(&account_id)?;
            }
            self.store.accounts.remove(pos);
            if self.store.active == Some(account_id) {
                self.store.active = None;
            }
            self.store.save()?;
        }
        Ok(())
    }

    pub fn add_offline(&mut self, username: String) -> Result<&Account, AccountError> {
        self.store.add_offline(username)
    }

    pub async fn start_microsoft_device_code(&self) -> Result<DeviceCodeInfo, AccountError> {
        Ok(self.auth.start_device_code().await?)
    }

    pub async fn complete_microsoft_login(
        &mut self,
        code: &DeviceCodeInfo,
    ) -> Result<&Account, AccountError> {
        let tokens: MicrosoftTokens = self.auth.poll_device_code(code).await?;
        let session = self.game.minecraft_session(&tokens).await?;
        self.store.upsert_microsoft(&session).await
    }

    pub async fn refresh_account(&mut self, account_id: &Uuid) -> Result<&Account, AccountError> {
        let secrets = load_microsoft_tokens(account_id)?.ok_or_else(|| {
            AccountError::Auth(microsoft_auth::AuthError::OAuth(
                "no tokens found".to_string(),
            ))
        })?;

        let tokens = self
            .auth
            .refresh_access_token(&secrets.refresh_token)
            .await?;
        let session = self.game.minecraft_session(&tokens).await?;
        self.store.upsert_microsoft(&session).await
    }

    pub async fn validate_active_account(&mut self) -> Result<&Account, AccountError> {
        let active_id = self.store.active.ok_or(AccountError::ProfileUnavailable(
            "No active account".to_string(),
        ))?;

        let is_microsoft = {
            let account = self
                .store
                .accounts
                .iter()
                .find(|a| a.id == active_id)
                .ok_or(AccountError::ProfileUnavailable(
                    "Active account not found in store".to_string(),
                ))?;
            matches!(account.kind, AccountKind::Microsoft { .. })
        };

        if is_microsoft {
            // Check if token is still valid before refreshing
            let should_refresh = match load_microsoft_tokens(&active_id)? {
                Some(secrets) => {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    // buffer of 5 minutes (300 seconds)
                    secrets.expires_at < now + 300 || secrets.access_token.is_empty()
                }
                None => true,
            };

            if !should_refresh {
                return Ok(self
                    .store
                    .accounts
                    .iter()
                    .find(|a| a.id == active_id)
                    .unwrap());
            }

            // We update the store via refresh_account, then re-fetch reference
            match self.refresh_account(&active_id).await {
                Ok(_) => Ok(self
                    .store
                    .accounts
                    .iter()
                    .find(|a| a.id == active_id)
                    .unwrap()),
                Err(e) => {
                    // Mark as requiring login
                    if let Some(account) =
                        self.store.accounts.iter_mut().find(|a| a.id == active_id)
                    {
                        account.requires_login = true;
                    }
                    self.store.save()?;
                    Err(e)
                }
            }
        } else {
            Ok(self
                .store
                .accounts
                .iter()
                .find(|a| a.id == active_id)
                .unwrap())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skin_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSession {
    pub access_token: String,
    pub expires_at: u64,
    pub refresh_token: String,
    pub profile: MinecraftProfile,
}

#[derive(Clone)]
pub struct MicrosoftGameClient {
    http: Client,
}

impl MicrosoftGameClient {
    pub fn new() -> Result<Self, AccountError> {
        let http = Client::builder().timeout(Duration::from_secs(15)).build()?;
        Ok(Self { http })
    }

    pub async fn minecraft_session(
        &self,
        microsoft: &MicrosoftTokens,
    ) -> Result<MinecraftSession, AccountError> {
        let (xbl_token, user_hash) = self.xbox_live_token(&microsoft.access_token).await?;
        let (xsts_token, user_hash) = self.xsts_token(&xbl_token, &user_hash).await?;
        let (minecraft_token, expires_in) = self.minecraft_login(&user_hash, &xsts_token).await?;
        let profile = self.minecraft_profile(&minecraft_token).await?;

        Ok(MinecraftSession {
            access_token: minecraft_token,
            expires_at: unix_timestamp_after(Duration::from_secs(expires_in)),
            refresh_token: microsoft.refresh_token.clone(),
            profile,
        })
    }

    async fn xbox_live_token(&self, access_token: &str) -> Result<(String, String), AccountError> {
        let payload = serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        });

        let response: XboxAuthResponse = self
            .http
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let uhs = response
            .display_claims
            .xui
            .first()
            .map(|c| c.uhs.clone())
            .ok_or(AccountError::MissingUserHash)?;

        Ok((response.token, uhs))
    }

    async fn xsts_token(
        &self,
        xbl_token: &str,
        uhs: &str,
    ) -> Result<(String, String), AccountError> {
        let payload = serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        });

        let response: XboxAuthResponse = self
            .http
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let user_hash = response
            .display_claims
            .xui
            .first()
            .map(|c| c.uhs.clone())
            .unwrap_or_else(|| uhs.to_string());

        Ok((response.token, user_hash))
    }

    async fn minecraft_login(
        &self,
        uhs: &str,
        xsts_token: &str,
    ) -> Result<(String, u64), AccountError> {
        let payload = serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
        });

        let response: MinecraftLoginResponse = self
            .http
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok((response.access_token, response.expires_in))
    }

    async fn minecraft_profile(
        &self,
        minecraft_token: &str,
    ) -> Result<MinecraftProfile, AccountError> {
        let response = self
            .http
            .get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(minecraft_token)
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            return Err(AccountError::ProfileUnavailable(
                "Minecraft not purchased for this account".to_string(),
            ));
        }

        let profile: MinecraftProfileResponse = response.error_for_status()?.json().await?;
        let skin_url = profile
            .skins
            .and_then(|skins| skins.into_iter().find(|s| s.state == "ACTIVE"))
            .map(|s| s.url);

        Ok(MinecraftProfile {
            id: profile.id,
            name: profile.name,
            skin_url,
        })
    }
}

#[derive(Debug, Deserialize)]
struct XboxAuthResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxUserHash>,
}

#[derive(Debug, Deserialize)]
struct XboxUserHash {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftLoginResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Option<Vec<MinecraftSkin>>,
}

#[derive(Debug, Deserialize)]
struct MinecraftSkin {
    #[serde(rename = "id")]
    _id: String,
    state: String,
    url: String,
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
            matches!(
                &acc.kind,
                AccountKind::Offline { username: name, .. } if name == &username
            )
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
            requires_login: false,
        };
        self.accounts.push(account);
        let last = self.accounts.last().unwrap().id;
        self.active = Some(last);
        self.save()?;
        Ok(self.accounts.last().unwrap())
    }

    pub async fn upsert_microsoft(
        &mut self,
        session: &MinecraftSession,
    ) -> Result<&Account, AccountError> {
        let profile = &session.profile;
        let skin_path = cache_skin_head(&profile.id).await?;

        if let Some(idx) = self.accounts.iter().position(|acc| {
            matches!(
                &acc.kind,
                AccountKind::Microsoft { uuid, .. } if uuid == &profile.id
            )
        }) {
            {
                let account = self.accounts.get_mut(idx).expect("valid index");
                account.display_name = profile.name.clone();
                account.skin_path = skin_path.clone();
                account.kind = AccountKind::Microsoft {
                    uuid: profile.id.clone(),
                    username: profile.name.clone(),
                };
                account.requires_login = false;
            }

            let account_id = self.accounts[idx].id;
            store_microsoft_tokens(account_id, session)?;
            self.active = Some(account_id);
            self.save()?;
            return Ok(&self.accounts[idx]);
        }

        let account = Account {
            id: Uuid::new_v4(),
            display_name: profile.name.clone(),
            skin_path: skin_path.clone(),
            kind: AccountKind::Microsoft {
                uuid: profile.id.clone(),
                username: profile.name.clone(),
            },
            requires_login: false,
        };

        self.accounts.push(account);
        let last_index = self.accounts.len() - 1;
        let last_id = self.accounts[last_index].id;
        store_microsoft_tokens(last_id, session)?;
        self.active = Some(last_id);
        self.save()?;
        Ok(&self.accounts[last_index])
    }

    pub fn microsoft_tokens(
        &self,
        account_id: &Uuid,
    ) -> Result<Option<MicrosoftSecrets>, AccountError> {
        load_microsoft_tokens(account_id)
    }

    pub fn clear_microsoft_tokens(&self, account_id: &Uuid) -> Result<(), AccountError> {
        let entry = keyring_entry(account_id)?;
        match entry.delete_password() {
            Ok(_) => Ok(()),
            Err(KeyringError::NoEntry) => Ok(()),
            Err(err) => Err(AccountError::Keyring(err)),
        }
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

async fn cache_skin_head(uuid: &str) -> Result<Option<String>, AccountError> {
    let cache_dir = skin_cache_dir()?;
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)?;
    }

    let url = format!(
        "https://crafatar.com/avatars/{}?size=64&overlay",
        uuid.replace('-', "")
    );

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response.bytes().await?;
    let dest = cache_dir.join(format!("{}.png", uuid));
    fs::write(&dest, bytes)?;
    Ok(Some(dest.to_string_lossy().to_string()))
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

fn unix_timestamp_after(duration: Duration) -> u64 {
    SystemTime::now()
        .checked_add(duration)
        .unwrap_or(SystemTime::now())
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn keyring_entry(account_id: &Uuid) -> Result<Entry, AccountError> {
    Ok(Entry::new(SERVICE_NAME, &format!("account-{account_id}"))?)
}

fn store_microsoft_tokens(
    account_id: Uuid,
    session: &MinecraftSession,
) -> Result<(), AccountError> {
    let secrets = MicrosoftSecrets {
        #[cfg(target_os = "windows")]
        access_token: String::new(), // Too large for Windows keyring; rely on refresh_token
        #[cfg(not(target_os = "windows"))]
        access_token: session.access_token.clone(),
        refresh_token: session.refresh_token.clone(),
        expires_at: session.expires_at,
    };

    let entry = keyring_entry(&account_id)?;
    let payload = serde_json::to_string(&secrets)?;
    entry.set_password(&payload)?;
    Ok(())
}

fn load_microsoft_tokens(account_id: &Uuid) -> Result<Option<MicrosoftSecrets>, AccountError> {
    let entry = keyring_entry(account_id)?;
    match entry.get_password() {
        Ok(raw) => Ok(Some(serde_json::from_str(&raw)?)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(AccountError::Keyring(err)),
    }
}
