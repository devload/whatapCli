use anyhow::{Context, Result};
use serde::Serialize;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

/// Project context information for AI analysis
#[derive(Serialize)]
struct ProjectContext {
    pcode: i64,
    name: String,
    platform: String,
    environment: String,
    license_key: Option<String>,
    api_token: Option<String>,
    created_at: Option<String>,
}

/// Baseline statistics (computed from recent data)
#[derive(Serialize)]
struct Baseline {
    tps: Option<MetricBaseline>,
    resp_time: Option<MetricBaseline>,
    err_rate: Option<MetricBaseline>,
    cpu: Option<MetricBaseline>,
    memory: Option<MetricBaseline>,
}

#[derive(Serialize)]
struct MetricBaseline {
    avg: f64,
    p50: f64,
    p95: f64,
}

/// Default thresholds by platform type
#[derive(Serialize)]
struct Thresholds {
    resp_time_warning: u64,
    resp_time_critical: u64,
    err_rate_warning: f64,
    err_rate_critical: f64,
    cpu_warning: f64,
    cpu_critical: f64,
    memory_warning: f64,
    memory_critical: f64,
}

/// Available categories for this project type
#[derive(Serialize)]
struct Categories {
    metrics: Vec<String>,
    logs: Vec<String>,
}

/// Full info response
#[derive(Serialize)]
struct InfoResponse {
    project: ProjectContext,
    baseline: Baseline,
    thresholds: Thresholds,
    categories: Categories,
}

pub async fn run(config: &ResolvedConfig, pcode: i64) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let projects = client.list_projects().await?;

    let project = projects
        .iter()
        .find(|p| p.project_code == pcode)
        .with_context(|| format!("Project with code {} not found", pcode))?;

    // Determine platform type
    let platform = project
        .product_type
        .as_ref()
        .or(project.platform.as_ref())
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    let platform_lower = platform.to_lowercase();

    // Build project context
    let project_ctx = ProjectContext {
        pcode: project.project_code,
        name: project.project_name.clone(),
        platform: platform.clone(),
        environment: detect_environment(&project.project_name),
        license_key: project.license_key.clone(),
        api_token: project.api_token.clone(),
        created_at: project.create_time.as_ref().map(|v| v.to_string()),
    };

    // Get baseline (try to fetch, default to None if unavailable)
    let baseline = compute_baseline(&client, pcode, &platform_lower).await.unwrap_or_else(|_| Baseline {
        tps: None,
        resp_time: None,
        err_rate: None,
        cpu: None,
        memory: None,
    });

    // Get thresholds based on platform
    let thresholds = get_thresholds(&platform_lower);

    // Get categories based on platform
    let categories = get_categories(&platform_lower);

    let response = InfoResponse {
        project: project_ctx,
        baseline,
        thresholds,
        categories,
    };

    if config.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        output::info(&format!("Project: {} ({})", project.project_name, pcode), config.quiet);
        println!();

        // Project info
        println!("Project:");
        println!("  Platform: {}", response.project.platform);
        println!("  Environment: {}", response.project.environment);
        if let Some(ref created) = response.project.created_at {
            println!("  Created: {}", created);
        }
        println!();

        // Baseline
        println!("Baseline (7-day average):");
        if let Some(ref tps) = response.baseline.tps {
            println!("  TPS: avg={:.0}, p50={:.0}, p95={:.0}", tps.avg, tps.p50, tps.p95);
        }
        if let Some(ref resp) = response.baseline.resp_time {
            println!("  Response Time: avg={:.0}ms, p50={:.0}ms, p95={:.0}ms", resp.avg, resp.p50, resp.p95);
        }
        if let Some(ref err) = response.baseline.err_rate {
            println!("  Error Rate: avg={:.2}%, p95={:.2}%", err.avg, err.p95);
        }
        if let Some(ref cpu) = response.baseline.cpu {
            println!("  CPU: avg={:.1}%, p95={:.1}%", cpu.avg, cpu.p95);
        }
        if let Some(ref mem) = response.baseline.memory {
            println!("  Memory: avg={:.1}%, p95={:.1}%", mem.avg, mem.p95);
        }
        if response.baseline.tps.is_none() && response.baseline.resp_time.is_none() {
            println!("  (no data available)");
        }
        println!();

        // Thresholds
        println!("Thresholds:");
        println!("  Response Time: warning={}ms, critical={}ms",
            response.thresholds.resp_time_warning, response.thresholds.resp_time_critical);
        println!("  Error Rate: warning={:.1}%, critical={:.1}%",
            response.thresholds.err_rate_warning, response.thresholds.err_rate_critical);
        println!("  CPU: warning={:.0}%, critical={:.0}%",
            response.thresholds.cpu_warning, response.thresholds.cpu_critical);
        println!("  Memory: warning={:.0}%, critical={:.0}%",
            response.thresholds.memory_warning, response.thresholds.memory_critical);
        println!();

        // Categories
        println!("Available Categories:");
        println!("  Metrics: {}", response.categories.metrics.join(", "));
        println!("  Logs: {}", response.categories.logs.join(", "));
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

fn get_thresholds(platform: &str) -> Thresholds {
    match platform {
        // Browser/Mobile - faster thresholds
        p if p.contains("browser") || p.contains("rum") => Thresholds {
            resp_time_warning: 3000,
            resp_time_critical: 5000,
            err_rate_warning: 1.0,
            err_rate_critical: 5.0,
            cpu_warning: 0.0,
            cpu_critical: 0.0,
            memory_warning: 0.0,
            memory_critical: 0.0,
        },
        p if p.contains("mobile") || p.contains("android") || p.contains("ios") => Thresholds {
            resp_time_warning: 2000,
            resp_time_critical: 5000,
            err_rate_warning: 0.5,
            err_rate_critical: 2.0,
            cpu_warning: 0.0,
            cpu_critical: 0.0,
            memory_warning: 0.0,
            memory_critical: 0.0,
        },
        // Database - slower thresholds
        p if p.contains("db") || p.contains("database") || p.contains("sql") => Thresholds {
            resp_time_warning: 1000,
            resp_time_critical: 3000,
            err_rate_warning: 0.5,
            err_rate_critical: 2.0,
            cpu_warning: 80.0,
            cpu_critical: 95.0,
            memory_warning: 85.0,
            memory_critical: 95.0,
        },
        // APM/Server - standard thresholds
        _ => Thresholds {
            resp_time_warning: 500,
            resp_time_critical: 1000,
            err_rate_warning: 1.0,
            err_rate_critical: 5.0,
            cpu_warning: 80.0,
            cpu_critical: 95.0,
            memory_warning: 85.0,
            memory_critical: 95.0,
        },
    }
}

fn get_categories(platform: &str) -> Categories {
    match platform {
        // Browser RUM
        p if p.contains("browser") || p.contains("rum") => Categories {
            metrics: vec![
                "rum_page_load_each_page".into(),
                "rum_ajax_each_page".into(),
                "rum_resource_each_page".into(),
                "rum_error_total_each_page".into(),
                "rum_web_vitals_each_page".into(),
            ],
            logs: vec![
                "browser_error".into(),
                "rum_event".into(),
            ],
        },
        // Mobile
        p if p.contains("mobile") || p.contains("android") || p.contains("ios") => Categories {
            metrics: vec![
                "mobile_app_start".into(),
                "mobile_app_use".into(),
                "mobile_crash".into(),
                "mobile_anr".into(),
                "mobile_network".into(),
            ],
            logs: vec![
                "mobile_crash".into(),
                "mobile_exception".into(),
                "mobile_user_action".into(),
            ],
        },
        // Database
        p if p.contains("db") || p.contains("database") || p.contains("sql") => Categories {
            metrics: vec![
                "db_counter".into(),
                "db_sql".into(),
                "db_pool".into(),
                "db_lock".into(),
            ],
            logs: vec![
                "db_log".into(),
                "db_slow_query".into(),
            ],
        },
        // APM/Server
        _ => Categories {
            metrics: vec![
                "app_counter".into(),
                "app_user".into(),
                "app_sql".into(),
                "app_httpc".into(),
                "server_cpu".into(),
                "server_memory".into(),
                "server_disk".into(),
                "server_network".into(),
            ],
            logs: vec![
                "app_log".into(),
                "sql_log".into(),
            ],
        },
    }
}

async fn compute_baseline(client: &WhatapClient, pcode: i64, platform: &str) -> Result<Baseline> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // 7 days ago
    let stime = now - 7 * 24 * 60 * 60 * 1000;

    let mut baseline = Baseline {
        tps: None,
        resp_time: None,
        err_rate: None,
        cpu: None,
        memory: None,
    };

    // Try to get baseline for APM/Server projects
    if !platform.contains("browser") && !platform.contains("mobile") {
        // Try app_counter for TPS and response time
        let path = format!(
            "/open/api/json/tag/app_counter/tps?stime={}&etime={}",
            stime, now
        );
        if let Ok(resp) = client.get_with_pcode(&path, Some(pcode)).await {
            if let Ok(body) = resp.text().await {
                if let Ok(values) = extract_metric_values(&body) {
                    if !values.is_empty() {
                        baseline.tps = Some(compute_stats(&values));
                    }
                }
            }
        }

        let path = format!(
            "/open/api/json/tag/app_counter/resp_time?stime={}&etime={}",
            stime, now
        );
        if let Ok(resp) = client.get_with_pcode(&path, Some(pcode)).await {
            if let Ok(body) = resp.text().await {
                if let Ok(values) = extract_metric_values(&body) {
                    if !values.is_empty() {
                        baseline.resp_time = Some(compute_stats(&values));
                    }
                }
            }
        }

        // Try server_cpu
        let path = format!(
            "/open/api/json/tag/server_cpu/cpu?stime={}&etime={}",
            stime, now
        );
        if let Ok(resp) = client.get_with_pcode(&path, Some(pcode)).await {
            if let Ok(body) = resp.text().await {
                if let Ok(values) = extract_metric_values(&body) {
                    if !values.is_empty() {
                        baseline.cpu = Some(compute_stats(&values));
                    }
                }
            }
        }

        // Try server_memory
        let path = format!(
            "/open/api/json/tag/server_memory/memory_pused?stime={}&etime={}",
            stime, now
        );
        if let Ok(resp) = client.get_with_pcode(&path, Some(pcode)).await {
            if let Ok(body) = resp.text().await {
                if let Ok(values) = extract_metric_values(&body) {
                    if !values.is_empty() {
                        baseline.memory = Some(compute_stats(&values));
                    }
                }
            }
        }
    }

    Ok(baseline)
}

fn extract_metric_values(json: &str) -> Result<Vec<f64>> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let mut values = Vec::new();

    // Try to extract from data array
    if let Some(data) = value.get("data").and_then(|d| d.as_array()) {
        for item in data {
            // Try common field names
            if let Some(v) = item.get("data").and_then(|d| d.as_f64()) {
                values.push(v);
            } else if let Some(v) = item.get("value").and_then(|d| d.as_f64()) {
                values.push(v);
            }
        }
    } else if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(v) = item.get("data").and_then(|d| d.as_f64()) {
                values.push(v);
            } else if let Some(v) = item.get("value").and_then(|d| d.as_f64()) {
                values.push(v);
            } else if let Some(v) = item.as_f64() {
                values.push(v);
            }
        }
    }

    Ok(values)
}

fn compute_stats(values: &[f64]) -> MetricBaseline {
    if values.is_empty() {
        return MetricBaseline { avg: 0.0, p50: 0.0, p95: 0.0 };
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let p50_idx = (sorted.len() as f64 * 0.50) as usize;
    let p95_idx = (sorted.len() as f64 * 0.95) as usize;

    MetricBaseline {
        avg,
        p50: sorted.get(p50_idx.min(sorted.len() - 1)).copied().unwrap_or(0.0),
        p95: sorted.get(p95_idx.min(sorted.len() - 1)).copied().unwrap_or(0.0),
    }
}
