use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Fetch current spot metrics via Open API
pub async fn run(config: &ResolvedConfig, pcode: Option<i64>, keys: Option<String>) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let path = format!("/open/api/json/spot");
    let resp = client.get_with_pcode(&path, pcode).await?;
    let body = resp.text().await?;
    let data: serde_json::Value = serde_json::from_str(&body)?;

    // If keys are specified, filter to only those
    if let Some(keys_str) = &keys {
        let filter_keys: Vec<&str> = keys_str.split(',').map(|s| s.trim()).collect();

        if config.json {
            let mut filtered = serde_json::Map::new();
            if let Some(obj) = data.as_object() {
                for key in &filter_keys {
                    if let Some(val) = obj.get(*key) {
                        filtered.insert(key.to_string(), val.clone());
                    }
                }
            }
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        } else {
            let mut rows: Vec<SpotRow> = Vec::new();
            if let Some(obj) = data.as_object() {
                for key in &filter_keys {
                    if let Some(val) = obj.get(*key) {
                        rows.push(SpotRow {
                            key: key.to_string(),
                            value: format_value(val),
                        });
                    }
                }
            }
            if rows.is_empty() {
                output::warn("No matching metrics found");
            } else {
                output::print_output(&rows, &config.output);
            }
        }
        return Ok(());
    }

    // Show all metrics
    if config.json {
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else {
        let mut rows: Vec<SpotRow> = Vec::new();
        if let Some(obj) = data.as_object() {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(val) = obj.get(key) {
                    // Skip null/empty values
                    if val.is_null() {
                        continue;
                    }
                    rows.push(SpotRow {
                        key: key.clone(),
                        value: format_value(val),
                    });
                }
            }
        }
        if rows.is_empty() {
            output::warn("No spot metrics available. Check pcode and API token.");
        } else {
            output::info(&format!("Spot metrics for pcode {} ({} fields)", pcode, rows.len()), config.quiet);
            output::print_output(&rows, &config.output);
        }
    }

    Ok(())
}

fn format_value(val: &serde_json::Value) -> String {
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
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

#[derive(Serialize, Tabled)]
struct SpotRow {
    #[tabled(rename = "Metric")]
    key: String,
    #[tabled(rename = "Value")]
    value: String,
}
