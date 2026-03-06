use anyhow::{Context, Result};

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::types::config::ResolvedConfig;

pub async fn run(config: &ResolvedConfig, pcode: i64) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let projects = client.list_projects().await?;

    let project = projects
        .iter()
        .find(|p| p.project_code == pcode)
        .with_context(|| format!("Project with code {} not found", pcode))?;

    output::print_value(project, &config.output);

    Ok(())
}
