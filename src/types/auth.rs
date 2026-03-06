use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub auth_mode: AuthMode,
    /// Session cookies (email/password mode)
    pub session: Option<SessionData>,
    /// API key (project-scoped mode)
    pub api_key: Option<String>,
    /// Project code (required for API key mode)
    pub pcode: Option<i64>,
    /// Server URL
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    EmailPassword,
    ApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub whatap_cookie: String,
    pub jsessionid: String,
    pub api_token: String,
    pub email: String,
    /// wa cookie for yard API (MXQL) access
    #[serde(default)]
    pub wa_cookie: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LoginResponse {
    pub result: Option<bool>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ApiTokenResponse {
    pub token: Option<String>,
    #[serde(rename = "accountEmail")]
    pub account_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiInfo {
    pub email: String,
    pub auth_mode: String,
    pub server: String,
    pub profile: String,
    pub pcode: Option<i64>,
}
