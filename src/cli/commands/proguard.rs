use anyhow::Result;
use std::path::{Path, PathBuf};
use tabled::Tabled;

use crate::cli::output;
use crate::core::client::WhatapClient;
use crate::core::symbol;
use crate::types::config::ResolvedConfig;
use crate::types::symbol::SymbolType;

#[derive(Tabled, serde::Serialize)]
struct SymbolRow {
    #[tabled(rename = "File")]
    file: String,
    #[tabled(rename = "Version")]
    version: String,
    #[tabled(rename = "Size")]
    size: String,
}

pub async fn upload(
    config: &ResolvedConfig,
    path: &str,
    pcode: Option<i64>,
    version: &str,
    dry_run: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(crate::core::error::CliError::FileNotFound(path.to_string()).into());
    }

    let files: Vec<PathBuf> = if file_path.is_file() {
        vec![file_path.to_path_buf()]
    } else {
        symbol::discover_files(file_path, None, None, SymbolType::Proguard)?
    };

    output::info(
        &format!("Found {} ProGuard mapping file(s)", files.len()),
        config.quiet,
    );

    if dry_run {
        for f in &files {
            println!("  {}", f.display());
        }
        output::info("(dry run, no files uploaded)", config.quiet);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let uploaded = symbol::upload_files(
        &client,
        pcode,
        &files,
        SymbolType::Proguard,
        version,
        None,
        config.quiet,
    )
    .await?;

    let elapsed = start.elapsed();
    output::success(&format!(
        "Uploaded {} file(s) in {:.1}s",
        uploaded,
        elapsed.as_secs_f64()
    ));

    Ok(())
}

pub async fn list(config: &ResolvedConfig, pcode: Option<i64>) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    let files = symbol::list_files(&client, pcode, SymbolType::Proguard).await?;

    let rows: Vec<SymbolRow> = files
        .iter()
        .map(|f| SymbolRow {
            file: f.name().to_string(),
            version: f.version.clone().unwrap_or_else(|| "-".to_string()),
            size: f
                .file_size
                .map(|s| format_size(s as u64))
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    output::info(
        &format!("Found {} ProGuard mapping(s) for pcode {}", rows.len(), pcode),
        config.quiet,
    );
    output::print_output(&rows, &config.output);

    Ok(())
}

pub async fn delete(
    config: &ResolvedConfig,
    pcode: Option<i64>,
    file: Option<&str>,
    version: Option<&str>,
    confirm: bool,
) -> Result<()> {
    let client = WhatapClient::new(config.clone())?;
    let pcode = client.resolve_pcode(pcode)?;

    if !confirm {
        let target = file.or(version).unwrap_or("all");
        output::warn(&format!(
            "This will delete ProGuard mapping(s) matching '{}' for pcode {}. Use --confirm to proceed.",
            target, pcode
        ));
        return Ok(());
    }

    symbol::delete_files(&client, pcode, SymbolType::Proguard, file, version).await?;
    output::success("ProGuard mapping(s) deleted");

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
