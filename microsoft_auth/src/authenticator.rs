use crate::errors::AuthError;
use crate::models::{
    DeviceCodeInfo, DeviceResponse, MicrosoftTokens, MinecraftProfile, MinecraftSession,
};
use crate::responses::{MinecraftLoginResponse, MinecraftProfileResponse, XboxAuthResponse};
use oauth2::basic::BasicClient;
use oauth2::devicecode::DeviceCodeErrorResponseType;
use oauth2::reqwest::http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, RequestTokenError, Scope, TokenResponse, TokenUrl,
};
use reqwest::blocking::Client;
use std::thread;
use std::time::{Duration, SystemTime};

pub struct MicrosoftAuthenticator {
    client: BasicClient,
    http: Client,
}

impl MicrosoftAuthenticator {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client: oauth_client(client_id.into()),
            http: Client::builder().build().expect("reqwest client"),
        }
    }

    pub fn start_device_code(&self) -> Result<DeviceCodeInfo, AuthError> {
        let request = self
            .client
            .exchange_device_code()
            .map_err(|err| AuthError::OAuth(err.to_string()))?;

        let response: DeviceResponse = request
            .add_scope(Scope::new("XboxLive.signin".into()))
            .add_scope(Scope::new("offline_access".into()))
            .request(http_client)
            .map_err(|err| AuthError::OAuth(err.to_string()))?;

        let message = format!(
            "Visit {} and enter code {}",
            response.verification_uri().as_str(),
            response.user_code().secret()
        );

        Ok(DeviceCodeInfo {
            user_code: response.user_code().secret().to_string(),
            verification_uri: response.verification_uri().as_str().to_string(),
            verification_uri_complete: response
                .verification_uri_complete()
                .map(|u| u.secret().to_string()),
            message,
            expires_in: response.expires_in().as_secs(),
            interval: response.interval().as_secs(),
            raw: response,
        })
    }

    pub fn poll_device_code(&self, code: &DeviceCodeInfo) -> Result<MicrosoftTokens, AuthError> {
        let token = self
            .client
            .exchange_device_access_token(&code.raw)
            .request(
                http_client,
                thread::sleep,
                Some(Duration::from_secs(code.expires_in)),
            )
            .map_err(|err| match err {
                RequestTokenError::ServerResponse(resp)
                    if resp.error() == &DeviceCodeErrorResponseType::ExpiredToken =>
                {
                    AuthError::OAuth("device code expired".to_string())
                }
                other => AuthError::OAuth(other.to_string()),
            })?;

        let access_token = token.access_token().secret().to_owned();
        let refresh_token = token
            .refresh_token()
            .map(|v| v.secret().to_owned())
            .ok_or(AuthError::MissingRefreshToken)?;
        let expires_in = token
            .expires_in()
            .unwrap_or_else(|| Duration::from_secs(3600));

        Ok(MicrosoftTokens {
            access_token,
            refresh_token,
            expires_at: unix_timestamp_after(expires_in),
        })
    }

    pub fn minecraft_session(
        &self,
        microsoft: &MicrosoftTokens,
    ) -> Result<MinecraftSession, AuthError> {
        let (xbl_token, user_hash) = self.xbox_live_token(&microsoft.access_token)?;
        let (xsts_token, user_hash) = self.xsts_token(&xbl_token, &user_hash)?;
        let (minecraft_token, expires_in) = self.minecraft_login(&user_hash, &xsts_token)?;
        let profile = self.minecraft_profile(&minecraft_token)?;

        Ok(MinecraftSession {
            access_token: minecraft_token,
            expires_at: unix_timestamp_after(Duration::from_secs(expires_in)),
            refresh_token: microsoft.refresh_token.clone(),
            profile,
        })
    }

    fn xbox_live_token(&self, access_token: &str) -> Result<(String, String), AuthError> {
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
            .send()?
            .error_for_status()?
            .json()?;

        let uhs = response
            .display_claims
            .xui
            .first()
            .map(|c| c.uhs.clone())
            .ok_or(AuthError::MissingUserHash)?;

        Ok((response.token, uhs))
    }

    fn xsts_token(&self, xbl_token: &str, uhs: &str) -> Result<(String, String), AuthError> {
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
            .send()?
            .error_for_status()?
            .json()?;

        let user_hash = response
            .display_claims
            .xui
            .first()
            .map(|c| c.uhs.clone())
            .unwrap_or_else(|| uhs.to_string());

        Ok((response.token, user_hash))
    }

    fn minecraft_login(&self, uhs: &str, xsts_token: &str) -> Result<(String, u64), AuthError> {
        let payload = serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
        });

        let response: MinecraftLoginResponse = self
            .http
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&payload)
            .send()?
            .error_for_status()?
            .json()?;

        Ok((response.access_token, response.expires_in))
    }

    fn minecraft_profile(&self, minecraft_token: &str) -> Result<MinecraftProfile, AuthError> {
        let response = self
            .http
            .get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(minecraft_token)
            .send()?;

        if response.status().as_u16() == 404 {
            return Err(AuthError::ProfileUnavailable(
                "Minecraft not purchased for this account".to_string(),
            ));
        }

        let profile: MinecraftProfileResponse = response.error_for_status()?.json()?;
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

fn oauth_client(client_id: String) -> BasicClient {
    BasicClient::new(
        ClientId::new(client_id),
        None,
        AuthUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize".to_string(),
        )
        .expect("Auth URL should be valid"),
        Some(
            TokenUrl::new(
                "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string(),
            )
            .expect("Token URL should be valid"),
        ),
    )
    .set_device_authorization_url(
        DeviceAuthorizationUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".to_string(),
        )
        .expect("Device authorization URL should be valid"),
    )
}

fn unix_timestamp_after(duration: Duration) -> u64 {
    SystemTime::now()
        .checked_add(duration)
        .unwrap_or(SystemTime::now())
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
