use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

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

fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 { return format!("{:.1}ms", ms); }
    if ms < 1000.0 { return format!("{:.0}ms", ms); }
    format!("{:.2}s", ms / 1000.0)
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

/// Browser step data - 리소스/AJAX 네트워크 요청 (크롬 네트워크 탭과 유사)
pub async fn resources(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    page_url: Option<String>,
    resource_type: Option<String>,
    slow: Option<u64>,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    limit: u64,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    // MXQL 쿼리 빌드
    let mut mql = "CATEGORY rum_resource_each_page\nTAGLOAD\nSELECT [page_group, request_host, request_path, type, resource_duration, resource_size, is3rdParty, time]".to_string();

    let mut filters = Vec::new();
    if let Some(ref url) = page_url {
        filters.push(format!("page_group like '%{}%'", url));
    }
    if let Some(ref rtype) = resource_type {
        filters.push(format!("type == '{}'", rtype));
    }
    if let Some(threshold) = slow {
        filters.push(format!("resource_duration > {}", threshold));
    }
    if !filters.is_empty() {
        mql.push_str(&format!("\nFILTER {{{}}}", filters.join(" && ")));
    }
    mql.push_str(&format!("\nLIMIT {}", limit));

    let now = now_millis();
    let (resolved_stime, resolved_etime) = resolve_time_range(stime, etime, &duration, now)?;

    if config.verbose {
        eprintln!("Step (resources) MXQL:\n{}", mql);
    }

    let request = build_yard_request(resolved_pcode, &mql, resolved_stime, resolved_etime);
    let result = client.yard_post(&request).await?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    display_resource_results(&result, config)?;
    Ok(())
}

/// AJAX 요청 조회 (API 호출 분석)
pub async fn ajax(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    page_url: Option<String>,
    error_only: bool,
    slow: Option<u64>,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    limit: u64,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    let mut mql = "CATEGORY rum_ajax_each_page\nTAGLOAD\nSELECT [page_group, request_host, request_path, ajax_count, ajax_5xx_count, ajax_4xx_count, ajax_duration, time]".to_string();

    let mut filters = Vec::new();
    if let Some(ref url) = page_url {
        filters.push(format!("page_group like '%{}%'", url));
    }
    if error_only {
        filters.push("ajax_5xx_count > 0".to_string());
    }
    if let Some(threshold) = slow {
        filters.push(format!("ajax_duration > {}", threshold));
    }
    if !filters.is_empty() {
        mql.push_str(&format!("\nFILTER {{{}}}", filters.join(" && ")));
    }
    mql.push_str(&format!("\nLIMIT {}", limit));

    let now = now_millis();
    let (resolved_stime, resolved_etime) = resolve_time_range(stime, etime, &duration, now)?;

    if config.verbose {
        eprintln!("Step (AJAX) MXQL:\n{}", mql);
    }

    let request = build_yard_request(resolved_pcode, &mql, resolved_stime, resolved_etime);
    let result = client.yard_post(&request).await?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    display_ajax_results(&result, config)?;
    Ok(())
}

/// 브라우저 JS 에러 상세 조회 (에러타입/메시지/페이지/브라우저별)
pub async fn errors(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    page_url: Option<String>,
    error_type: Option<String>,
    browser: Option<String>,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    limit: u64,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    let mut mql = "CATEGORY rum_error_total_each_page\nTAGLOAD\nSELECT [page_group, error_type, error_message, count, browser, os, device, @timestamp]".to_string();

    let mut filters = Vec::new();
    if let Some(ref url) = page_url {
        filters.push(format!("page_group like '%{}%'", url));
    }
    if let Some(ref etype) = error_type {
        filters.push(format!("error_type like '%{}%'", etype));
    }
    if let Some(ref b) = browser {
        filters.push(format!("browser like '%{}%'", b));
    }
    if !filters.is_empty() {
        mql.push_str(&format!("\nFILTER {{{}}}", filters.join(" && ")));
    }
    mql.push_str(&format!("\nLIMIT {}", limit));

    let now = now_millis();
    let (resolved_stime, resolved_etime) = resolve_time_range(stime, etime, &duration, now)?;

    if config.verbose {
        eprintln!("Step (errors) MXQL:\n{}", mql);
    }

    let request = build_yard_request(resolved_pcode, &mql, resolved_stime, resolved_etime);
    let result = client.yard_post(&request).await?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    display_error_results(&result, config)?;
    Ok(())
}

/// 페이지 로드 타이밍 분석 (워터폴 타이밍 분해)
pub async fn pageload(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    page_url: Option<String>,
    slow: Option<u64>,
    stime: Option<u64>,
    etime: Option<u64>,
    duration: Option<String>,
    limit: u64,
    raw: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let resolved_pcode = client.resolve_pcode(pcode)?;

    let mut mql = "CATEGORY rum_page_load_each_page\nTAGLOAD\nSELECT [page_group, page_load_count, page_load_duration, page_load_backend_time, page_load_frontend_time, page_load_firstbyte_time, page_load_render_time, page_load_dns_time, page_load_connect_time, page_load_ssl_time, page_load_download_time, time]".to_string();

    let mut filters = Vec::new();
    if let Some(ref url) = page_url {
        filters.push(format!("page_group like '%{}%'", url));
    }
    if let Some(threshold) = slow {
        filters.push(format!("page_load_duration > {}", threshold));
    }
    if !filters.is_empty() {
        mql.push_str(&format!("\nFILTER {{{}}}", filters.join(" && ")));
    }
    mql.push_str(&format!("\nLIMIT {}", limit));

    let now = now_millis();
    let (resolved_stime, resolved_etime) = resolve_time_range(stime, etime, &duration, now)?;

    if config.verbose {
        eprintln!("Step (pageload) MXQL:\n{}", mql);
    }

    let request = build_yard_request(resolved_pcode, &mql, resolved_stime, resolved_etime);
    let result = client.yard_post(&request).await?;

    if raw || config.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    display_pageload_results(&result, config)?;
    Ok(())
}

// ── Display helpers ──

fn display_resource_results(result: &serde_json::Value, config: &ResolvedConfig) -> Result<()> {
    let records = extract_records(result);
    if records.is_empty() {
        output::warn("No resource step data found");
        return Ok(());
    }

    output::info(&format!("{} resource entries", records.len()), config.quiet);

    let mut rows: Vec<ResourceRow> = Vec::new();
    for rec in &records {
        let page = get_str(rec, "page_group").unwrap_or("-");
        let time_ms = get_f64(rec, "time").unwrap_or(0.0) as u64;
        let host = get_str(rec, "request_host").unwrap_or("-");
        let path = get_str(rec, "request_path").unwrap_or("-");
        let url = format!("{}{}", host, path);
        let short_url = if url.len() > 50 {
            format!("{}...{}", &url[..25], &url[url.len()-20..])
        } else {
            url.to_string()
        };

        rows.push(ResourceRow {
            key: make_key(page, time_ms),
            resource_type: get_str(rec, "type").unwrap_or("-").to_string(),
            url: short_url,
            duration: get_f64(rec, "resource_duration").map(format_duration_ms).unwrap_or("-".into()),
            size: get_f64(rec, "resource_size").map(|s| format_size(s as u64)).unwrap_or("-".into()),
            is_3rd_party: get_str(rec, "is3rdParty").unwrap_or("-").to_string(),
        });
    }
    output::print_output(&rows, &config.output);
    Ok(())
}

fn display_ajax_results(result: &serde_json::Value, config: &ResolvedConfig) -> Result<()> {
    let records = extract_records(result);
    if records.is_empty() {
        output::warn("No AJAX step data found");
        return Ok(());
    }

    output::info(&format!("{} AJAX entries", records.len()), config.quiet);

    let mut rows: Vec<AjaxRow> = Vec::new();
    for rec in &records {
        let page = get_str(rec, "page_group").unwrap_or("-");
        let time_ms = get_f64(rec, "time").unwrap_or(0.0) as u64;
        let host = get_str(rec, "request_host").unwrap_or("-");
        let path = get_str(rec, "request_path").unwrap_or("-");
        let url = format!("{}{}", host, path);
        let short_url = if url.len() > 40 {
            format!("{}...{}", &url[..20], &url[url.len()-15..])
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
            key: make_key(page, time_ms),
            url: short_url,
            count: get_f64(rec, "ajax_count").map(|c| format!("{}", c as i64)).unwrap_or("-".into()),
            duration: get_f64(rec, "ajax_duration").map(format_duration_ms).unwrap_or("-".into()),
            errors: error_info,
        });
    }
    output::print_output(&rows, &config.output);
    Ok(())
}

fn display_error_results(result: &serde_json::Value, config: &ResolvedConfig) -> Result<()> {
    let records = extract_records(result);
    if records.is_empty() {
        output::warn("No browser errors found");
        return Ok(());
    }

    output::info(&format!("{} error entries", records.len()), config.quiet);

    let mut rows: Vec<ErrorRow> = Vec::new();
    for rec in &records {
        let page = get_str(rec, "page_group").unwrap_or("-");
        let time_ms = get_f64(rec, "time").unwrap_or(0.0) as u64;

        rows.push(ErrorRow {
            key: make_key(page, time_ms),
            error_type: get_str(rec, "error_type").unwrap_or("-").to_string(),
            message: truncate(get_str(rec, "error_message").unwrap_or("-"), 40),
            count: get_f64(rec, "error_count").map(|c| format!("{}", c as i64)).unwrap_or("-".into()),
            browser: get_str(rec, "browser").unwrap_or("-").to_string(),
            device: get_str(rec, "device").unwrap_or("-").to_string(),
        });
    }
    output::print_output(&rows, &config.output);
    Ok(())
}

fn display_pageload_results(result: &serde_json::Value, config: &ResolvedConfig) -> Result<()> {
    let records = extract_records(result);
    if records.is_empty() {
        output::warn("No page load data found");
        return Ok(());
    }

    output::info(&format!("{} page load entries", records.len()), config.quiet);

    // 워터폴 스타일 표시
    for rec in &records {
        let page = get_str(rec, "page_group").unwrap_or("unknown");
        let time_ms = get_u64(rec, "time").unwrap_or(0);
        let total = get_f64(rec, "page_load_duration").unwrap_or(0.0);
        let count = get_f64(rec, "page_load_count").unwrap_or(1.0) as i64;
        let key = make_key(page, time_ms);

        println!("\n  {} (count: {}, total: {})", key, count, format_duration_ms(total));
        println!("  {}", "-".repeat(60));

        // 타이밍 분해
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
                    let bar_len = ((ms / max_val) * 30.0) as usize;
                    let bar: String = "#".repeat(bar_len.min(30));
                    println!("  {:>10} {:>8} |{}", label, format_duration_ms(*ms), bar);
                }
            }
        }
    }
    println!();

    Ok(())
}

// ── Utility ──

fn resolve_time_range(
    stime: Option<u64>, etime: Option<u64>, duration: &Option<String>, now: u64,
) -> Result<(u64, u64)> {
    if let Some(dur_str) = duration {
        let dur_ms = parse_duration_ms(dur_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid duration '{}'. Use: 5m, 1h, 30s, 1d", dur_str))?;
        let e = etime.unwrap_or(now);
        let s = stime.unwrap_or(e - dur_ms);
        Ok((s, e))
    } else {
        let e = etime.unwrap_or(now);
        let s = stime.unwrap_or(e - 3600 * 1000);
        Ok((s, e))
    }
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

fn get_str<'a>(val: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    val.get(key).and_then(|v| v.as_str())
}

fn get_f64(val: &serde_json::Value, key: &str) -> Option<f64> {
    val.get(key).and_then(|v| v.as_f64())
}

fn get_u64(val: &serde_json::Value, key: &str) -> Option<u64> {
    val.get(key).and_then(|v| {
        if let Some(n) = v.as_u64() {
            Some(n)
        } else if let Some(n) = v.as_f64() {
            Some(n as u64)
        } else if let Some(n) = v.as_i64() {
            Some(n as u64)
        } else {
            None
        }
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}...", &s[..max.saturating_sub(3)]) }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { return format!("{}B", bytes); }
    if bytes < 1024 * 1024 { return format!("{:.1}KB", bytes as f64 / 1024.0); }
    format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
}

// ── Table Row Structs ──

fn make_key(page: &str, time_ms: u64) -> String {
    // time 하위 6자리로 키 생성 (예: /cart@123456)
    let time_suffix = time_ms % 1_000_000;
    format!("{}@{}", page, time_suffix)
}

#[derive(Serialize, Tabled)]
struct ResourceRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Type")]
    resource_type: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "3rd")]
    is_3rd_party: String,
}

#[derive(Serialize, Tabled)]
struct AjaxRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Count")]
    count: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(rename = "Errors")]
    errors: String,
}

#[derive(Serialize, Tabled)]
struct ErrorRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Error Type")]
    error_type: String,
    #[tabled(rename = "Message")]
    message: String,
    #[tabled(rename = "Count")]
    count: String,
    #[tabled(rename = "Browser")]
    browser: String,
    #[tabled(rename = "Device")]
    device: String,
}
