use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use serde::{Deserialize, Serialize};

pub(crate) type DeviceResponse = StandardDeviceAuthorizationResponse;

#[derive(Debug, Clone)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub message: String,
    pub expires_in: u64,
    pub interval: u64,
    pub(crate) raw: DeviceResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
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
