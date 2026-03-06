use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_server")]
    pub server: String,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            server: default_server(),
            profile: default_profile(),
            output: default_output(),
            timeout: default_timeout(),
            profiles: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub pcode: Option<i64>,
    pub server: Option<String>,
    pub sourcemaps: Option<SourcemapConfig>,
    pub proguard: Option<ProguardConfig>,
    pub dsym: Option<DsymConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourcemapConfig {
    pub host: Option<String>,
    pub include: Option<String>,
    pub exclude: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProguardConfig {
    pub mapping_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DsymConfig {
    pub path: Option<String>,
}

/// Resolved configuration from all sources
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub server: String,
    pub profile: String,
    pub pcode: Option<i64>,
    pub output: String,
    pub timeout: u64,
    pub json: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub no_color: bool,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            server: default_server(),
            profile: default_profile(),
            pcode: None,
            output: default_output(),
            timeout: default_timeout(),
            json: false,
            quiet: false,
            verbose: false,
            no_color: false,
        }
    }
}

fn default_server() -> String {
    "https://service.whatap.io".to_string()
}

fn default_profile() -> String {
    "default".to_string()
}

fn default_output() -> String {
    "table".to_string()
}

fn default_timeout() -> u64 {
    30000
}
