use anyhow::Result;
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

pub async fn run(config: &ResolvedConfig, filter: Option<String>) -> Result<()> {
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

    // Apply filter
    if let Some(f) = &filter {
        let f_upper = f.to_uppercase();
        rows.retain(|r| r.platform.to_uppercase().contains(&f_upper));
    }

    output::info(
        &format!("Found {} project(s)", rows.len()),
        config.quiet,
    );
    output::print_output(&rows, &config.output);

    Ok(())
}
