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

/// Parse human-readable duration to milliseconds (e.g. "5m", "1h", "30s", "1d")
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

/// Format epoch ms to human-readable time
fn format_time(epoch_ms: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_millis(epoch_ms);
    let dt: chrono::DateTime<chrono::Local> = d.into();
    dt.format("%H:%M:%S").to_string()
}

/// Fetch time-series metric statistics via Open API tag endpoint
pub async fn run(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    category: String,
    field: String,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

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

    // Build Open API tag endpoint URL
    let path = format!(
        "/open/api/json/tag/{}/{}?stime={}&etime={}",
        category, field, resolved_stime, resolved_etime
    );

    if config.verbose {
        eprintln!("Stat query: {}/{}", category, field);
        eprintln!("Time range: {} ~ {} ({} ms)", resolved_stime, resolved_etime, resolved_etime - resolved_stime);
    }

    let resp = client.get_with_pcode(&path, pcode).await?;
    let body = resp.text().await?;
    let data: serde_json::Value = serde_json::from_str(&body)?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    // Parse response - typical format: {"pcode":..., "type":"tag", "data":[...]}
    let records = if let Some(arr) = data.get("data").and_then(|d| d.as_array()) {
        arr.clone()
    } else if let Some(arr) = data.as_array() {
        arr.clone()
    } else {
        // Single value or flat object
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    };

    if records.is_empty() {
        output::warn("No stat data returned for this time range");
        return Ok(());
    }

    // Build table rows from time-series data
    let mut rows: Vec<StatRow> = Vec::new();
    for record in &records {
        let time = record.get("time")
            .or_else(|| record.get("stime"))
            .and_then(|t| t.as_u64())
            .map(format_time)
            .unwrap_or_else(|| "-".to_string());

        // Try to find the field value
        let value = if let Some(v) = record.get(&field) {
            format_number(v)
        } else if let Some(v) = record.get("data") {
            format_number(v)
        } else if let Some(v) = record.get("value") {
            format_number(v)
        } else {
            // Collect all numeric values
            if let Some(obj) = record.as_object() {
                let vals: Vec<String> = obj.iter()
                    .filter(|(k, _)| k.as_str() != "time" && k.as_str() != "stime" && k.as_str() != "etime")
                    .map(|(k, v)| format!("{}={}", k, format_number(v)))
                    .collect();
                vals.join(", ")
            } else {
                format_number(record)
            }
        };

        // Extract oid/oname if present (per-agent breakdown)
        let agent = record.get("oname")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| record.get("oid").map(|v| v.to_string()))
            .unwrap_or_default();

        rows.push(StatRow { time, agent, value });
    }

    // If no agent column has values, use simplified output
    let has_agents = rows.iter().any(|r| !r.agent.is_empty());
    if has_agents {
        output::info(&format!("{}/{} ({} data points)", category, field, rows.len()), config.quiet);
        output::print_output(&rows, &config.output);
    } else {
        let simple_rows: Vec<SimpleStatRow> = rows.iter().map(|r| SimpleStatRow {
            time: r.time.clone(),
            value: r.value.clone(),
        }).collect();
        output::info(&format!("{}/{} ({} data points)", category, field, simple_rows.len()), config.quiet);
        output::print_output(&simple_rows, &config.output);
    }

    Ok(())
}

/// List available stat categories and their fields
pub async fn categories(config: &ResolvedConfig, pcode: Option<i64>) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    // Common categories and fields (built-in reference)
    let cats = vec![
        CategoryInfo { category: "app_counter".into(), fields: "tps, resp_time, actx, apdex, error_cnt".into(), description: "Application counters".into() },
        CategoryInfo { category: "app_user".into(), fields: "realtime_user".into(), description: "Real-time user count".into() },
        CategoryInfo { category: "app_httpc".into(), fields: "count, error, time".into(), description: "HTTP outbound calls".into() },
        CategoryInfo { category: "app_sql".into(), fields: "count, error, time, fetch".into(), description: "SQL execution stats".into() },
        CategoryInfo { category: "server_cpu".into(), fields: "cpu, load1, load5, load15".into(), description: "Server CPU metrics".into() },
        CategoryInfo { category: "server_memory".into(), fields: "memory_pused, memory_available".into(), description: "Server memory usage".into() },
        CategoryInfo { category: "server_disk".into(), fields: "disk_usage, disk_io".into(), description: "Server disk metrics".into() },
        CategoryInfo { category: "server_network".into(), fields: "traffic_in, traffic_out".into(), description: "Server network I/O".into() },
        CategoryInfo { category: "db_pool".into(), fields: "active_connection, idle_connection".into(), description: "DB connection pool".into() },
        CategoryInfo { category: "rum_page_load_each_page".into(), fields: "load_time, frontend_time, backend_time, ttfb".into(), description: "Browser page load (per URL)".into() },
        CategoryInfo { category: "rum_web_vitals_each_page".into(), fields: "lcp, fid, cls".into(), description: "Browser Core Web Vitals".into() },
        CategoryInfo { category: "rum_ajax_each_page".into(), fields: "ajax_time, ajax_count, ajax_error_rate".into(), description: "Browser AJAX stats".into() },
        CategoryInfo { category: "mobile_device_session".into(), fields: "session_count, crash_count, anr_count".into(), description: "Mobile session stats".into() },
    ];

    if config.json {
        println!("{}", serde_json::to_string_pretty(&cats)?);
    } else {
        output::info("Available stat categories", config.quiet);
        output::print_output(&cats, &config.output);
    }

    Ok(())
}

fn format_number(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f == f.floor() && f.abs() < 1e15 {
                    format!("{}", f as i64)
                } else {
                    format!("{:.2}", f)
                }
            } else {
                n.to_string()
            }
        }
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

#[derive(Serialize, Tabled)]
struct StatRow {
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "Agent")]
    agent: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Serialize, Tabled)]
struct SimpleStatRow {
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Serialize, Tabled)]
struct CategoryInfo {
    #[tabled(rename = "Category")]
    category: String,
    #[tabled(rename = "Fields")]
    fields: String,
    #[tabled(rename = "Description")]
    description: String,
}
