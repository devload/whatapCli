use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(rename = "projectCode")]
    pub project_code: i64,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "productType", default)]
    pub product_type: Option<String>,
    #[serde(rename = "platform", default)]
    pub platform: Option<String>,
    #[serde(rename = "status", default)]
    pub status: Option<String>,
    #[serde(rename = "licenseKey", default)]
    pub license_key: Option<String>,
    #[serde(rename = "apiToken", default)]
    pub api_token: Option<String>,
    #[serde(rename = "createTime", default)]
    pub create_time: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectListResponse {
    #[serde(default)]
    pub data: Vec<Project>,
    #[serde(rename = "accountEmail", default)]
    pub account_email: Option<String>,
    #[serde(default)]
    pub total: Option<i64>,
}
