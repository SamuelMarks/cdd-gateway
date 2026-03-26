use serde::{Deserialize, Serialize};

/// GitHubUser structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubUser {
    /// id field
    pub id: i64,
    /// login field
    pub login: String,
    /// avatar_url field
    pub avatar_url: String,
}

/// GitHubEmail structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubEmail {
    /// email field
    pub email: String,
    /// primary field
    pub primary: bool,
    /// verified field
    pub verified: bool,
}

/// GitHubOrg structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubOrg {
    /// id field
    pub id: i64,
    /// login field
    pub login: String,
    /// description field
    pub description: Option<String>,
}

/// GitHubRepo structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRepo {
    /// id field
    pub id: i64,
    /// name field
    pub name: String,
    /// full_name field
    pub full_name: String,
    /// description field
    pub description: Option<String>,
}

/// GitHubRelease structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRelease {
    /// id field
    pub id: i64,
    /// tag_name field
    pub tag_name: String,
    /// name field
    pub name: Option<String>,
    /// body field
    pub body: Option<String>,
}

/// GitHubPublicKey structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubPublicKey {
    /// key_id field
    pub key_id: String,
    /// key field
    pub key: String,
}

/// AccessTokenResponse structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AccessTokenResponse {
    /// access_token field
    pub access_token: String,
    /// token_type field
    pub token_type: String,
    /// scope field
    pub scope: String,
}
