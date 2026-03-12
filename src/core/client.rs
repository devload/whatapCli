use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};

use crate::core::auth;
use crate::core::error::CliError;
use crate::types::auth::Credentials;
use crate::types::config::ResolvedConfig;
use crate::types::project::Project;

pub struct WhatapClient {
    http: reqwest::Client,
    pub config: ResolvedConfig,
    pub creds: Option<Credentials>,
}

impl WhatapClient {
    pub fn new(config: ResolvedConfig) -> Result<Self> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            USER_AGENT,
            HeaderValue::from_static("WhatapCLI/0.1.0"),
        );

        let http = reqwest::Client::builder()
            .default_headers(default_headers)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        // Try to load credentials
        let creds = auth::load_credentials(&config.profile).ok();

        Ok(Self { http, config, creds })
    }

    /// Get authenticated headers
    fn auth_headers(&self, pcode: Option<i64>) -> Result<HeaderMap> {
        let creds = self
            .creds
            .as_ref()
            .ok_or(CliError::NotAuthenticated)?;
        auth::build_auth_headers(creds, pcode)
    }

    /// Get the server URL (from creds if available, otherwise config)
    pub fn server(&self) -> &str {
        self.creds
            .as_ref()
            .and_then(|c| c.server.as_deref())
            .unwrap_or(&self.config.server)
    }

    /// Resolve pcode from CLI flag > env > project config > credentials
    pub fn resolve_pcode(&self, pcode_override: Option<i64>) -> Result<i64> {
        pcode_override
            .or(self.config.pcode)
            .or_else(|| self.creds.as_ref().and_then(|c| c.pcode))
            .ok_or_else(|| {
                CliError::Config(
                    "No project code (pcode) specified. Use --pcode, WHATAP_PCODE env var, or .whataprc.yml".to_string()
                ).into()
            })
    }

    /// GET request with authentication
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.get_with_pcode(path, None).await
    }

    /// GET request with authentication and optional pcode
    pub async fn get_with_pcode(&self, path: &str, pcode: Option<i64>) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.server(), path);
        let headers = self.auth_headers(pcode)?;
        let resp = self
            .http
            .get(&url)
            .headers(headers)
            .send()
            .await
            .with_context(|| format!("Request failed: GET {}", url))?;

        Self::check_response(resp).await
    }

    /// POST request with JSON body
    pub async fn post_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.server(), path);
        let headers = self.auth_headers(None)?;
        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .json(body)
            .send()
            .await
            .with_context(|| format!("Request failed: POST {}", url))?;

        Self::check_response(resp).await
    }

    /// POST multipart form
    pub async fn post_multipart(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.server(), path);
        let headers = self.auth_headers(None)?;
        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Request failed: POST {}", url))?;

        Self::check_response(resp).await
    }

    /// POST form-urlencoded
    pub async fn post_form(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.server(), path);
        let headers = self.auth_headers(None)?;
        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .form(params)
            .send()
            .await
            .with_context(|| format!("Request failed: POST {}", url))?;

        Self::check_response(resp).await
    }

    /// Check response status and return error if not OK
    async fn check_response(resp: reqwest::Response) -> Result<reqwest::Response> {
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(CliError::SessionExpired.into());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CliError::Api {
                status: status.as_u16(),
                message: body,
            }
            .into());
        }
        Ok(resp)
    }

    /// List all accessible projects
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let resp = self.get("/open/api/json/projects").await?;
        let body = resp.text().await?;

        // Try parsing as array first, then as object with data field
        if let Ok(projects) = serde_json::from_str::<Vec<Project>>(&body) {
            return Ok(projects);
        }
        if let Ok(wrapper) = serde_json::from_str::<crate::types::project::ProjectListResponse>(&body) {
            return Ok(wrapper.data);
        }

        // Try parsing the raw JSON and extracting project info
        let value: serde_json::Value = serde_json::from_str(&body)?;
        if let Some(arr) = value.as_array() {
            let mut projects = Vec::new();
            for item in arr {
                if let Ok(p) = serde_json::from_value::<Project>(item.clone()) {
                    projects.push(p);
                }
            }
            return Ok(projects);
        }

        Ok(vec![])
    }

    /// Get project detail
    pub async fn project_info(&self, pcode: i64) -> Result<serde_json::Value> {
        let resp = self.get(&format!("/api/project/{}", pcode)).await?;
        let body = resp.text().await?;
        let value: serde_json::Value = serde_json::from_str(&body)?;
        Ok(value)
    }

    /// Build web session cookie string (JSESSIONID + wa)
    fn web_cookie(&self) -> Result<String> {
        let creds = self
            .creds
            .as_ref()
            .ok_or(CliError::NotAuthenticated)?;
        let session = creds
            .session
            .as_ref()
            .ok_or(CliError::NotAuthenticated)?;

        if session.jsessionid.is_empty() {
            bail!(
                "Web session required. Re-login with: whatap login -e <email> -p <password>"
            );
        }

        let mut cookie = format!("JSESSIONID={}", session.jsessionid);
        if !session.wa_cookie.is_empty() {
            cookie.push_str(&format!("; wa={}", session.wa_cookie));
        }
        Ok(cookie)
    }

    /// GET request with web session cookies
    pub async fn web_get(&self, path: &str) -> Result<serde_json::Value> {
        let cookie = self.web_cookie()?;
        let url = format!("{}{}", self.server(), path);

        let resp = self
            .http
            .get(&url)
            .header(COOKIE, &cookie)
            .header(USER_AGENT, "WhatapCLI/0.1.0")
            .send()
            .await
            .with_context(|| format!("Request failed: GET {}", url))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            bail!("Web session expired. Re-login with: whatap login -e <email> -p <password>");
        }

        let body_text = resp.text().await?;
        if !status.is_success() {
            bail!("API error ({}): {}", status, body_text);
        }

        let value: serde_json::Value =
            serde_json::from_str(&body_text).context("Failed to parse response")?;
        Ok(value)
    }

    /// POST JSON with web session cookies
    pub async fn web_post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let cookie = self.web_cookie()?;
        let url = format!("{}{}", self.server(), path);

        let resp = self
            .http
            .post(&url)
            .header(COOKIE, &cookie)
            .header(USER_AGENT, "WhatapCLI/0.1.0")
            .json(body)
            .send()
            .await
            .with_context(|| format!("Request failed: POST {}", url))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            bail!("Web session expired. Re-login with: whatap login -e <email> -p <password>");
        }

        let body_text = resp.text().await?;
        if !status.is_success() {
            bail!("API error ({}): {}", status, body_text);
        }

        let value: serde_json::Value =
            serde_json::from_str(&body_text).context("Failed to parse response")?;
        Ok(value)
    }

    /// POST to yard API with web session cookies
    pub async fn yard_post(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        let cookie = self.web_cookie()?;
        let url = format!("{}/yard/api/flush", self.server());

        let resp = self
            .http
            .post(&url)
            .header(COOKIE, &cookie)
            .header(USER_AGENT, "WhatapCLI/0.1.0")
            .json(body)
            .send()
            .await
            .with_context(|| "Yard API request failed")?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            bail!("Web session expired. Re-login with: whatap login -e <email> -p <password>");
        }

        let body_text = resp.text().await?;
        if !status.is_success() {
            bail!("Yard API error ({}): {}", status, body_text);
        }

        let value: serde_json::Value =
            serde_json::from_str(&body_text).context("Failed to parse yard API response")?;
        Ok(value)
    }
}
