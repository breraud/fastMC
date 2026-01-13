use crate::errors::AuthError;
use crate::models::{DeviceCodeInfo, DeviceResponse, MicrosoftTokens};
use oauth2::basic::BasicClient;
use oauth2::devicecode::DeviceCodeErrorResponseType;
use oauth2::reqwest::http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, RequestTokenError, Scope, TokenResponse, TokenUrl,
};
use std::thread;
use std::time::{Duration, SystemTime};

pub struct MicrosoftAuthenticator {
    client: BasicClient,
}

impl MicrosoftAuthenticator {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client: oauth_client(client_id.into()),
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
            .ok_or_else(|| AuthError::OAuth("missing refresh token".to_string()))?;
        let expires_in = token
            .expires_in()
            .unwrap_or_else(|| Duration::from_secs(3600));

        Ok(MicrosoftTokens {
            access_token,
            refresh_token,
            expires_at: unix_timestamp_after(expires_in),
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
