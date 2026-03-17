use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::types::config::{GlobalConfig, ProjectConfig, ResolvedConfig};

/// Get the WhatAp config directory (~/.whatap/)
pub fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".whatap"))
}

/// Get the credentials directory (~/.whatap/credentials/)
pub fn credentials_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("credentials"))
}

/// Ensure config directories exist
pub fn ensure_dirs() -> Result<()> {
    let config = config_dir()?;
    std::fs::create_dir_all(&config)?;
    std::fs::create_dir_all(config.join("credentials"))?;
    Ok(())
}

/// Load global config from ~/.whatap/config.yml
pub fn load_global_config() -> Result<GlobalConfig> {
    let path = config_dir()?.join("config.yml");
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: GlobalConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

/// Save global config to ~/.whatap/config.yml
pub fn save_global_config(config: &GlobalConfig) -> Result<()> {
    ensure_dirs()?;
    let path = config_dir()?.join("config.yml");
    let content = serde_yaml::to_string(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Load project config from .whataprc.yml in current or parent directories
pub fn load_project_config() -> Result<Option<ProjectConfig>> {
    let mut dir = std::env::current_dir()?;
    loop {
        let rc = dir.join(".whataprc.yml");
        if rc.exists() {
            let content = std::fs::read_to_string(&rc)?;
            let config: ProjectConfig = serde_yaml::from_str(&content)?;
            return Ok(Some(config));
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(None)
}

/// Resolve configuration from all sources (CLI flags > env vars > project > global > defaults)
pub fn resolve_config(
    profile: &str,
    server_override: Option<&str>,
    json: bool,
    markdown: bool,
    quiet: bool,
    verbose: bool,
    no_color: bool,
) -> Result<ResolvedConfig> {
    let global = load_global_config().unwrap_or_default();
    let project = load_project_config().ok().flatten();

    // Resolve server URL
    let server = server_override
        .map(String::from)
        .or_else(|| std::env::var("WHATAP_SERVER").ok())
        .or_else(|| {
            project.as_ref().and_then(|p| p.server.clone())
        })
        .or_else(|| {
            global
                .profiles
                .get(profile)
                .and_then(|p| p.server.clone())
        })
        .unwrap_or(global.server);

    // Resolve pcode
    let pcode = std::env::var("WHATAP_PCODE")
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| project.as_ref().and_then(|p| p.pcode));

    // CI environment detection
    let is_ci = std::env::var("CI").is_ok();
    let quiet = quiet || is_ci || std::env::var("WHATAP_QUIET").map(|v| v == "1").unwrap_or(false);

    Ok(ResolvedConfig {
        server,
        profile: profile.to_string(),
        pcode,
        output: if json {
            "json".to_string()
        } else if markdown {
            "markdown".to_string()
        } else {
            global.output
        },
        timeout: global.timeout,
        json,
        markdown,
        quiet,
        verbose,
        no_color,
    })
}

/// Get the path for a credential file
pub fn credential_path(profile: &str) -> Result<PathBuf> {
    Ok(credentials_dir()?.join(format!("{}.json", profile)))
}
