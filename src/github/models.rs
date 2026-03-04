use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubOrg {
    pub id: i64,
    pub login: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRelease {
    pub id: i64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubPublicKey {
    pub key_id: String,
    pub key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}
