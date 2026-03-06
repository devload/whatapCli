use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

use crate::core::client::WhatapClient;
use crate::core::error::CliError;
use crate::types::symbol::{SymbolFile, SymbolListResponse, SymbolType};

/// Discover files matching glob pattern in a directory
pub fn discover_files(
    dir: &Path,
    include: Option<&str>,
    exclude: Option<&str>,
    symbol_type: SymbolType,
) -> Result<Vec<PathBuf>> {
    let default_pattern = match symbol_type {
        SymbolType::Sourcemap => "**/*.map",
        SymbolType::Proguard => "**/mapping*.txt",
        SymbolType::Dsym => "**/*.dSYM/**",
    };

    let pattern = include.unwrap_or(default_pattern);
    let full_pattern = format!("{}/{}", dir.display(), pattern);

    let mut files: Vec<PathBuf> = glob::glob(&full_pattern)
        .with_context(|| format!("Invalid glob pattern: {}", full_pattern))?
        .filter_map(|entry| entry.ok())
        .filter(|path| path.is_file())
        .collect();

    // Apply exclude pattern
    if let Some(exclude_pat) = exclude {
        let exclude_full = format!("{}/{}", dir.display(), exclude_pat);
        let excluded: Vec<PathBuf> = glob::glob(&exclude_full)
            .unwrap_or_else(|_| glob::glob("").unwrap())
            .filter_map(|e| e.ok())
            .collect();
        files.retain(|f| !excluded.contains(f));
    }

    files.sort();
    Ok(files)
}

/// Upload symbol files in batches
pub async fn upload_files(
    client: &WhatapClient,
    pcode: i64,
    files: &[PathBuf],
    symbol_type: SymbolType,
    version: &str,
    host: Option<&str>,
    quiet: bool,
) -> Result<usize> {
    if files.is_empty() {
        return Err(CliError::Input("No files found to upload".to_string()).into());
    }

    let max_per_batch = symbol_type.max_files_per_upload();
    let max_size = symbol_type.max_file_size_mb() * 1024 * 1024;
    let mut uploaded = 0;
    let total = files.len();

    let pb = if !quiet {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  Uploading [{bar:30}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
        Some(pb)
    } else {
        None
    };

    // Process in batches
    for batch in files.chunks(max_per_batch) {
        let mut form = reqwest::multipart::Form::new();

        // Add metadata fields
        form = form.text("version", version.to_string());
        if let Some(h) = host {
            form = form.text("host", h.to_string());
        }

        for file_path in batch {
            let file_size = std::fs::metadata(file_path)?.len();
            if file_size > max_size {
                if let Some(pb) = &pb {
                    pb.println(format!(
                        "  ! Skipping {} ({}MB exceeds {}MB limit)",
                        file_path.display(),
                        file_size / (1024 * 1024),
                        symbol_type.max_file_size_mb()
                    ));
                }
                continue;
            }

            let file_name = file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let file_bytes = std::fs::read(file_path)
                .with_context(|| format!("Failed to read {}", file_path.display()))?;

            let mime = mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string();

            let part = reqwest::multipart::Part::bytes(file_bytes)
                .file_name(file_name.clone())
                .mime_str(&mime)?;

            // Field name depends on symbol type
            let field_name = match symbol_type {
                SymbolType::Sourcemap => "sourcemap",
                SymbolType::Proguard => "proguard",
                SymbolType::Dsym => "dsym",
            };
            form = form.part(field_name, part);

            if let Some(pb) = &pb {
                let size_str = format_size(file_size);
                pb.set_message(format!("{} ({})", file_name, size_str));
            }
        }

        let upload_path = format!(
            "/open/api/pcode/{}/{}",
            pcode,
            symbol_type.upload_path()
        );

        client.post_multipart(&upload_path, form).await?;
        uploaded += batch.len();

        if let Some(pb) = &pb {
            pb.set_position(uploaded as u64);
        }
    }

    if let Some(pb) = &pb {
        pb.finish_and_clear();
    }

    Ok(uploaded)
}

/// List uploaded symbol files
pub async fn list_files(
    client: &WhatapClient,
    pcode: i64,
    symbol_type: SymbolType,
) -> Result<Vec<SymbolFile>> {
    let path = format!(
        "/open/api/pcode/{}/{}",
        pcode,
        symbol_type.list_path()
    );
    let resp = client.get(&path).await?;
    let body = resp.text().await?;

    // Try parsing as SymbolListResponse
    if let Ok(list) = serde_json::from_str::<SymbolListResponse>(&body) {
        return Ok(list.files().to_vec());
    }

    // Try as direct array
    if let Ok(files) = serde_json::from_str::<Vec<SymbolFile>>(&body) {
        return Ok(files);
    }

    Ok(vec![])
}

/// Delete symbol files
pub async fn delete_files(
    client: &WhatapClient,
    pcode: i64,
    symbol_type: SymbolType,
    file: Option<&str>,
    version: Option<&str>,
) -> Result<()> {
    let path = format!(
        "/open/api/pcode/{}/{}",
        pcode,
        symbol_type.delete_path()
    );

    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(f) = file {
        params.push(("fileName", f));
    }
    if let Some(v) = version {
        params.push(("version", v));
    }

    client.post_form(&path, &params).await?;
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
