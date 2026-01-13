use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct XboxAuthResponse {
    #[serde(rename = "Token")]
    pub(crate) token: String,
    #[serde(rename = "DisplayClaims")]
    pub(crate) display_claims: XboxDisplayClaims,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XboxDisplayClaims {
    pub(crate) xui: Vec<XboxUserHash>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XboxUserHash {
    pub(crate) uhs: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MinecraftLoginResponse {
    pub(crate) access_token: String,
    pub(crate) expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MinecraftProfileResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) skins: Option<Vec<MinecraftSkin>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MinecraftSkin {
    #[serde(rename = "id")]
    pub(crate) _id: String,
    pub(crate) state: String,
    pub(crate) url: String,
}
