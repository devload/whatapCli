use anyhow::Result;
use serde::Serialize;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn parse_duration_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() { return None; }
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else {
        let last = s.chars().last()?;
        if last.is_alphabetic() { (&s[..s.len() - 1], &s[s.len() - 1..]) }
        else { (s, "ms") }
    };
    let num: u64 = num_str.parse().ok()?;
    match unit {
        "ms" => Some(num),
        "s" => Some(num * 1000),
        "m" => Some(num * 60 * 1000),
        "h" => Some(num * 3600 * 1000),
        "d" => Some(num * 86400 * 1000),
        _ => None,
    }
}

/// Snapshot response for AI analysis
#[derive(Serialize)]
struct SnapshotResponse {
    timestamp: String,
    pcode: i64,
    project: ProjectInfo,
    spot: serde_json::Value,
    metrics_1h: MetricsData,
    top_issues: TopIssues,
}

#[derive(Serialize)]
struct ProjectInfo {
    name: String,
    platform: String,
    environment: String,
}

#[derive(Serialize)]
struct MetricsData {
    tps: Vec<MetricPoint>,
    resp_time: Vec<MetricPoint>,
    err_rate: Vec<MetricPoint>,
}

#[derive(Serialize)]
struct MetricPoint {
    time: String,
    value: f64,
}

#[derive(Serialize)]
struct TopIssues {
    slow_queries: Vec<SlowQuery>,
    slow_apis: Vec<SlowApi>,
    recent_errors: Vec<ErrorEntry>,
}

#[derive(Serialize)]
struct SlowQuery {
    sql: String,
    avg_time_ms: f64,
    count: i64,
}

#[derive(Serialize)]
struct SlowApi {
    service: String,
    method: String,
    avg_time_ms: f64,
    err_count: i64,
}

#[derive(Serialize)]
struct ErrorEntry {
    time: String,
    level: String,
    message: String,
}

/// Fetch integrated analysis snapshot
pub async fn run(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    duration: Option<String>,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    let now = now_millis();
    let dur_ms = duration
        .as_ref()
        .and_then(|d| parse_duration_ms(d))
        .unwrap_or(60 * 60 * 1000); // default 1h
    let stime = now - dur_ms;

    // Get project info
    let projects = client.list_projects().await?;
    let project = projects
        .iter()
        .find(|p| p.project_code == resolved_pcode);

    let (project_info, platform) = if let Some(p) = project {
        let platform = p.product_type.as_ref()
            .or(p.platform.as_ref())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let info = ProjectInfo {
            name: p.project_name.clone(),
            platform: platform.clone(),
            environment: detect_environment(&p.project_name),
        };
        (info, platform)
    } else {
        let info = ProjectInfo {
            name: format!("pcode-{}", resolved_pcode),
            platform: "unknown".to_string(),
            environment: "unknown".to_string(),
        };
        (info, "unknown".to_string())
    };

    if config.verbose {
        eprintln!("Fetching snapshot for pcode {} (last {})", resolved_pcode, duration.as_deref().unwrap_or("1h"));
    }

    // Fetch spot metrics via MXQL (platform-aware)
    let spot = fetch_spot_via_mxql(&client, resolved_pcode, &platform, stime, now).await.unwrap_or_else(|e| {
        if config.verbose {
            eprintln!("Warning: Failed to fetch spot metrics: {}", e);
        }
        serde_json::json!({})
    });

    // Fetch metrics and issues in parallel (platform-aware)
    let metrics_future = fetch_metrics(&client, resolved_pcode, &platform, stime, now);
    let issues_future = fetch_top_issues(&client, resolved_pcode, &platform, stime, now);

    let (metrics, issues) = futures::try_join!(metrics_future, issues_future)?;

    let response = SnapshotResponse {
        timestamp: chrono::Local::now().to_rfc3339(),
        pcode: resolved_pcode,
        project: project_info,
        spot,
        metrics_1h: metrics,
        top_issues: issues,
    };

    if config.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        print_snapshot_human(&response, config);
    }

    Ok(())
}

fn detect_environment(name: &str) -> String {
    let name_lower = name.to_lowercase();
    if name_lower.contains("prod") || name_lower.contains("live") {
        "production".to_string()
    } else if name_lower.contains("staging") || name_lower.contains("stage") {
        "staging".to_string()
    } else if name_lower.contains("dev") || name_lower.contains("development") {
        "development".to_string()
    } else if name_lower.contains("test") || name_lower.contains("qa") {
        "test".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Fetch spot metrics via MXQL (yard API) - platform-aware
async fn fetch_spot_via_mxql(client: &WhatapClient, pcode: i64, platform: &str, stime: u64, etime: u64) -> Result<serde_json::Value> {
    let platform_lower = platform.to_lowercase();

    let mql = if platform_lower.contains("browser") || platform_lower.contains("rum") {
        // Browser/RUM metrics
        r#"CATEGORY rum_page_load_each_page
TAGLOAD
SELECT [page_load_count, page_load_duration, page_load_frontend_time, page_load_backend_time]
LIMIT 1"#
    } else if platform_lower.contains("mobile") {
        // Mobile metrics
        r#"CATEGORY mobile_app_use
TAGLOAD
SELECT [use_count, use_time]
LIMIT 1"#
    } else if platform_lower.contains("db") || platform_lower.contains("database") {
        // Database metrics
        r#"CATEGORY db_counter
TAGLOAD
SELECT [count, avg_time, err_count]
LIMIT 1"#
    } else {
        // APM/Server metrics (default)
        r#"CATEGORY app_counter
TAGLOAD
SELECT [tps, resp_time, err_rate, cpu, memory]
LIMIT 1"#
    };

    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 1,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    // Extract latest values from result (may be array directly or wrapped in {"data": [...]})
    if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
        if let Some(latest) = data.first() {
            return Ok(latest.clone());
        }
    } else if let Some(arr) = result.as_array() {
        if let Some(latest) = arr.first() {
            return Ok(latest.clone());
        }
    }

    Ok(serde_json::json!({}))
}

async fn fetch_metrics(client: &WhatapClient, pcode: i64, platform: &str, stime: u64, etime: u64) -> Result<MetricsData> {
    let platform_lower = platform.to_lowercase();

    let mql = if platform_lower.contains("browser") || platform_lower.contains("rum") {
        // Browser/RUM metrics
        r#"CATEGORY rum_page_load_each_page
TAGLOAD
SELECT [time, page_load_count, page_load_duration]"#
    } else if platform_lower.contains("mobile") {
        // Mobile metrics
        r#"CATEGORY mobile_app_use
TAGLOAD
SELECT [time, use_count, use_time]"#
    } else if platform_lower.contains("db") || platform_lower.contains("database") {
        // Database metrics
        r#"CATEGORY db_counter
TAGLOAD
SELECT [time, count, avg_time]"#
    } else {
        // APM/Server metrics (default)
        r#"CATEGORY app_counter
TAGLOAD
SELECT [time, tps, resp_time, err_rate]"#
    };

    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 60,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut tps_values = Vec::new();
    let mut resp_values = Vec::new();
    let mut err_values = Vec::new();

    // Yard API may return array directly or wrapped in {"data": [...]}
    let records = if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
        data.clone()
    } else if let Some(arr) = result.as_array() {
        arr.clone()
    } else {
        vec![]
    };

    for rec in records {
            let time = format_metric_time(rec.get("time").and_then(|t| t.as_u64()));

            // For browser/RUM
            if let Some(v) = rec.get("page_load_count").and_then(|t| t.as_f64()) {
                tps_values.push(MetricPoint { time: time.clone(), value: v });
            }
            if let Some(v) = rec.get("page_load_duration").and_then(|t| t.as_f64()) {
                resp_values.push(MetricPoint { time: time.clone(), value: v });
            }

            // For APM/Server
            if let Some(v) = rec.get("tps").and_then(|t| t.as_f64()) {
                tps_values.push(MetricPoint { time: time.clone(), value: v });
            }
            if let Some(v) = rec.get("resp_time").and_then(|t| t.as_f64()) {
                resp_values.push(MetricPoint { time: time.clone(), value: v });
            }
            if let Some(v) = rec.get("err_rate").and_then(|t| t.as_f64()) {
                err_values.push(MetricPoint { time: time.clone(), value: v });
            }

            // For database
            if let Some(v) = rec.get("count").and_then(|t| t.as_f64()) {
                tps_values.push(MetricPoint { time: time.clone(), value: v });
            }
            if let Some(v) = rec.get("avg_time").and_then(|t| t.as_f64()) {
                resp_values.push(MetricPoint { time: time.clone(), value: v });
            }

            // For mobile
            if let Some(v) = rec.get("use_count").and_then(|t| t.as_f64()) {
                tps_values.push(MetricPoint { time: time.clone(), value: v });
            }
            if let Some(v) = rec.get("use_time").and_then(|t| t.as_f64()) {
                resp_values.push(MetricPoint { time: time.clone(), value: v });
            }
    }

    Ok(MetricsData {
        tps: tps_values,
        resp_time: resp_values,
        err_rate: err_values,
    })
}

fn format_metric_time(epoch_ms: Option<u64>) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    epoch_ms.map(|ms| {
        let d = UNIX_EPOCH + Duration::from_millis(ms);
        let dt: chrono::DateTime<chrono::Local> = d.into();
        dt.format("%H:%M").to_string()
    }).unwrap_or_else(|| "-".to_string())
}

/// Extract records array from yard API response (may be array directly or wrapped in {"data": [...]})
fn extract_records(result: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
        data.iter().collect()
    } else if let Some(arr) = result.as_array() {
        arr.iter().collect()
    } else {
        vec![]
    }
}

async fn fetch_top_issues(client: &WhatapClient, pcode: i64, platform: &str, stime: u64, etime: u64) -> Result<TopIssues> {
    let platform_lower = platform.to_lowercase();

    let (slow_queries, slow_apis, recent_errors) = if platform_lower.contains("browser") || platform_lower.contains("rum") {
        // Browser/RUM: Look for slow AJAX calls and browser errors
        let slow_ajax_mql = r#"CATEGORY rum_ajax_each_page
TAGLOAD
SELECT [page_group, request_host, request_path, ajax_duration, count]
FILTER { ajax_duration > 1000 }
LIMIT 5"#;

        let slow_apis = fetch_browser_slow_apis(client, pcode, stime, etime, slow_ajax_mql).await.unwrap_or_default();

        let errors_mql = r#"CATEGORY browser_error
TAGLOAD
SELECT [page_group, message, count, time]
LIMIT 10"#;

        let recent_errors = fetch_browser_errors(client, pcode, stime, etime, errors_mql).await.unwrap_or_default();

        (vec![], slow_apis, recent_errors)
    } else {
        // APM/Server: Look for slow queries, slow APIs, and app errors
        let slow_queries_mql = r#"CATEGORY sql_summary
TAGLOAD
SELECT [sql_text, count, avg_time, max_time]
FILTER { avg_time > 100 }
LIMIT 5"#;

        let slow_queries = fetch_slow_queries(client, pcode, stime, etime, slow_queries_mql).await.unwrap_or_default();

        let slow_apis_mql = r#"CATEGORY tx_detail
TAGLOAD
SELECT [service, method, count, avg_time, err_count]
FILTER { avg_time > 200 }
LIMIT 5"#;

        let slow_apis = fetch_slow_apis(client, pcode, stime, etime, slow_apis_mql).await.unwrap_or_default();

        let errors_mql = r#"CATEGORY app_log
TAGLOAD
SELECT [oid, oname, @message, @level, @timestamp]
FILTER { @level == 'ERROR' }
LIMIT 10"#;

        let recent_errors = fetch_errors(client, pcode, stime, etime, errors_mql).await.unwrap_or_default();

        (slow_queries, slow_apis, recent_errors)
    };

    Ok(TopIssues {
        slow_queries,
        slow_apis,
        recent_errors,
    })
}

async fn fetch_slow_queries(
    client: &WhatapClient,
    pcode: i64,
    stime: u64,
    etime: u64,
    mql: &str,
) -> Result<Vec<SlowQuery>> {
    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 5,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut queries = Vec::new();
    for rec in extract_records(&result) {
        queries.push(SlowQuery {
            sql: rec.get("sql_text")
                .or_else(|| rec.get("sql"))
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            avg_time_ms: rec.get("avg_time")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            count: rec.get("count")
                .and_then(|v| v.as_f64())
                .map(|v| v as i64)
                .unwrap_or(0),
        });
    }

    Ok(queries)
}

async fn fetch_slow_apis(
    client: &WhatapClient,
    pcode: i64,
    stime: u64,
    etime: u64,
    mql: &str,
) -> Result<Vec<SlowApi>> {
    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 5,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut apis = Vec::new();
    for rec in extract_records(&result) {
        apis.push(SlowApi {
                service: rec.get("service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
                method: rec.get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
                avg_time_ms: rec.get("avg_time")
                    .or_else(|| rec.get("time"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                err_count: rec.get("err_count")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as i64)
                    .unwrap_or(0),
            });
    }

    Ok(apis)
}

async fn fetch_errors(
    client: &WhatapClient,
    pcode: i64,
    stime: u64,
    etime: u64,
    mql: &str,
) -> Result<Vec<ErrorEntry>> {
    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 10,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut errors = Vec::new();
    if let Some(records) = result.get("data").and_then(|d| d.as_array()) {
        for rec in records {
            let time_ms = rec.get("@timestamp")
                .or_else(|| rec.get("timestamp"))
                .or_else(|| rec.get("time"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            errors.push(ErrorEntry {
                time: format_metric_time(Some(time_ms)),
                level: rec.get("@level")
                    .or_else(|| rec.get("level"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("ERROR")
                    .to_string(),
                message: rec.get("@message")
                    .or_else(|| rec.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
            });
        }
    }

    Ok(errors)
}

async fn fetch_browser_slow_apis(
    client: &WhatapClient,
    pcode: i64,
    stime: u64,
    etime: u64,
    mql: &str,
) -> Result<Vec<SlowApi>> {
    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 5,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut apis = Vec::new();
    for rec in extract_records(&result) {
        let host = rec.get("request_host")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let path = rec.get("request_path")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let page = rec.get("page_group")
            .and_then(|v| v.as_str())
            .unwrap_or("-");

        apis.push(SlowApi {
            service: format!("{}{}", host, path),
            method: page.to_string(),
            avg_time_ms: rec.get("ajax_duration")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            err_count: 0,
        });
    }

    Ok(apis)
}

async fn fetch_browser_errors(
    client: &WhatapClient,
    pcode: i64,
    stime: u64,
    etime: u64,
    mql: &str,
) -> Result<Vec<ErrorEntry>> {
    let request_body = serde_json::json!({
        "type": "mxql",
        "pcode": pcode,
        "params": {
            "pcode": pcode,
            "stime": stime,
            "etime": etime,
            "trigger": 0,
            "mql": mql,
            "limit": 10,
            "pageKey": "mxql",
            "param": {}
        },
        "path": "text",
        "authKey": ""
    });

    let result = client.yard_post(&request_body).await?;

    let mut errors = Vec::new();
    for rec in extract_records(&result) {
        let time_ms = rec.get("time")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let page = rec.get("page_group")
            .and_then(|v| v.as_str())
            .unwrap_or("-");

        errors.push(ErrorEntry {
            time: format_metric_time(Some(time_ms)),
            level: "ERROR".to_string(),
            message: format!("{}: {}", page, rec.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("-")),
        });
    }

    Ok(errors)
}

fn print_snapshot_human(response: &SnapshotResponse, config: &ResolvedConfig) {
    output::info(
        &format!("Snapshot: {} ({}) - {}",
            response.project.name,
            response.pcode,
            response.timestamp
        ),
        config.quiet,
    );
    println!();

    // Project
    println!("Project:");
    println!("  Platform: {}", response.project.platform);
    println!("  Environment: {}", response.project.environment);
    println!();

    // Real-time spot
    println!("Real-time Metrics:");
    print_spot_metrics(&response.spot);
    println!();

    // Metrics trend
    println!("Trend ({} points):", response.metrics_1h.tps.len());
    if !response.metrics_1h.tps.is_empty() {
        let last_tps = response.metrics_1h.tps.last().map(|p| p.value).unwrap_or(0.0);
        let last_resp = response.metrics_1h.resp_time.last().map(|p| p.value).unwrap_or(0.0);
        let last_err = response.metrics_1h.err_rate.last().map(|p| p.value).unwrap_or(0.0);
        println!("  Latest: TPS={:.0}, Resp Time={:.0}ms, Error Rate={:.2}%", last_tps, last_resp, last_err);
    } else {
        println!("  (no time-series data available)");
    }
    println!();

    // Top issues
    println!("Top Issues:");

    if !response.top_issues.slow_queries.is_empty() {
        println!("  Slow Queries:");
        for q in &response.top_issues.slow_queries {
            println!("    - {}ms (count: {}): {}",
                q.avg_time_ms as i64,
                q.count,
                truncate(&q.sql, 50)
            );
        }
    }

    if !response.top_issues.slow_apis.is_empty() {
        println!("  Slow APIs:");
        for a in &response.top_issues.slow_apis {
            let err_info = if a.err_count > 0 {
                format!(" (errors: {})", a.err_count)
            } else {
                "".to_string()
            };
            println!("    - {}ms: {}.{}{}",
                a.avg_time_ms as i64,
                a.service,
                a.method,
                err_info
            );
        }
    }

    if !response.top_issues.recent_errors.is_empty() {
        println!("  Recent Errors:");
        for e in &response.top_issues.recent_errors {
            println!("    [{}] {}: {}", e.time, e.level, truncate(&e.message, 60));
        }
    }

    if response.top_issues.slow_queries.is_empty()
        && response.top_issues.slow_apis.is_empty()
        && response.top_issues.recent_errors.is_empty()
    {
        println!("  (no issues detected)");
    }
}

fn print_spot_metrics(spot: &serde_json::Value) {
    if let Some(obj) = spot.as_object() {
        // Key metrics to show
        let key_metrics: [(&str, &str); 6] = [
            ("tps", "TPS"),
            ("resp_time", "Response Time"),
            ("err_rate", "Error Rate"),
            ("cpu", "CPU"),
            ("memory", "Memory"),
            ("active_threads", "Active Threads"),
        ];

        for (key, label) in &key_metrics {
            if let Some(val) = obj.get(*key) {
                let formatted = format_spot_value(val);
                let unit = if *key == "resp_time" { "ms" }
                    else if *key == "err_rate" { "%" }
                    else if *key == "cpu" || *key == "memory" { "%" }
                    else { "" };
                println!("  {}: {}{}", label, formatted, unit);
            }
        }
    }
}

fn format_spot_value(val: &serde_json::Value) -> String {
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
        serde_json::Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}...", &s[..max.saturating_sub(3)]) }
}
