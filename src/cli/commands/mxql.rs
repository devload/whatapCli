use anyhow::{bail, Context, Result};
use std::io::Read;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Build the yard API request body for MXQL
fn build_mxql_request(
    pcode: i64,
    mql: &str,
    stime: u64,
    etime: u64,
    limit: u64,
    param: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": limit,
            "pageKey": "mxql",
            "param": param.unwrap_or(serde_json::json!({}))
        },
        "path": "text",
        "authKey": ""
    })
}

/// Get current epoch milliseconds
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Execute MXQL query
pub async fn run(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    query: Option<String>,
    file: Option<String>,
    input_json: Option<String>,
    stime: Option<u64>,
    etime: Option<u64>,
    limit: u64,
    category: Option<String>,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;

    // Determine the MXQL query from various sources
    let (resolved_pcode, mql, resolved_stime, resolved_etime, resolved_limit, param) =
        if let Some(ref json_str) = input_json {
            // MCP mode: full params as JSON
            let input: serde_json::Value =
                serde_json::from_str(json_str).context("Invalid --input-json")?;

            let p = input["pcode"]
                .as_i64()
                .or(pcode.map(|v| v))
                .ok_or_else(|| anyhow::anyhow!("pcode required in --input-json or --pcode"))?;
            let m = input["mql"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("mql required in --input-json"))?
                .to_string();
            let s = input["stime"].as_u64().or(stime);
            let e = input["etime"].as_u64().or(etime);
            let l = input["limit"].as_u64().unwrap_or(limit);
            let par = input.get("param").cloned();

            (p, m, s, e, l, par)
        } else {
            // Resolve MXQL from query arg, file, stdin, or category
            let mql = if let Some(ref q) = query {
                // Replace literal \n with newline
                q.replace("\\n", "\n")
            } else if let Some(ref path) = file {
                std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read MXQL file: {}", path))?
            } else if let Some(ref cat) = category {
                // Shorthand: --category → build simple query
                format!("CATEGORY {}\nTAGLOAD\nSELECT", cat)
            } else if atty::is(atty::Stream::Stdin) {
                bail!(
                    "No MXQL query provided. Use one of:\n  \
                     whatap mxql --pcode <P> \"CATEGORY app_counter\\nTAGLOAD\\nSELECT\"\n  \
                     whatap mxql --pcode <P> -f query.mxql\n  \
                     whatap mxql --pcode <P> --category mobile_crash\n  \
                     echo '<query>' | whatap mxql --pcode <P>\n  \
                     whatap mxql --input-json '<json>'  (for MCP)"
                );
            } else {
                // Read from stdin (pipe mode)
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                let trimmed = buf.trim().to_string();
                if trimmed.is_empty() {
                    bail!("Empty MXQL query from stdin");
                }
                trimmed
            };

            let p = client.resolve_pcode(pcode)?;
            (p, mql, stime, etime, limit, None)
        };

    // Default time range: last 24 hours
    let now = now_millis();
    let resolved_stime = resolved_stime.unwrap_or(now - 24 * 60 * 60 * 1000);
    let resolved_etime = resolved_etime.unwrap_or(now);

    // Build request
    let request_body = build_mxql_request(
        resolved_pcode,
        &mql,
        resolved_stime,
        resolved_etime,
        resolved_limit,
        param,
    );

    if config.verbose {
        eprintln!("MXQL query:\n{}", mql);
        eprintln!(
            "Time range: {} ~ {}",
            resolved_stime, resolved_etime
        );
        eprintln!("Pcode: {}, Limit: {}", resolved_pcode, resolved_limit);
    }

    // Execute
    let result = client.yard_post(&request_body).await?;

    // Output
    if config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        format_mxql_result(&result, config.quiet);
    }

    Ok(())
}

/// Format MXQL result for human-readable output
fn format_mxql_result(result: &serde_json::Value, quiet: bool) {
    // yard API returns {"pcode":..., "data":[...], ...} or array of records
    if let Some(records) = result.get("data").and_then(|d| d.as_array()) {
        if records.is_empty() {
            if !quiet {
                output::warn("No data returned");
            }
            return;
        }

        if !quiet {
            output::info(
                &format!("Returned {} record(s)", records.len()),
                false,
            );
            println!();
        }

        // Collect all unique keys for table header
        let mut keys: Vec<String> = Vec::new();
        for record in records {
            if let Some(obj) = record.as_object() {
                for key in obj.keys() {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
            }
        }

        if keys.is_empty() {
            // Non-object records, print as-is
            for record in records {
                println!("{}", record);
            }
            return;
        }

        // Print as simple table
        // Header
        let header: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        println!("{}", header.join(" | "));
        println!("{}", header.iter().map(|h| "-".repeat(h.len().max(8))).collect::<Vec<_>>().join("-+-"));

        // Rows
        for record in records {
            let row: Vec<String> = keys
                .iter()
                .map(|k| {
                    match record.get(k) {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(serde_json::Value::Null) => "-".to_string(),
                        Some(v) => v.to_string(),
                        None => "-".to_string(),
                    }
                })
                .collect();
            println!("{}", row.join(" | "));
        }
    } else if result.is_array() {
        // Direct array response
        if let Some(arr) = result.as_array() {
            if arr.is_empty() {
                if !quiet {
                    output::warn("No data returned");
                }
                return;
            }
            // Pretty print each record
            for item in arr {
                println!("{}", serde_json::to_string_pretty(item).unwrap_or_default());
            }
        }
    } else {
        // Single value or unknown structure
        println!("{}", serde_json::to_string_pretty(result).unwrap_or_default());
    }
}
