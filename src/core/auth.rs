use anyhow::{bail, Context, Result};
use reqwest::cookie::CookieStore;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};

use crate::core::config;
use crate::core::error::CliError;
use crate::types::auth::{AuthMode, Credentials, SessionData};

const CLI_USER_AGENT: &str = "WhatapCLI/0.1.0";

/// Save credentials to disk
pub fn save_credentials(profile: &str, creds: &Credentials) -> Result<()> {
    config::ensure_dirs()?;
    let path = config::credential_path(profile)?;
    let json = serde_json::to_string_pretty(creds)?;
    std::fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Load credentials from disk
pub fn load_credentials(profile: &str) -> Result<Credentials> {
    let path = config::credential_path(profile)?;
    if !path.exists() {
        return Err(CliError::NotAuthenticated.into());
    }
    let json = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read credentials: {}", path.display()))?;
    let creds: Credentials = serde_json::from_str(&json)?;
    Ok(creds)
}

/// Remove credentials for a profile
pub fn remove_credentials(profile: &str) -> Result<bool> {
    let path = config::credential_path(profile)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Remove all credentials
pub fn remove_all_credentials() -> Result<usize> {
    let dir = config::credentials_dir()?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
            std::fs::remove_file(entry.path())?;
            count += 1;
        }
    }
    Ok(count)
}

/// Extract CSRF token from login page HTML
fn extract_csrf(html: &str) -> Option<String> {
    let csrf_idx = html.find("_csrf")?;
    let start = csrf_idx.saturating_sub(200);
    let end = std::cmp::min(csrf_idx + 200, html.len());
    let region = &html[start..end];
    let val_pos = region.find("value=\"")?;
    let val_start = val_pos + 7;
    let val_end = region[val_start..].find('"')?;
    Some(region[val_start..val_start + val_end].to_string())
}

/// Perform web login to get JSESSIONID + wa cookies (needed for yard API / MXQL)
pub async fn web_login(
    server: &str,
    email: &str,
    password: &str,
) -> Result<(String, String)> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Step 1: GET /account/login → extract CSRF token
    let login_page_url = format!("{}/account/login", server);
    let page_resp = client
        .get(&login_page_url)
        .header(USER_AGENT, CLI_USER_AGENT)
        .send()
        .await
        .context("Failed to fetch login page")?;

    let html = page_resp.text().await?;
    let csrf = extract_csrf(&html)
        .ok_or_else(|| anyhow::anyhow!("Failed to extract CSRF token from login page"))?;

    // Step 2: POST /account/login with form data (plain password works)
    let _resp = client
        .post(&format!("{}/account/login", server))
        .header(USER_AGENT, CLI_USER_AGENT)
        .form(&[
            ("email", email),
            ("password", password),
            ("_csrf", &csrf),
        ])
        .send()
        .await
        .context("Web login request failed")?;

    // Step 3: Extract cookies from the jar
    let url = reqwest::Url::parse(server)?;
    let cookie_header = jar
        .cookies(&url)
        .ok_or_else(|| anyhow::anyhow!("No cookies received from web login"))?;

    let cookie_str = cookie_header.to_str().unwrap_or("").to_string();
    let mut jsessionid = String::new();
    let mut wa_cookie = String::new();

    for part in cookie_str.split("; ") {
        if let Some(val) = part.strip_prefix("JSESSIONID=") {
            jsessionid = val.to_string();
        } else if let Some(val) = part.strip_prefix("wa=") {
            wa_cookie = val.to_string();
        }
    }

    if jsessionid.is_empty() {
        bail!("Web login failed: no JSESSIONID received (check email/password)");
    }

    Ok((jsessionid, wa_cookie))
}

/// Login with email/password via mobile API (plain password, no CAPTCHA lockout)
/// Also performs web login for yard API (MXQL) access
pub async fn login_email_password(
    server: &str,
    email: &str,
    password: &str,
) -> Result<SessionData> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Mobile API login → apiToken + whatap_cookie
    let token_url = format!("{}/mobile/api/login", server);
    let login_body = serde_json::json!({
        "email": email,
        "password": password,
        "appVersion": "1.0.0",
        "deviceInfo": "CLI",
        "deviceModel": "CLI",
        "deviceType": "CLI",
        "fcmToken": "CLI",
        "mobileDeviceToken": "",
        "osVersion": std::env::consts::OS
    });

    let resp = client
        .post(&token_url)
        .header(USER_AGENT, CLI_USER_AGENT)
        .json(&login_body)
        .send()
        .await
        .context("Failed to connect to server")?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        if let Ok(err_data) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(msg) = err_data["msg"].as_str() {
                bail!("Login failed: {}", msg);
            }
        }
        bail!("Login failed (HTTP {}): invalid email or password", status);
    }

    let data: serde_json::Value =
        serde_json::from_str(&body).context("Failed to parse login response")?;

    let api_token = data["apiToken"]
        .as_str()
        .context("No API token in response")?
        .to_string();

    let whatap_cookie = data["cookie"].as_str().unwrap_or("").to_string();

    // Web login → JSESSIONID + wa cookie (for yard API / MXQL)
    let (jsessionid, wa_cookie) = match web_login(server, email, password).await {
        Ok(cookies) => cookies,
        Err(e) => {
            eprintln!(
                "{} Web session setup failed (MXQL will be unavailable): {}",
                colored::Colorize::yellow("!"),
                e
            );
            (String::new(), String::new())
        }
    };

    Ok(SessionData {
        whatap_cookie,
        jsessionid,
        api_token,
        email: email.to_string(),
        wa_cookie,
    })
}

/// Build auth headers for API requests
pub fn build_auth_headers(creds: &Credentials) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    match creds.auth_mode {
        AuthMode::ApiKey => {
            let api_key = creds
                .api_key
                .as_ref()
                .ok_or(CliError::NotAuthenticated)?;
            headers.insert(
                "X-WhaTap-Token",
                HeaderValue::from_str(api_key)?,
            );
            if let Some(pcode) = creds.pcode {
                headers.insert(
                    "X-WhaTap-Pcode",
                    HeaderValue::from_str(&pcode.to_string())?,
                );
            }
        }
        AuthMode::EmailPassword => {
            let session = creds
                .session
                .as_ref()
                .ok_or(CliError::NotAuthenticated)?;
            headers.insert(
                "x-whatap-token",
                HeaderValue::from_str(&session.api_token)?,
            );
            if !session.whatap_cookie.is_empty() {
                let cookie = format!("WHATAP={}", session.whatap_cookie);
                headers.insert(COOKIE, HeaderValue::from_str(&cookie)?);
            }
        }
    }
    Ok(headers)
}
