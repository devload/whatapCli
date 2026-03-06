mod cli;
mod core;
mod types;

use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(name = "whatap", about = "WhatAp CLI - monitoring platform control", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Auth profile to use
    #[arg(long, global = true, default_value = "default")]
    profile: String,

    /// Override server URL
    #[arg(long, global = true)]
    server: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Login to WhatAp
    Login {
        /// Account email
        #[arg(short, long)]
        email: Option<String>,
        /// Account password
        #[arg(short, long)]
        password: Option<String>,
        /// Project API key (CI/CD recommended)
        #[arg(long)]
        api_key: Option<String>,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Server URL override
        #[arg(short, long)]
        server: Option<String>,
    },

    /// Clear stored credentials
    Logout {
        /// Remove all profiles
        #[arg(long)]
        all: bool,
    },

    /// Show current user info
    Whoami,

    /// List accessible projects
    Projects {
        /// Filter by platform type (BROWSER, MOBILE, APM)
        #[arg(long)]
        filter: Option<String>,
    },

    /// Show project details
    Info {
        /// Project code
        pcode: i64,
    },

    /// Execute MXQL query via yard API
    Mxql {
        /// MXQL query string (use \n for newlines)
        query: Option<String>,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Read MXQL from file
        #[arg(short, long)]
        file: Option<String>,
        /// Full query params as JSON (for MCP integration)
        #[arg(long)]
        input_json: Option<String>,
        /// Start time (epoch ms, default: 24h ago)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms, default: now)
        #[arg(long)]
        etime: Option<u64>,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: u64,
        /// Shorthand: auto-build query with CATEGORY + TAGLOAD + SELECT
        #[arg(long)]
        category: Option<String>,
    },

    /// Manage browser sourcemaps
    Sourcemaps {
        #[command(subcommand)]
        action: SourcemapAction,
    },

    /// Manage Android ProGuard mappings
    Proguard {
        #[command(subcommand)]
        action: ProguardAction,
    },

    /// Manage iOS dSYM files
    Dsym {
        #[command(subcommand)]
        action: DsymAction,
    },
}

#[derive(Subcommand)]
enum SourcemapAction {
    /// Upload sourcemap files
    Upload {
        /// Directory containing .map files
        #[arg(default_value = "./dist")]
        path: String,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Host identifier
        #[arg(long, default_value = "default")]
        host: String,
        /// Version identifier
        #[arg(long, default_value = "default")]
        version: String,
        /// Glob pattern for files to include
        #[arg(long)]
        include: Option<String>,
        /// Glob pattern for files to exclude
        #[arg(long)]
        exclude: Option<String>,
        /// Show what would be uploaded without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// List uploaded sourcemaps
    List {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
    },
    /// Delete sourcemaps
    Delete {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Specific file to delete
        #[arg(long)]
        file: Option<String>,
        /// Version to delete
        #[arg(long)]
        version: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum ProguardAction {
    /// Upload ProGuard mapping file
    Upload {
        /// Path to mapping.txt or directory
        #[arg(default_value = "app/build/outputs/mapping/release/mapping.txt")]
        path: String,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Version code
        #[arg(long, default_value = "default")]
        version: String,
        /// Show what would be uploaded without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// List uploaded ProGuard mappings
    List {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
    },
    /// Delete ProGuard mappings
    Delete {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Specific file to delete
        #[arg(long)]
        file: Option<String>,
        /// Version to delete
        #[arg(long)]
        version: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum DsymAction {
    /// Upload dSYM files
    Upload {
        /// Path to .dSYM file or directory
        path: String,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Version identifier
        #[arg(long, default_value = "default")]
        version: String,
        /// Show what would be uploaded without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// List uploaded dSYM files
    List {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
    },
    /// Delete dSYM files
    Delete {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Specific file to delete
        #[arg(long)]
        file: Option<String>,
        /// Version to delete
        #[arg(long)]
        version: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }

    let config = match core::config::resolve_config(
        &cli.profile,
        cli.server.as_deref(),
        cli.json,
        cli.quiet,
        cli.verbose,
        cli.no_color,
    ) {
        Ok(c) => c,
        Err(e) => {
            cli::output::error(&format!("Configuration error: {}", e));
            process::exit(3);
        }
    };

    let result = match cli.command {
        Commands::Login {
            email,
            password,
            api_key,
            pcode,
            server,
        } => {
            cli::commands::login::run(&config, email, password, api_key, pcode, server).await
        }

        Commands::Logout { all } => cli::commands::logout::run(&config, all),

        Commands::Whoami => cli::commands::whoami::run(&config),

        Commands::Projects { filter } => {
            cli::commands::projects::run(&config, filter).await
        }

        Commands::Info { pcode } => {
            cli::commands::info::run(&config, pcode).await
        }

        Commands::Mxql {
            query,
            pcode,
            file,
            input_json,
            stime,
            etime,
            limit,
            category,
        } => {
            cli::commands::mxql::run(
                &config,
                pcode,
                query,
                file,
                input_json,
                stime,
                etime,
                limit,
                category,
            )
            .await
        }

        Commands::Sourcemaps { action } => match action {
            SourcemapAction::Upload {
                path,
                pcode,
                host,
                version,
                include,
                exclude,
                dry_run,
            } => {
                cli::commands::sourcemaps::upload(
                    &config,
                    &path,
                    pcode,
                    &host,
                    &version,
                    include.as_deref(),
                    exclude.as_deref(),
                    dry_run,
                )
                .await
            }
            SourcemapAction::List { pcode } => {
                cli::commands::sourcemaps::list(&config, pcode).await
            }
            SourcemapAction::Delete {
                pcode,
                file,
                version,
                confirm,
            } => {
                cli::commands::sourcemaps::delete(
                    &config,
                    pcode,
                    file.as_deref(),
                    version.as_deref(),
                    confirm,
                )
                .await
            }
        },

        Commands::Proguard { action } => match action {
            ProguardAction::Upload {
                path,
                pcode,
                version,
                dry_run,
            } => {
                cli::commands::proguard::upload(&config, &path, pcode, &version, dry_run).await
            }
            ProguardAction::List { pcode } => {
                cli::commands::proguard::list(&config, pcode).await
            }
            ProguardAction::Delete {
                pcode,
                file,
                version,
                confirm,
            } => {
                cli::commands::proguard::delete(
                    &config,
                    pcode,
                    file.as_deref(),
                    version.as_deref(),
                    confirm,
                )
                .await
            }
        },

        Commands::Dsym { action } => match action {
            DsymAction::Upload {
                path,
                pcode,
                version,
                dry_run,
            } => {
                cli::commands::dsym::upload(&config, &path, pcode, &version, dry_run).await
            }
            DsymAction::List { pcode } => {
                cli::commands::dsym::list(&config, pcode).await
            }
            DsymAction::Delete {
                pcode,
                file,
                version,
                confirm,
            } => {
                cli::commands::dsym::delete(
                    &config,
                    pcode,
                    file.as_deref(),
                    version.as_deref(),
                    confirm,
                )
                .await
            }
        },
    };

    if let Err(e) = result {
        // Show root cause, then context chain if verbose
        let chain: Vec<String> = e.chain().map(|c| c.to_string()).collect();
        if chain.len() > 1 {
            // Show most specific error (last in chain = root cause)
            cli::output::error(chain.last().unwrap());
            if config.verbose {
                for ctx in &chain[..chain.len() - 1] {
                    eprintln!("  caused by: {}", ctx);
                }
            }
        } else {
            cli::output::error(&e.to_string());
        }

        // Determine exit code from error type
        let code = if let Some(cli_err) = e.downcast_ref::<core::error::CliError>() {
            cli_err.exit_code()
        } else {
            1
        };
        process::exit(code);
    }
}
