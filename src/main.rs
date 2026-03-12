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

    /// Create a new monitoring project
    #[command(name = "project")]
    Project {
        #[command(subcommand)]
        action: ProjectAction,
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

    /// Fetch current spot metrics (real-time counters)
    Spot {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Filter specific metric keys (comma-separated, e.g. "cpu,tps,actx")
        #[arg(long)]
        keys: Option<String>,
    },

    /// Fetch integrated analysis snapshot for AI (spot + metrics + issues)
    Snapshot {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Duration lookback (e.g. "1h", "30m", "1d")
        #[arg(short, long, default_value = "1h")]
        duration: String,
    },

    /// Query time-series metric statistics
    Stat {
        #[command(subcommand)]
        action: StatAction,
    },

    /// Search application logs
    Log {
        #[command(subcommand)]
        action: LogAction,
    },

    /// Browser step data analysis (resources, AJAX, errors, page load)
    Step {
        #[command(subcommand)]
        action: StepAction,
    },

    /// Trace correlated data using key from step commands
    Trace {
        /// Key from step command output (e.g. /cart@123456)
        key: String,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Duration lookback (e.g. "1h", "30m", "1d")
        #[arg(short, long, default_value = "1h")]
        duration: String,
        /// Show only specific category (pageload, ajax, resources, errors)
        #[arg(long)]
        only: Option<String>,
        /// Show only slow items (duration > threshold ms)
        #[arg(long)]
        slow: Option<u64>,
        /// Show summary only (counts, no details)
        #[arg(long)]
        summary: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Output as CSV
        #[arg(long)]
        csv: bool,
        /// Output raw JSON from API
        #[arg(long)]
        raw: bool,
    },

    /// Manage metric alerts
    Alert {
        #[command(subcommand)]
        action: AlertAction,
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

#[derive(Subcommand)]
enum ProjectAction {
    /// List accessible projects
    List {
        /// Filter by platform or name
        #[arg(long)]
        filter: Option<String>,
    },
    /// Create a new monitoring project
    Create {
        /// Project name
        #[arg(long)]
        name: String,
        /// Platform type (java, nodejs, python, php, dotnet, go, kubernetes, server, browser, android, ios)
        #[arg(long)]
        platform: String,
        /// Group ID to assign the project to
        #[arg(long)]
        group_id: Option<i64>,
    },
    /// Delete a project
    Delete {
        /// Project code to delete
        pcode: i64,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum StatAction {
    /// Query time-series data for a specific metric
    Query {
        /// Metric category (e.g. app_counter, server_cpu, rum_page_load_each_page)
        #[arg(long)]
        category: String,
        /// Metric field (e.g. tps, resp_time, cpu, load_time)
        #[arg(long)]
        field: String,
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback (e.g. "5m", "1h", "30s", "1d")
        #[arg(short, long)]
        duration: Option<String>,
        /// Output raw JSON response
        #[arg(long)]
        raw: bool,
    },
    /// List available stat categories and fields
    Categories {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
    },
}

#[derive(Subcommand)]
enum LogAction {
    /// Search log entries
    Search {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Search keyword (filters message field)
        #[arg(short, long)]
        keyword: Option<String>,
        /// Log level filter (ERROR, WARN, INFO, DEBUG)
        #[arg(short, long)]
        level: Option<String>,
        /// Log category (default: app_log)
        #[arg(long)]
        category: Option<String>,
        /// Custom SELECT fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback (e.g. "5m", "1h", "30s", "1d")
        #[arg(short, long)]
        duration: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: u64,
        /// Output raw JSON response
        #[arg(long)]
        raw: bool,
    },
    /// List available log categories
    Categories,
}

#[derive(Subcommand)]
enum StepAction {
    /// Query browser resource loading data (images, scripts, CSS, fonts)
    Resources {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Filter by page URL
        #[arg(long)]
        page: Option<String>,
        /// Filter by resource type (script, image, link, font, xhr, fetch)
        #[arg(long, name = "type")]
        resource_type: Option<String>,
        /// Show only slow resources (duration > threshold ms)
        #[arg(long)]
        slow: Option<u64>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback (e.g. "1h", "30m", "1d")
        #[arg(short, long)]
        duration: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: u64,
        /// Output raw JSON
        #[arg(long)]
        raw: bool,
    },
    /// Query AJAX/API request data
    Ajax {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Filter by page URL
        #[arg(long)]
        page: Option<String>,
        /// Show only requests with errors
        #[arg(long)]
        errors: bool,
        /// Show only slow requests (time > threshold ms)
        #[arg(long)]
        slow: Option<u64>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback
        #[arg(short, long)]
        duration: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: u64,
        /// Output raw JSON
        #[arg(long)]
        raw: bool,
    },
    /// Query browser JavaScript errors
    Errors {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Filter by page URL
        #[arg(long)]
        page: Option<String>,
        /// Filter by error type (TypeError, ReferenceError, SyntaxError, etc.)
        #[arg(long, name = "type")]
        error_type: Option<String>,
        /// Filter by browser
        #[arg(long)]
        browser: Option<String>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback
        #[arg(short, long)]
        duration: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: u64,
        /// Output raw JSON
        #[arg(long)]
        raw: bool,
    },
    /// Query page load timing breakdown (waterfall)
    Pageload {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Filter by page URL
        #[arg(long)]
        page: Option<String>,
        /// Show only slow pages (loadTime > threshold ms)
        #[arg(long)]
        slow: Option<u64>,
        /// Start time (epoch ms)
        #[arg(long)]
        stime: Option<u64>,
        /// End time (epoch ms)
        #[arg(long)]
        etime: Option<u64>,
        /// Duration lookback
        #[arg(short, long)]
        duration: Option<String>,
        /// Max results
        #[arg(long, default_value = "10")]
        limit: u64,
        /// Output raw JSON
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand)]
enum AlertAction {
    /// List metric alerts
    List {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
    },
    /// Create a metric alert
    Create {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Alert title
        #[arg(long)]
        title: Option<String>,
        /// Metric category (e.g. app_counter, server_cpu)
        #[arg(long)]
        category: Option<String>,
        /// Warning condition rule (e.g. "cpu > 80")
        #[arg(long)]
        warning: Option<String>,
        /// Critical condition rule (e.g. "cpu > 95")
        #[arg(long)]
        critical: Option<String>,
        /// Info condition rule
        #[arg(long)]
        info: Option<String>,
        /// Alert message template
        #[arg(long)]
        message: Option<String>,
        /// Track alert resolution
        #[arg(long)]
        stateful: bool,
        /// Filter expression
        #[arg(long)]
        select: Option<String>,
        /// Repeat count before alerting
        #[arg(long, default_value = "1")]
        repeat_count: u64,
        /// Duration between repeats (seconds)
        #[arg(long, default_value = "0")]
        repeat_duration: u64,
        /// Silent period after alert (seconds)
        #[arg(long, default_value = "0")]
        silent: u64,
        /// Full alert as JSON (for MCP integration)
        #[arg(long)]
        input_json: Option<String>,
    },
    /// Delete a metric alert
    Delete {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Event ID to delete
        #[arg(long)]
        id: String,
        /// Alert category
        #[arg(long)]
        category: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
    /// Enable an alert
    Enable {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Event ID
        #[arg(long)]
        id: String,
    },
    /// Disable an alert
    Disable {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Event ID
        #[arg(long)]
        id: String,
    },
    /// Export alerts to JSON
    Export {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        file: Option<String>,
    },
    /// Import alerts from JSON file
    Import {
        /// Project code
        #[arg(long)]
        pcode: Option<i64>,
        /// JSON file containing alert definitions
        #[arg(short, long)]
        file: String,
        /// Overwrite existing alerts
        #[arg(long)]
        overwrite: bool,
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

        Commands::Project { action } => match action {
            ProjectAction::List { filter } => {
                cli::commands::project::list(&config, filter).await
            }
            ProjectAction::Create {
                name,
                platform,
                group_id,
            } => {
                cli::commands::project::create(&config, name, platform, group_id).await
            }
            ProjectAction::Delete { pcode, confirm } => {
                cli::commands::project::delete(&config, pcode, confirm).await
            }
        },

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

        Commands::Spot { pcode, keys } => {
            cli::commands::spot::run(&config, pcode, keys).await
        }

        Commands::Snapshot { pcode, duration } => {
            cli::commands::snapshot::run(&config, pcode, Some(duration)).await
        }

        Commands::Stat { action } => match action {
            StatAction::Query {
                category,
                field,
                pcode,
                stime,
                etime,
                duration,
                raw,
            } => {
                cli::commands::stat::run(
                    &config, pcode, category, field, stime, etime, duration, raw,
                )
                .await
            }
            StatAction::Categories { pcode } => {
                cli::commands::stat::categories(&config, pcode).await
            }
        },

        Commands::Log { action } => match action {
            LogAction::Search {
                pcode,
                keyword,
                level,
                category,
                fields,
                stime,
                etime,
                duration,
                limit,
                raw,
            } => {
                cli::commands::log::search(
                    &config, pcode, keyword, level, category, fields, stime, etime, duration,
                    limit, raw,
                )
                .await
            }
            LogAction::Categories => {
                cli::commands::log::categories(&config).await
            }
        },

        Commands::Step { action } => match action {
            StepAction::Resources {
                pcode, page, resource_type, slow, stime, etime, duration, limit, raw,
            } => {
                cli::commands::step::resources(
                    &config, pcode, page, resource_type, slow, stime, etime, duration, limit, raw,
                ).await
            }
            StepAction::Ajax {
                pcode, page, errors, slow, stime, etime, duration, limit, raw,
            } => {
                cli::commands::step::ajax(
                    &config, pcode, page, errors, slow, stime, etime, duration, limit, raw,
                ).await
            }
            StepAction::Errors {
                pcode, page, error_type, browser, stime, etime, duration, limit, raw,
            } => {
                cli::commands::step::errors(
                    &config, pcode, page, error_type, browser, stime, etime, duration, limit, raw,
                ).await
            }
            StepAction::Pageload {
                pcode, page, slow, stime, etime, duration, limit, raw,
            } => {
                cli::commands::step::pageload(
                    &config, pcode, page, slow, stime, etime, duration, limit, raw,
                ).await
            }
        },

        Commands::Trace { key, pcode, duration, only, slow, summary, json, csv, raw } => {
            cli::commands::trace::run(&config, pcode, &key, &duration, only.as_deref(), slow, summary, json, csv, raw).await
        },

        Commands::Alert { action } => match action {
            AlertAction::List { pcode } => {
                cli::commands::alert::list(&config, pcode).await
            }
            AlertAction::Create {
                pcode,
                title,
                category,
                warning,
                critical,
                info,
                message,
                stateful,
                select,
                repeat_count,
                repeat_duration,
                silent,
                input_json,
            } => {
                cli::commands::alert::create(
                    &config,
                    pcode,
                    title,
                    category,
                    warning,
                    critical,
                    info,
                    message,
                    stateful,
                    select,
                    repeat_count,
                    repeat_duration,
                    silent,
                    input_json,
                )
                .await
            }
            AlertAction::Delete {
                pcode,
                id,
                category,
                confirm,
            } => {
                cli::commands::alert::delete(&config, pcode, &id, category, confirm)
                    .await
            }
            AlertAction::Enable { pcode, id } => {
                cli::commands::alert::toggle(&config, pcode, &id, true).await
            }
            AlertAction::Disable { pcode, id } => {
                cli::commands::alert::toggle(&config, pcode, &id, false).await
            }
            AlertAction::Export { pcode, file } => {
                cli::commands::alert::export(&config, pcode, file).await
            }
            AlertAction::Import {
                pcode,
                file,
                overwrite,
            } => {
                cli::commands::alert::import(&config, pcode, &file, overwrite).await
            }
        },

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
