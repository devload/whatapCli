use anyhow::{bail, Context, Result};
use tabled::Tabled;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

#[derive(Tabled, serde::Serialize)]
struct ProjectRow {
    #[tabled(rename = "Code")]
    code: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Platform")]
    platform: String,
    #[tabled(rename = "Status")]
    status: String,
}

/// Platform type mapping
/// Returns (productType, platform_api_value, display_name)
fn resolve_platform(platform: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match platform.to_lowercase().as_str() {
        "java" => Ok(("APM", "JAVA", "Java")),
        "nodejs" | "node" => Ok(("APM", "NODEJS", "Node.js")),
        "python" => Ok(("APM", "PYTHON", "Python")),
        "php" => Ok(("APM", "PHP", "PHP")),
        "dotnet" | ".net" => Ok(("APM", "DOTNET", ".NET")),
        "go" | "golang" => Ok(("APM", "GO", "Go")),
        "kubernetes" | "k8s" => Ok(("INFRA", "KUBERNETES", "Kubernetes")),
        "server" | "linux" | "infra" => Ok(("INFRA", "INFRA", "Server/Infra")),
        "browser" => Ok(("BROWSER", "BROWSER", "Browser")),
        "android" => Ok(("MOBILE", "ANDROID", "Android")),
        "ios" => Ok(("MOBILE", "IOS", "iOS")),
        _ => bail!(
            "Unknown platform: '{}'. Supported: java, nodejs, python, php, dotnet, go, kubernetes, server, browser, android, ios",
            platform
        ),
    }
}

pub async fn create(
    config: &ResolvedConfig,
    name: String,
    platform: String,
    group_id: Option<i64>,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;

    let (product_type, platform_api, platform_display) = resolve_platform(&platform)?;

    output::info(
        &format!("Creating {} project '{}' ...", platform_display, name),
        config.quiet,
    );

    // Step 1: Get data center regions (returns JSON array)
    let regions = client
        .web_get("/account/region")
        .await
        .context("Failed to fetch regions")?;

    let regions_arr = regions
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected regions format"))?;

    if regions_arr.is_empty() {
        bail!("No regions available");
    }

    // Find the first suitable region (prefer AWS-Seoul, fallback to first)
    let region_data = regions_arr
        .iter()
        .find(|r| {
            r.get("textKey")
                .and_then(|v| v.as_str())
                .map(|k| k.contains("Seoul"))
                .unwrap_or(false)
        })
        .unwrap_or(&regions_arr[0]);

    let region_key = region_data
        .get("textKey")
        .and_then(|v| v.as_str())
        .unwrap_or("AWS-Tokyo");

    let proxy = region_data
        .get("proxyAddress")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Step 2: Get create project token
    let token_resp = client
        .web_get("/management/api/v2/create/project/token")
        .await
        .context("Failed to get project creation token")?;

    let token = token_resp
        .get("data")
        .and_then(|v| v.as_str())
        .or_else(|| token_resp.get("token").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No token in response: {}",
                serde_json::to_string(&token_resp).unwrap_or_default()
            )
        })?;

    // Step 3: Create the project
    // API expects: name, platform (uppercase), productType, regionName (textKey),
    // tempToken, timezoneOffset, groupId
    // timezone offset in minutes (e.g., KST = -540)
    let timezone_offset: i64 = -540; // Default KST; can be made dynamic later

    let mut create_body = serde_json::json!({
        "name": name,
        "productType": product_type,
        "platform": platform_api,
        "regionName": region_key,
        "tempToken": token,
        "timezoneOffset": timezone_offset,
    });

    if let Some(gid) = group_id {
        create_body["groupId"] = serde_json::json!(gid.to_string());
    }

    let project_resp = client
        .web_post_json("/project/api/v4/create/project", &create_body)
        .await
        .context("Failed to create project")?;

    // Response format: { "code": 200, "data": { "projectCode": ..., ... }, "msg": "success" }
    let project_data = project_resp
        .get("data")
        .unwrap_or(&project_resp);

    let pcode = project_data
        .get("projectCode")
        .and_then(|v| v.as_i64())
        .or_else(|| project_data.get("pcode").and_then(|v| v.as_i64()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No project code in response: {}",
                serde_json::to_string(&project_resp).unwrap_or_default()
            )
        })?;

    // Try to get proxy from the create response region data
    let resp_proxy = project_data
        .get("region")
        .and_then(|r| r.get("proxyAddress"))
        .and_then(|v| v.as_str())
        .unwrap_or(proxy);

    // Step 4: Issue license key
    let license_resp = client
        .web_post_json(
            &format!("/project/api/v4/{}/license", pcode),
            &serde_json::json!({}),
        )
        .await
        .context("Failed to issue license key")?;

    // License response: { "data": "license-key-string", ... }
    // or: { "data": { "licenseKey": "..." }, ... }
    let license = license_resp
        .get("data")
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.get("licenseKey").and_then(|k| k.as_str()).map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "-".to_string());

    let server_host = if !resp_proxy.is_empty() {
        resp_proxy.to_string()
    } else {
        "-".to_string()
    };

    // Output result
    if config.output == "json" {
        let result = serde_json::json!({
            "pcode": pcode,
            "projectName": name,
            "productType": product_type,
            "platform": platform_display,
            "licenseKey": license,
            "serverHost": server_host,
            "dataRegion": region_key,
        });
        output::print_value(&result, &config.output);
    } else {
        output::success(&format!("Project created: {} (pcode: {})", name, pcode));
        println!();
        println!("  Project Code:  {}", pcode);
        println!("  Platform:      {}", platform_display);
        println!("  License Key:   {}", license);
        println!("  Server Host:   {}", server_host);
        println!("  Region:        {}", region_key);
        println!();
        println!("Add to your agent config (whatap.conf):");
        println!("  license={}", license);
        println!("  whatap.server.host={}", server_host);
    }

    Ok(())
}

pub async fn list(config: &ResolvedConfig, filter: Option<String>) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let projects = client.list_projects().await?;

    let mut rows: Vec<ProjectRow> = projects
        .iter()
        .map(|p| ProjectRow {
            code: p.project_code,
            name: p.project_name.clone(),
            platform: p
                .product_type
                .clone()
                .or_else(|| p.platform.clone())
                .unwrap_or_else(|| "-".to_string()),
            status: p
                .status
                .clone()
                .unwrap_or_else(|| "active".to_string()),
        })
        .collect();

    if let Some(f) = &filter {
        let f_upper = f.to_uppercase();
        rows.retain(|r| {
            r.platform.to_uppercase().contains(&f_upper)
                || r.name.to_uppercase().contains(&f_upper)
        });
    }

    output::info(
        &format!("Found {} project(s)", rows.len()),
        config.quiet,
    );
    output::print_output(&rows, &config.output);

    Ok(())
}

pub async fn delete(config: &ResolvedConfig, pcode: i64, confirm: bool) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;

    // Show project info before deleting
    if !confirm {
        // Look up project name
        let projects = client.list_projects().await?;
        let project_name = projects
            .iter()
            .find(|p| p.project_code == pcode)
            .map(|p| p.project_name.as_str())
            .unwrap_or("Unknown");

        eprintln!(
            "About to delete project: {} (pcode: {})",
            project_name, pcode
        );
        eprint!("Are you sure? (y/N): ");
        use std::io::Write;
        std::io::stderr().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            output::info("Cancelled.", config.quiet);
            return Ok(());
        }
    }

    // POST /project/api/v4/{pcode}/delete
    client
        .web_post_json(
            &format!("/project/api/v4/{}/delete", pcode),
            &serde_json::json!({}),
        )
        .await
        .context("Failed to delete project")?;

    if config.output == "json" {
        let result = serde_json::json!({
            "deleted": true,
            "pcode": pcode,
        });
        output::print_value(&result, &config.output);
    } else {
        output::success(&format!("Project {} deleted", pcode));
    }

    Ok(())
}
