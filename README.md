# whatap-cli

A command-line interface for the [WhatAp](https://whatap.io) monitoring platform, built in Rust.

## Features

- **Authentication**: Email/password login (via mobile API) and API key modes
- **Project Management**: List and inspect monitoring projects
- **MXQL Queries**: Execute MXQL queries via yard API with multiple input modes
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

**Options:**

| Option | Description |
|--------|-------------|
| `--pcode <PCODE>` | Project code |
| `--stime <MS>` | Start time in epoch ms (default: 24h ago) |
| `--etime <MS>` | End time in epoch ms (default: now) |
| `--limit <N>` | Max results (default: 100) |
| `--category <CAT>` | Shorthand for `CATEGORY <CAT>\nTAGLOAD\nSELECT` |
| `-f, --file <PATH>` | Read MXQL from file |
| `--input-json <JSON>` | Full query params as JSON (for MCP) |
| `--json` | Output as JSON |

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
