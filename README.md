# whatap-cli

A command-line interface for the [WhatAp](https://whatap.io) monitoring platform, built in Rust.

## Features

- **Authentication**: Email/password login (via mobile API) and API key modes
- **Project Management**: List and inspect monitoring projects
- **MXQL Queries**: Execute MXQL queries via yard API with multiple input modes
- **Real-time Metrics**: Fetch current spot metrics (TPS, CPU, memory, etc.)
- **Time-series Stats**: Query historical metric statistics
- **Log Search**: Search application logs with filtering
- **Browser RUM**: Step-by-step analysis (page load, AJAX, resources, errors) and trace correlation
- **Symbol Upload**: Upload sourcemaps, ProGuard mappings, and dSYM files
- **Alert Management**: Create, list, enable/disable, export/import metric alerts
- **MCP-Ready**: Designed for integration with MCP servers for AI-driven query generation

## Installation

```bash
# Build from source
cargo build --release

# Binary at target/release/whatap
```

## Quick Start

```bash
# Login with email/password
whatap login -e user@example.com -p 'password'

# Login with API key (project-scoped)
whatap login --api-key YOUR_API_KEY --pcode 12345

# Show current auth info
whatap whoami

# List projects
whatap projects
```

## Commands

### Authentication

```bash
whatap login -e <email> -p <password>   # Email/password login
whatap login --api-key <key> --pcode <p> # API key login
whatap logout                            # Clear credentials
whatap logout --all                      # Clear all profiles
whatap whoami                            # Show current user
```

### Projects

```bash
whatap projects                          # List all projects
whatap projects --filter MOBILE          # Filter by platform
whatap info <pcode>                      # Project details
```

### Real-time Metrics (Spot)

```bash
# All metrics
whatap spot --pcode 12345

# Specific metrics
whatap spot --pcode 12345 --keys cpu,tps,resp_time

# JSON output
whatap spot --pcode 12345 --json
```

### Time-series Statistics (Stat)

```bash
# Query TPS trend
whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h

# Response time trend
whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 30m

# List available categories
whatap stat categories --pcode 12345
```

### Log Search

```bash
# Recent logs
whatap log search --pcode 12345 --duration 10m

# Search with keyword
whatap log search --pcode 12345 --keyword "error" --duration 1h

# Filter by level
whatap log search --pcode 12345 --level ERROR --duration 1h

# List log categories
whatap log categories --pcode 12345
```

### Browser RUM Analysis

#### Step Commands (Individual Data)

```bash
# Page load analysis
whatap step pageload --pcode 12345 --duration 1h
whatap step pageload --pcode 12345 --slow 3000 --duration 1h

# AJAX requests
whatap step ajax --pcode 12345 --duration 1h
whatap step ajax --pcode 12345 --errors --duration 1h

# Resources
whatap step resources --pcode 12345 --duration 1h
whatap step resources --pcode 12345 --type script --duration 1h

# JavaScript errors
whatap step errors --pcode 12345 --duration 1h
whatap step errors --pcode 12345 --type TypeError --duration 1h
```

#### Trace Commands (Correlated Data)

```bash
# Summary only
whatap trace /products --pcode 12345 --summary

# Specific category
whatap trace /products --pcode 12345 --only ajax
whatap trace /products --pcode 12345 --only errors

# Slow items only
whatap trace /products --pcode 12345 --slow 2000

# Output formats
whatap trace /products --pcode 12345 --json
whatap trace /products --pcode 12345 --csv
```

### MXQL Queries

Execute MXQL queries against the WhatAp yard API.

```bash
# Direct query (use \n for newlines)
whatap mxql --pcode 45452 "CATEGORY mobile_crash\nTAGLOAD\nSELECT"

# Category shorthand
whatap mxql --pcode 45452 --category app_counter

# From file
whatap mxql --pcode 45452 -f query.mxql

# Pipe from stdin
echo "CATEGORY mobile_crash
TAGLOAD
SELECT" | whatap mxql --pcode 45452

# MCP integration (structured JSON input)
whatap mxql --json --input-json '{"pcode":45452,"mql":"CATEGORY mobile_crash\nTAGLOAD\nSELECT","stime":1769000000000,"etime":1772100000000,"limit":100}'

# With time range and limit
whatap mxql --pcode 45452 --stime 1769000000000 --etime 1772100000000 --limit 50 --category mobile_crash
```

### Alert Management

```bash
# List alerts
whatap alert list --pcode 12345

# Create alert with conditions
whatap alert create --pcode 12345 \
  --title "High CPU" --category app_counter \
  --warning "cpu > 80" --critical "cpu > 95" \
  --message 'CPU: ${cpu}%' --stateful

# Create alert from JSON (MCP integration)
whatap alert create --pcode 12345 --input-json '{...}'

# Enable / Disable
whatap alert enable --pcode 12345 --id <event_id>
whatap alert disable --pcode 12345 --id <event_id>

# Delete
whatap alert delete --pcode 12345 --id <event_id>

# Export / Import (backup & restore)
whatap alert export --pcode 12345 -f alerts.json
whatap alert import --pcode 12345 -f alerts.json
whatap alert import --pcode 12345 -f alerts.json --overwrite
```

### Symbol Management

```bash
# Sourcemaps
whatap sourcemaps upload ./dist --pcode 123 --version 1.0.0
whatap sourcemaps list --pcode 123
whatap sourcemaps delete --pcode 123 --version 1.0.0

# ProGuard mappings
whatap proguard upload ./mapping.txt --pcode 123
whatap proguard list --pcode 123

# dSYM files
whatap dsym upload ./App.dSYM --pcode 123
whatap dsym list --pcode 123
```

## Global Options

```
--json         Output as JSON
--quiet        Suppress non-essential output
--verbose      Enable verbose logging
--profile      Auth profile to use (default: "default")
--server       Override server URL
--no-color     Disable colored output
```

## Configuration

Credentials are stored in `~/.whatap/credentials/<profile>.json`.

Project-level config can be placed in `.whataprc.yml`:

```yaml
pcode: 12345
server: https://service.whatap.io
```

## Documentation

### Usage Guides (Command Reference)
- [브라우저 사용가이드](브라우저_사용가이드.md) - Browser RUM command options
- [모바일 사용가이드](모바일_사용가이드.md) - Mobile app command options
- [APM 사용가이드](APM_사용가이드.md) - APM server command options
- [DB 사용가이드](DB_사용가이드.md) - Database monitoring command options

### Analysis Guides (Troubleshooting)
- [브라우저 분석가이드](브라우저_분석가이드.md) - Page load, JS errors, AJAX issues
- [모바일 분석가이드](모바일_분석가이드.md) - Crash, ANR, app startup issues
- [APM 분석가이드](APM_분석가이드.md) - TPS, response time, CPU/memory issues
- [DB 분석가이드](DB_분석가이드.md) - Slow queries, locks, connection pool issues

## MCP Integration

The `mxql` command is designed for programmatic use with MCP servers:

1. **Structured input**: `--input-json` accepts all query parameters as a single JSON object
2. **JSON output**: `--json` flag produces machine-parseable output
3. **Stdin support**: MCP can pipe MXQL queries via stdin
4. **Exit codes**: `0` success, `2` auth error, `4` API error, `6` input error

Example MCP workflow:
```
MCP Server (generates MXQL) --> whatap mxql --json --input-json '{...}' --> JSON result
```

## License

Private - WhatAp internal use.
