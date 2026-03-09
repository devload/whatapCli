use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Get current epoch milliseconds
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Build MXQL query for log search
fn build_log_mxql(
    category: &str,
    keyword: Option<&str>,
    level: Option<&str>,
    select_fields: Option<&str>,
    limit: u64,
) -> String {
    let mut mql = format!("CATEGORY {}\nTAGLOAD", category);

    // SELECT fields
    if let Some(fields) = select_fields {
        mql.push_str(&format!("\nSELECT [{}]", fields));
    } else {
        // Default fields by category
        let default_fields = match category {
            "app_log" => "[oid, oname, @message, @level, @timestamp]",
            "log" => "[oid, oname, @message, @category, @timestamp]",
            "browser_error" => "[errorType, errorMessage, pageUrl, userAgent, @timestamp]",
            "mobile_crash" => "[crashType, crashMessage, device, osVersion, @timestamp]",
            _ => "[*]",
        };
        mql.push_str(&format!("\nSELECT {}", default_fields));
    }

    // FILTER
    let mut filters = Vec::new();
    if let Some(kw) = keyword {
        // Escape single quotes in keyword
        let escaped = kw.replace('\'', "\\'");
        filters.push(format!("message like '%{}%'", escaped));
    }
    if let Some(lvl) = level {
        filters.push(format!("level == '{}'", lvl.to_uppercase()));
    }

    if !filters.is_empty() {
        mql.push_str(&format!("\nFILTER {{{}}}", filters.join(" && ")));
    }

    mql.push_str(&format!("\nLIMIT {}", limit));
    mql
}

/// Build the yard API request body for MXQL log query
fn build_yard_request(pcode: i64, mql: &str, stime: u64, etime: u64) -> serde_json::Value {
    serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 500,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    })
}

/// Search application logs via MXQL
pub async fn search(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    keyword: Option<String>,
    level: Option<String>,
    category: Option<String>,
    fields: Option<String>,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    limit: u64,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    let cat = category.as_deref().unwrap_or("app_log");

    // Build MXQL query
    let mql = build_log_mxql(
        cat,
        keyword.as_deref(),
        level.as_deref(),
        fields.as_deref(),
        limit,
    );

    // Resolve time range
    let now = now_millis();
    let (resolved_stime, resolved_etime) = if let Some(dur_str) = &duration {
        let dur_ms = parse_duration_ms(dur_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid duration '{}'. Use: 5m, 1h, 30s, 1d", dur_str))?;
        let e = etime.unwrap_or(now);
        let s = stime.unwrap_or(e - dur_ms);
        (s, e)
    } else {
        let e = etime.unwrap_or(now);
        let s = stime.unwrap_or(e - 3600 * 1000); // default: last 1 hour
        (s, e)
    };

    if config.verbose {
        eprintln!("Log category: {}", cat);
        eprintln!("MXQL:\n{}", mql);
        eprintln!("Time range: {} ~ {}", resolved_stime, resolved_etime);
    }

    // Execute via yard API
    let request_body = build_yard_request(resolved_pcode, &mql, resolved_stime, resolved_etime);
    let result = client.yard_post(&request_body).await?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Parse and display results
    let records = if let Some(arr) = result.get("data").and_then(|d| d.as_array()) {
        arr.clone()
    } else if let Some(arr) = result.as_array() {
        arr.clone()
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    };

    if records.is_empty() {
        output::warn("No log entries found");
        return Ok(());
    }

    output::info(&format!("{} log entries (category: {})", records.len(), cat), config.quiet);

    // Display as formatted log entries
    for record in &records {
        let timestamp = record.get("@timestamp")
            .or_else(|| record.get("timestamp"))
            .or_else(|| record.get("time"))
            .and_then(|t| t.as_u64())
            .map(format_time)
            .unwrap_or_else(|| "-".to_string());

        let level_str = record.get("@level")
            .or_else(|| record.get("level"))
            .and_then(|v| v.as_str())
            .unwrap_or("-");

        let message = record.get("@message")
            .or_else(|| record.get("message"))
            .or_else(|| record.get("errorMessage"))
            .or_else(|| record.get("crashMessage"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let agent = record.get("oname")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let level_colored = match level_str.to_uppercase().as_str() {
            "ERROR" | "FATAL" => format!("[{}]", level_str),
            "WARN" | "WARNING" => format!("[{}]", level_str),
            _ => format!("[{}]", level_str),
        };

        if agent.is_empty() {
            println!("{} {} {}", timestamp, level_colored, message);
        } else {
            println!("{} {} ({}) {}", timestamp, level_colored, agent, message);
        }
    }

    Ok(())
}

/// List available log categories
pub async fn categories(config: &ResolvedConfig) -> Result<()> {
    let cats = vec![
        LogCategory { category: "app_log".into(), description: "Application logs (Java, Node.js, Python, etc.)".into() },
        LogCategory { category: "log".into(), description: "Generic log entries".into() },
        LogCategory { category: "browser_error".into(), description: "Browser JavaScript errors".into() },
        LogCategory { category: "mobile_crash".into(), description: "Mobile app crash logs".into() },
        LogCategory { category: "mobile_exception".into(), description: "Mobile handled exceptions".into() },
        LogCategory { category: "server_log".into(), description: "Server infrastructure logs".into() },
        LogCategory { category: "db_log".into(), description: "Database logs".into() },
    ];

    if config.json {
        println!("{}", serde_json::to_string_pretty(&cats)?);
    } else {
        output::info("Available log categories", config.quiet);
        output::print_output(&cats, &config.output);
    }

    Ok(())
}

fn parse_duration_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else {
        let last = s.chars().last()?;
        if last.is_alphabetic() {
            (&s[..s.len() - 1], &s[s.len() - 1..])
        } else {
            (s, "ms")
        }
    };
    let num: u64 = num_str.parse().ok()?;
    let ms = match unit {
        "ms" => num,
        "s" => num * 1000,
        "m" => num * 60 * 1000,
        "h" => num * 3600 * 1000,
        "d" => num * 86400 * 1000,
        _ => return None,
    };
    Some(ms)
}

fn format_time(epoch_ms: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_millis(epoch_ms);
    let dt: chrono::DateTime<chrono::Local> = d.into();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Serialize, Tabled)]
struct LogCategory {
    #[tabled(rename = "Category")]
    category: String,
    #[tabled(rename = "Description")]
    description: String,
}
