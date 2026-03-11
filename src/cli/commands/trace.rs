use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Parse key format "page_group@time_suffix" into (page_group, time_suffix)
fn parse_key(key: &str) -> Option<(String, u64)> {
    let parts: Vec<&str> = key.rsplitn(2, '@').collect();
    if parts.len() != 2 {
        // Allow plain page_group without @suffix
        return Some((key.to_string(), 0));
    }
    let time_suffix: u64 = parts[0].parse().ok()?;
    let page_group = parts[1].to_string();
    Some((page_group, time_suffix))
}

/// Parse duration string to milliseconds
fn parse_duration(duration: &str) -> Result<u64> {
    let duration = duration.trim();
    let (num, unit) = if duration.ends_with("s") {
        (duration.trim_end_matches('s'), 1000)
    } else if duration.ends_with("m") {
        (duration.trim_end_matches('m'), 60 * 1000)
    } else if duration.ends_with("h") {
        (duration.trim_end_matches('h'), 3600 * 1000)
    } else if duration.ends_with("d") {
        (duration.trim_end_matches('d'), 24 * 3600 * 1000)
    } else {
        (duration, 1000) // default to ms
    };
    let value: u64 = num.parse()?;
    Ok(value * unit)
}

/// Get time range based on duration
fn get_time_range(duration: &str) -> Result<(u64, u64)> {
    let duration_ms = parse_duration(duration)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let stime = now.saturating_sub(duration_ms);
    Ok((stime, now))
}

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

fn get_str<'a>(val: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    val.get(key).and_then(|v| v.as_str())
}

fn get_f64(val: &serde_json::Value, key: &str) -> Option<f64> {
    val.get(key).and_then(|v| v.as_f64())
}

fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 { return format!("{:.1}ms", ms); }
    if ms < 1000.0 { return format!("{:.0}ms", ms); }
    format!("{:.2}s", ms / 1000.0)
}

fn format_size(bytes: f64) -> String {
    if bytes < 1024.0 { return format!("{}B", bytes as i64); }
    if bytes < 1024.0 * 1024.0 { return format!("{:.1}KB", bytes / 1024.0); }
    format!("{:.1}MB", bytes / (1024.0 * 1024.0))
}

fn extract_records(result: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = result.get("data").and_then(|d| d.as_array()) {
        arr.clone()
    } else if let Some(arr) = result.as_array() {
        arr.clone()
    } else {
        vec![]
    }
}

/// Trace correlated data across browser monitoring categories
pub async fn run(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    key: &str,
    duration: &str,
    only: Option<&str>,
    slow: Option<u64>,
    summary: bool,
    json: bool,
    csv: bool,
    raw: bool,
) -> Result<()> {
    let (page_group, _time_suffix) = parse_key(key)
        .ok_or_else(|| anyhow::anyhow!("Invalid key format. Expected: page_group@time_suffix (e.g. /cart@123456) or just page_group"))?;
    
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;
    
    let (stime, etime) = get_time_range(duration)?;
    
    if config.verbose {
        eprintln!("Key: {} (page_group={})", key, page_group);
        eprintln!("Time range: {} - {} (duration={})", stime, etime, duration);
    }
    
    output::info(&format!("Tracing: {}", key), config.quiet);
    println!("Page: {}", page_group);
    if let Some(slow_ms) = slow {
        println!("Filter: slow > {}ms", slow_ms);
    }
    println!();
    
    // Determine which categories to show
    let show_pageload = only.is_none() || only == Some("pageload");
    let show_ajax = only.is_none() || only == Some("ajax");
    let show_resources = only.is_none() || only == Some("resources");
    let show_errors = only.is_none() || only == Some("errors");
    
    if raw {
        // Raw JSON output
        let mut all_data = serde_json::Map::new();
        
        if show_pageload {
            all_data.insert("pageload".into(), fetch_pageload(&client, resolved_pcode, &page_group, stime, etime).await?);
        }
        if show_ajax {
            all_data.insert("ajax".into(), fetch_ajax(&client, resolved_pcode, &page_group, stime, etime).await?);
        }
        if show_resources {
            all_data.insert("resources".into(), fetch_resources(&client, resolved_pcode, &page_group, stime, etime).await?);
        }
        if show_errors {
            all_data.insert("errors".into(), fetch_errors(&client, resolved_pcode, &page_group, stime, etime).await?);
        }
        
        println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(all_data))?);
        return Ok(());
    }
    
    let mut found_any = false;
    let mut summary_data = TraceSummary::default();
    
    if show_pageload {
        let result = trace_pageload(&client, resolved_pcode, &page_group, stime, etime, slow, summary, json, csv, &mut summary_data).await?;
        found_any |= result;
    }
    
    if show_ajax {
        let result = trace_ajax(&client, resolved_pcode, &page_group, stime, etime, slow, summary, json, csv, &mut summary_data).await?;
        found_any |= result;
    }
    
    if show_resources {
        let result = trace_resources(&client, resolved_pcode, &page_group, stime, etime, slow, summary, json, csv, &mut summary_data).await?;
        found_any |= result;
    }
    
    if show_errors {
        let result = trace_errors(&client, resolved_pcode, &page_group, stime, etime, slow, summary, json, csv, &mut summary_data).await?;
        found_any |= result;
    }
    
    // Print summary if requested
    if summary {
        println!("=== Summary ===");
        println!("Page Load entries: {}", summary_data.pageload_count);
        println!("AJAX requests: {} (errors: {})", summary_data.ajax_count, summary_data.ajax_errors);
        println!("Resources: {} (3rd party: {})", summary_data.resource_count, summary_data.resource_3rd_party);
        println!("Errors: {}", summary_data.error_count);
        println!();
        
        if summary_data.ajax_errors > 0 || summary_data.error_count > 0 {
            output::warn("Issues detected: check AJAX errors and JS errors");
        }
    }
    
    if !found_any && !summary {
        output::warn("No correlated data found for this key");
    }
    
    Ok(())
}

#[derive(Default)]
struct TraceSummary {
    pageload_count: usize,
    ajax_count: usize,
    ajax_errors: usize,
    resource_count: usize,
    resource_3rd_party: usize,
    error_count: usize,
}

// Fetch functions

async fn fetch_pageload(
    client: &WhatapClient,
    pcode: i64,
    _page_group: &str,
    stime: u64,
    etime: u64,
) -> Result<serde_json::Value> {
    let mql = "CATEGORY rum_page_load_each_page\nTAGLOAD\nSELECT [page_group, page_load_count, page_load_duration, page_load_backend_time, page_load_frontend_time, page_load_firstbyte_time, page_load_render_time, time]\nLIMIT 200";
    let request = build_yard_request(pcode, mql, stime, etime);
    client.yard_post(&request).await
}

async fn fetch_ajax(
    client: &WhatapClient,
    pcode: i64,
    _page_group: &str,
    stime: u64,
    etime: u64,
) -> Result<serde_json::Value> {
    let mql = "CATEGORY rum_ajax_each_page\nTAGLOAD\nSELECT [page_group, request_host, request_path, ajax_count, ajax_5xx_count, ajax_4xx_count, ajax_duration, time]\nLIMIT 300";
    let request = build_yard_request(pcode, mql, stime, etime);
    client.yard_post(&request).await
}

async fn fetch_resources(
    client: &WhatapClient,
    pcode: i64,
    _page_group: &str,
    stime: u64,
    etime: u64,
) -> Result<serde_json::Value> {
    let mql = "CATEGORY rum_resource_each_page\nTAGLOAD\nSELECT [page_group, request_host, request_path, type, resource_duration, resource_size, is3rdParty, time]\nLIMIT 300";
    let request = build_yard_request(pcode, mql, stime, etime);
    client.yard_post(&request).await
}

async fn fetch_errors(
    client: &WhatapClient,
    pcode: i64,
    _page_group: &str,
    stime: u64,
    etime: u64,
) -> Result<serde_json::Value> {
    let mql = "CATEGORY rum_error_total_each_page\nTAGLOAD\nSELECT [page_group, error_type, error_message, count, browser, os, device, @timestamp]\nLIMIT 200";
    let request = build_yard_request(pcode, mql, stime, etime);
    client.yard_post(&request).await
}

// Trace functions

async fn trace_pageload(
    client: &WhatapClient,
    pcode: i64,
    page_group: &str,
    stime: u64,
    etime: u64,
    slow: Option<u64>,
    summary: bool,
    json: bool,
    csv: bool,
    summary_data: &mut TraceSummary,
) -> Result<bool> {
    let result = fetch_pageload(client, pcode, page_group, stime, etime).await?;
    let all_records = extract_records(&result);
    
    let records: Vec<_> = all_records
        .into_iter()
        .filter(|rec| {
            let pg = get_str(rec, "page_group").unwrap_or("");
            if pg != page_group {
                return false;
            }
            if let Some(slow_ms) = slow {
                let duration = get_f64(rec, "page_load_duration").unwrap_or(0.0);
                if duration < slow_ms as f64 {
                    return false;
                }
            }
            true
        })
        .collect();
    
    summary_data.pageload_count = records.len();
    
    if records.is_empty() {
        return Ok(false);
    }
    
    if summary {
        return Ok(true);
    }
    
    println!("=== Page Load ===");
    
    if json {
        let rows: Vec<PageLoadRow> = records.iter().map(|rec| {
            PageLoadRow {
                count: get_f64(rec, "page_load_count").unwrap_or(0.0) as i64,
                total_duration_ms: get_f64(rec, "page_load_duration").unwrap_or(0.0),
                ttfb_ms: get_f64(rec, "page_load_firstbyte_time").unwrap_or(0.0),
                backend_ms: get_f64(rec, "page_load_backend_time").unwrap_or(0.0),
                frontend_ms: get_f64(rec, "page_load_frontend_time").unwrap_or(0.0),
                render_ms: get_f64(rec, "page_load_render_time").unwrap_or(0.0),
            }
        }).collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(true);
    }
    
    if csv {
        println!("count,total_duration_ms,ttfb_ms,backend_ms,frontend_ms,render_ms");
        for rec in &records {
            let count = get_f64(rec, "page_load_count").unwrap_or(0.0) as i64;
            let total = get_f64(rec, "page_load_duration").unwrap_or(0.0);
            let ttfb = get_f64(rec, "page_load_firstbyte_time").unwrap_or(0.0);
            let backend = get_f64(rec, "page_load_backend_time").unwrap_or(0.0);
            let frontend = get_f64(rec, "page_load_frontend_time").unwrap_or(0.0);
            let render = get_f64(rec, "page_load_render_time").unwrap_or(0.0);
            println!("{},{},{},{},{},{}", count, total, ttfb, backend, frontend, render);
        }
        return Ok(true);
    }
    
    // Default: visual output
    for rec in &records {
        let total = get_f64(rec, "page_load_duration").unwrap_or(0.0);
        let count = get_f64(rec, "page_load_count").unwrap_or(1.0) as i64;
        
        println!("\n  Count: {} | Total Duration: {}", count, format_duration_ms(total));
        println!("  {}", "-".repeat(50));
        
        let phases = [
            ("DNS", get_f64(rec, "page_load_dns_time")),
            ("Connect", get_f64(rec, "page_load_connect_time")),
            ("SSL", get_f64(rec, "page_load_ssl_time")),
            ("TTFB", get_f64(rec, "page_load_firstbyte_time")),
            ("Download", get_f64(rec, "page_load_download_time")),
            ("Backend", get_f64(rec, "page_load_backend_time")),
            ("Frontend", get_f64(rec, "page_load_frontend_time")),
            ("Render", get_f64(rec, "page_load_render_time")),
        ];
        
        let max_val = phases.iter()
            .filter_map(|(_, v)| *v)
            .fold(0.0f64, f64::max)
            .max(1.0);
        
        for (label, val) in &phases {
            if let Some(ms) = val {
                if *ms > 0.0 {
                    let bar_len = ((ms / max_val) * 20.0) as usize;
                    let bar: String = "#".repeat(bar_len.min(20));
                    println!("  {:>10} {:>8} |{}", label, format_duration_ms(*ms), bar);
                }
            }
        }
    }
    println!();
    
    Ok(true)
}

async fn trace_ajax(
    client: &WhatapClient,
    pcode: i64,
    page_group: &str,
    stime: u64,
    etime: u64,
    slow: Option<u64>,
    summary_only: bool,
    json: bool,
    csv: bool,
    summary_data: &mut TraceSummary,
) -> Result<bool> {
    let result = fetch_ajax(client, pcode, page_group, stime, etime).await?;
    let all_records = extract_records(&result);
    
    let records: Vec<_> = all_records
        .into_iter()
        .filter(|rec| {
            let pg = get_str(rec, "page_group").unwrap_or("");
            if pg != page_group {
                return false;
            }
            if let Some(slow_ms) = slow {
                let duration = get_f64(rec, "ajax_duration").unwrap_or(0.0);
                if duration < slow_ms as f64 {
                    return false;
                }
            }
            true
        })
        .collect();
    
    summary_data.ajax_count = records.len();
    summary_data.ajax_errors = records.iter().filter(|rec| {
        let err_5xx = get_f64(rec, "ajax_5xx_count").unwrap_or(0.0) as i64;
        let err_4xx = get_f64(rec, "ajax_4xx_count").unwrap_or(0.0) as i64;
        err_5xx > 0 || err_4xx > 0
    }).count();
    
    if records.is_empty() {
        return Ok(false);
    }
    
    if summary_only {
        return Ok(true);
    }
    
    println!("=== AJAX Requests ===");
    
    let mut rows: Vec<AjaxRow> = Vec::new();
    for rec in &records {
        let host = get_str(rec, "request_host").unwrap_or("-");
        let path = get_str(rec, "request_path").unwrap_or("-");
        let url = format!("{}{}", host, path);
        let short_url = if url.len() > 45 {
            format!("{}...{}", &url[..20], &url[url.len()-20..])
        } else {
            url.to_string()
        };
        
        let err_5xx = get_f64(rec, "ajax_5xx_count").unwrap_or(0.0) as i64;
        let err_4xx = get_f64(rec, "ajax_4xx_count").unwrap_or(0.0) as i64;
        let error_info = if err_5xx > 0 || err_4xx > 0 {
            format!("5xx:{} 4xx:{}", err_5xx, err_4xx)
        } else {
            "-".to_string()
        };
        
        rows.push(AjaxRow {
            url: short_url,
            count: get_f64(rec, "ajax_count").map(|c| format!("{}", c as i64)).unwrap_or("-".into()),
            duration: get_f64(rec, "ajax_duration").map(format_duration_ms).unwrap_or("-".into()),
            duration_ms: get_f64(rec, "ajax_duration").unwrap_or(0.0),
            errors: error_info,
        });
    }
    
    if json {
        let output: Vec<JsonAjaxRow> = rows.iter().map(|r| JsonAjaxRow {
            url: r.url.clone(),
            count: r.count.clone(),
            duration: r.duration.clone(),
            duration_ms: r.duration_ms,
            errors: r.errors.clone(),
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(true);
    }
    
    if csv {
        println!("url,count,duration_ms,errors");
        for row in &rows {
            println!("{},{},{},{}", row.url, row.count, row.duration_ms, row.errors);
        }
        return Ok(true);
    }
    
    output::print_output(&rows, "table");
    Ok(true)
}

async fn trace_resources(
    client: &WhatapClient,
    pcode: i64,
    page_group: &str,
    stime: u64,
    etime: u64,
    slow: Option<u64>,
    summary_only: bool,
    json: bool,
    csv: bool,
    summary_data: &mut TraceSummary,
) -> Result<bool> {
    let result = fetch_resources(client, pcode, page_group, stime, etime).await?;
    let all_records = extract_records(&result);
    
    let records: Vec<_> = all_records
        .into_iter()
        .filter(|rec| {
            let pg = get_str(rec, "page_group").unwrap_or("");
            if pg != page_group {
                return false;
            }
            if let Some(slow_ms) = slow {
                let duration = get_f64(rec, "resource_duration").unwrap_or(0.0);
                if duration < slow_ms as f64 {
                    return false;
                }
            }
            true
        })
        .collect();
    
    summary_data.resource_count = records.len();
    summary_data.resource_3rd_party = records.iter().filter(|rec| {
        get_str(rec, "is3rdParty").map(|s| s == "true").unwrap_or(false)
    }).count();
    
    if records.is_empty() {
        return Ok(false);
    }
    
    if summary_only {
        return Ok(true);
    }
    
    println!("=== Resources ===");
    
    let mut rows: Vec<ResourceRow> = Vec::new();
    for rec in &records {
        let host = get_str(rec, "request_host").unwrap_or("-");
        let path = get_str(rec, "request_path").unwrap_or("-");
        let url = format!("{}{}", host, path);
        let short_url = if url.len() > 45 {
            format!("{}...{}", &url[..20], &url[url.len()-20..])
        } else {
            url.to_string()
        };
        
        let size_val = get_f64(rec, "resource_size").unwrap_or(0.0);
        
        rows.push(ResourceRow {
            resource_type: get_str(rec, "type").unwrap_or("-").to_string(),
            url: short_url,
            duration: get_f64(rec, "resource_duration").map(format_duration_ms).unwrap_or("-".into()),
            duration_ms: get_f64(rec, "resource_duration").unwrap_or(0.0),
            size: format_size(size_val),
            size_bytes: size_val as i64,
            is_3rd_party: get_str(rec, "is3rdParty").unwrap_or("-").to_string(),
        });
    }
    
    if json {
        let output: Vec<JsonResourceRow> = rows.iter().map(|r| JsonResourceRow {
            resource_type: r.resource_type.clone(),
            url: r.url.clone(),
            duration: r.duration.clone(),
            duration_ms: r.duration_ms,
            size: r.size.clone(),
            size_bytes: r.size_bytes,
            is_3rd_party: r.is_3rd_party.clone(),
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(true);
    }
    
    if csv {
        println!("type,url,duration_ms,size_bytes,3rd_party");
        for row in &rows {
            println!("{},{},{},{},{}", row.resource_type, row.url, row.duration_ms, row.size_bytes, row.is_3rd_party);
        }
        return Ok(true);
    }
    
    output::print_output(&rows, "table");
    Ok(true)
}

async fn trace_errors(
    client: &WhatapClient,
    pcode: i64,
    page_group: &str,
    stime: u64,
    etime: u64,
    slow: Option<u64>,
    summary_only: bool,
    json: bool,
    csv: bool,
    summary_data: &mut TraceSummary,
) -> Result<bool> {
    let _ = slow; // errors don't have duration
    let result = fetch_errors(client, pcode, page_group, stime, etime).await?;
    let all_records = extract_records(&result);
    
    let records: Vec<_> = all_records
        .into_iter()
        .filter(|rec| {
            let pg = get_str(rec, "page_group").unwrap_or("");
            pg == page_group
        })
        .collect();
    
    summary_data.error_count = records.len();
    
    if records.is_empty() {
        return Ok(false);
    }
    
    if summary_only {
        return Ok(true);
    }
    
    println!("=== Errors ===");
    
    let mut rows: Vec<ErrorRow> = Vec::new();
    for rec in &records {
        let msg = get_str(rec, "error_message").unwrap_or("-");
        let truncated = if msg.len() > 45 {
            format!("{}...", &msg[..42])
        } else {
            msg.to_string()
        };
        
        rows.push(ErrorRow {
            error_type: get_str(rec, "error_type").unwrap_or("-").to_string(),
            message: truncated,
            full_message: msg.to_string(),
            count: get_f64(rec, "count").map(|c| format!("{}", c as i64)).unwrap_or("-".into()),
            count_num: get_f64(rec, "count").unwrap_or(0.0) as i64,
            browser: get_str(rec, "browser").unwrap_or("-").to_string(),
            device: get_str(rec, "device").unwrap_or("-").to_string(),
        });
    }
    
    if json {
        let output: Vec<JsonErrorRow> = rows.iter().map(|r| JsonErrorRow {
            error_type: r.error_type.clone(),
            message: r.full_message.clone(),
            count: r.count_num,
            browser: r.browser.clone(),
            device: r.device.clone(),
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(true);
    }
    
    if csv {
        println!("error_type,message,count,browser,device");
        for row in &rows {
            let escaped_msg = row.full_message.replace("\"", "\"\"");
            println!("{},\"{}\",{},{},{}", row.error_type, escaped_msg, row.count_num, row.browser, row.device);
        }
        return Ok(true);
    }
    
    output::print_output(&rows, "table");
    Ok(true)
}

// Row structs for table display

#[derive(Serialize, Tabled)]
struct AjaxRow {
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Count")]
    count: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(skip)]
    duration_ms: f64,
    #[tabled(rename = "Errors")]
    errors: String,
}

#[derive(Serialize)]
struct JsonAjaxRow {
    url: String,
    count: String,
    duration: String,
    duration_ms: f64,
    errors: String,
}

#[derive(Serialize, Tabled)]
struct ResourceRow {
    #[tabled(rename = "Type")]
    resource_type: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(skip)]
    duration_ms: f64,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(skip)]
    size_bytes: i64,
    #[tabled(rename = "3rd")]
    is_3rd_party: String,
}

#[derive(Serialize)]
struct JsonResourceRow {
    resource_type: String,
    url: String,
    duration: String,
    duration_ms: f64,
    size: String,
    size_bytes: i64,
    is_3rd_party: String,
}

#[derive(Serialize, Tabled)]
struct ErrorRow {
    #[tabled(rename = "Error Type")]
    error_type: String,
    #[tabled(rename = "Message")]
    message: String,
    #[tabled(skip)]
    full_message: String,
    #[tabled(rename = "Count")]
    count: String,
    #[tabled(skip)]
    count_num: i64,
    #[tabled(rename = "Browser")]
    browser: String,
    #[tabled(rename = "Device")]
    device: String,
}

#[derive(Serialize)]
struct JsonErrorRow {
    error_type: String,
    message: String,
    count: i64,
    browser: String,
    device: String,
}

#[derive(Serialize)]
struct PageLoadRow {
    count: i64,
    total_duration_ms: f64,
    ttfb_ms: f64,
    backend_ms: f64,
    frontend_ms: f64,
    render_ms: f64,
}
