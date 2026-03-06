use anyhow::{bail, Context, Result};

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Generate a 32-char hex event ID
fn generate_event_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let nanos = now.as_nanos();
    let pid = std::process::id() as u128;
    format!("{:032x}", nanos.wrapping_mul(pid | 1))
}

/// List metric alerts for a project
pub async fn list(config: &ResolvedConfig, pcode: Option<i64>) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let result = client
        .web_get(&format!(
            "/project/api/pcode/{}/event/metrics?page=0&pageSize=200",
            pcode
        ))
        .await?;

    // Response: { ok, code, msg, data: { data: [...], total, page, pageSize } }
    let alerts = result
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_array());

    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let alerts = match alerts {
        Some(a) if !a.is_empty() => a,
        _ => {
            output::info("No metric alerts found.", config.quiet);
            return Ok(());
        }
    };

    output::info(
        &format!("Found {} alert(s) for project {}", alerts.len(), pcode),
        config.quiet,
    );
    println!();

    // Print table
    println!(
        "{:<34} {:<7} {:<10} {:<20} {}",
        "EVENT ID", "ENABLED", "STATEFUL", "CATEGORY", "TITLE"
    );
    println!("{}", "-".repeat(100));

    for alert in alerts {
        let event_id = alert["eventId"].as_str().unwrap_or("-");
        let enabled = alert["enabled"].as_bool().unwrap_or(false);
        let stateful = alert["stateful"].as_bool().unwrap_or(false);
        let category = alert["category"].as_str().unwrap_or("-");
        let title = alert["title"].as_str().unwrap_or("-");

        println!(
            "{:<34} {:<7} {:<10} {:<20} {}",
            event_id,
            if enabled { "on" } else { "off" },
            if stateful { "yes" } else { "no" },
            truncate(category, 18),
            title
        );
    }

    Ok(())
}

/// Create a metric alert
pub async fn create(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    title: Option<String>,
    category: Option<String>,
    warning: Option<String>,
    critical: Option<String>,
    info_rule: Option<String>,
    message: Option<String>,
    stateful: bool,
    select: Option<String>,
    repeat_count: u64,
    repeat_duration: u64,
    silent: u64,
    input_json: Option<String>,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let params = if let Some(ref json_str) = input_json {
        // MCP mode: full alert as JSON
        let mut params: serde_json::Value =
            serde_json::from_str(json_str).context("Invalid --input-json")?;

        // Ensure required fields
        if params.get("eventId").is_none() {
            params["eventId"] = serde_json::json!(generate_event_id());
        }
        if params.get("version").is_none() {
            params["version"] = serde_json::json!(1);
        }
        if params.get("basic").is_none() {
            params["basic"] = serde_json::json!(false);
        }
        params
    } else {
        // CLI mode: build from flags
        let title = title.ok_or_else(|| anyhow::anyhow!("--title is required"))?;
        let category = category.ok_or_else(|| anyhow::anyhow!("--category is required"))?;

        // Build conditions
        let mut conditions = Vec::new();
        if let Some(ref rule) = info_rule {
            conditions.push(serde_json::json!({
                "level": 10, "enabled": true, "rule": rule
            }));
        }
        if let Some(ref rule) = warning {
            conditions.push(serde_json::json!({
                "level": 20, "enabled": true, "rule": rule
            }));
        }
        if let Some(ref rule) = critical {
            conditions.push(serde_json::json!({
                "level": 30, "enabled": true, "rule": rule
            }));
        }

        if conditions.is_empty() {
            bail!("At least one condition required: --warning, --critical, or --info");
        }

        let msg = message.unwrap_or_else(|| format!("{} alert triggered", title));

        serde_json::json!({
            "eventId": generate_event_id(),
            "version": 1,
            "basic": false,
            "enabled": true,
            "stateful": stateful,
            "title": title,
            "message": msg,
            "category": category,
            "alertLabel": [],
            "selectString": select.unwrap_or_default(),
            "repeatCount": repeat_count,
            "repeatDuration": repeat_duration,
            "silent": silent,
            "receiver": [],
            "timeTag": [],
            "conditions": conditions
        })
    };

    let event_id = params["eventId"].as_str().unwrap_or("").to_string();

    let body = serde_json::json!({
        "type": "event/v2",
        "path": "/metrics/create",
        "pcode": pcode,
        "params": params
    });

    if config.verbose {
        eprintln!("Request: {}", serde_json::to_string_pretty(&body)?);
    }

    let result = client.yard_post(&body).await?;

    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let title = params["title"].as_str().unwrap_or("-");
        output::success(&format!(
            "Alert created: {} (id: {})",
            title,
            &event_id[..8.min(event_id.len())]
        ));
    }

    Ok(())
}

/// Delete a metric alert
pub async fn delete(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    event_id: &str,
    category: Option<String>,
    confirm: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    // Resolve category: use provided or fetch from alert list
    let cat = if let Some(c) = category {
        c
    } else {
        let list_result = client
            .web_get(&format!(
                "/project/api/pcode/{}/event/metrics?page=0&pageSize=500",
                pcode
            ))
            .await?;
        let alerts = list_result
            .get("data")
            .and_then(|d| d.get("data"))
            .and_then(|d| d.as_array());
        alerts
            .and_then(|a| {
                a.iter().find(|item| item["eventId"].as_str() == Some(event_id))
            })
            .and_then(|a| a["category"].as_str())
            .unwrap_or("")
            .to_string()
    };

    if !confirm {
        eprint!(
            "Delete alert {} from project {}? [y/N] ",
            event_id, pcode
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            output::info("Cancelled.", false);
            return Ok(());
        }
    }

    let body = serde_json::json!({
        "type": "event/v2",
        "path": format!("/metrics/delete/{}", event_id),
        "pcode": pcode,
        "params": {
            "category": cat,
            "basic": false
        }
    });

    let result = client.yard_post(&body).await?;

    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        output::success(&format!("Alert {} deleted", event_id));
    }

    Ok(())
}

/// Enable or disable an alert (update enabled field)
pub async fn toggle(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    event_id: &str,
    enable: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    // Fetch all alerts and find the one to update
    let list_result = client
        .web_get(&format!(
            "/project/api/pcode/{}/event/metrics?page=0&pageSize=500",
            pcode
        ))
        .await?;

    let alerts = list_result
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_array());

    let alert = alerts
        .and_then(|a| {
            a.iter().find(|item| {
                item["eventId"].as_str() == Some(event_id)
            })
        })
        .ok_or_else(|| anyhow::anyhow!("Alert {} not found", event_id))?;

    let mut params = alert.clone();
    params["enabled"] = serde_json::json!(enable);

    let body = serde_json::json!({
        "type": "event/v2",
        "path": "/metrics/update",
        "pcode": pcode,
        "params": params
    });

    let result = client.yard_post(&body).await?;

    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let title = alert["title"].as_str().unwrap_or(event_id);
        let action = if enable { "enabled" } else { "disabled" };
        output::success(&format!("Alert '{}' {}", title, action));
    }

    Ok(())
}

/// Export alerts to JSON
pub async fn export(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    file: Option<String>,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let result = client
        .web_get(&format!(
            "/project/api/pcode/{}/event/metrics?page=0&pageSize=500",
            pcode
        ))
        .await?;

    let alerts = result
        .get("data")
        .and_then(|d| d.get("data"))
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let json = serde_json::to_string_pretty(&alerts)?;

    if let Some(path) = file {
        std::fs::write(&path, &json)?;
        let count = alerts.as_array().map(|a| a.len()).unwrap_or(0);
        output::success(&format!("Exported {} alert(s) to {}", count, path));
    } else {
        println!("{}", json);
    }

    Ok(())
}

/// Import alerts from JSON file
pub async fn import(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    file: &str,
    overwrite: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let json = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file))?;
    let alerts: serde_json::Value =
        serde_json::from_str(&json).context("Invalid JSON in alert file")?;

    let list = if alerts.is_array() {
        alerts
    } else {
        bail!("Alert file must contain a JSON array of alert objects");
    };

    let count = list.as_array().map(|a| a.len()).unwrap_or(0);
    let path = if overwrite {
        "/metrics/overwrite"
    } else {
        "/metrics/import"
    };

    let body = serde_json::json!({
        "type": "event/v2",
        "path": path,
        "pcode": pcode,
        "params": {
            "list": list
        }
    });

    let result = client.yard_post(&body).await?;

    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let mode = if overwrite { "overwritten" } else { "imported" };
        output::success(&format!("{} {} alert(s) to project {}", mode, count, pcode));
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max - 3])
    } else {
        s.to_string()
    }
}
