use colored::Colorize;
use tabled::{Table, Tabled};

/// Print data as table, JSON, CSV, or Markdown based on format
pub fn print_output<T: serde::Serialize + Tabled>(data: &[T], format: &str) {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(data).unwrap_or_default();
            println!("{}", json);
        }
        "csv" => {
            if data.is_empty() {
                return;
            }
            // Use tabled to get headers, then output CSV
            let table = Table::new(data).to_string();
            let lines: Vec<&str> = table.lines().collect();
            // Convert table format to CSV
            for line in &lines {
                let trimmed = line.trim_matches(|c| c == '│' || c == '┌' || c == '┐' || c == '└' || c == '┘' || c == '├' || c == '┤');
                if trimmed.contains('─') || trimmed.contains('┼') {
                    continue;
                }
                let csv_line: String = trimmed
                    .split('│')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(",");
                if !csv_line.is_empty() {
                    println!("{}", csv_line);
                }
            }
        }
        "markdown" => {
            if data.is_empty() {
                println!("No data found.");
                return;
            }
            print_markdown_table(data);
        }
        _ => {
            if data.is_empty() {
                println!("No data found.");
                return;
            }
            let table = Table::new(data).to_string();
            println!("{}", table);
        }
    }
}

/// Print data as a GitHub-flavored Markdown table
fn print_markdown_table<T: serde::Serialize + Tabled>(data: &[T]) {
    // Use tabled to render, then parse the ASCII table into markdown
    let table = Table::new(data).to_string();
    let lines: Vec<&str> = table.lines().collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in &lines {
        let trimmed = line.trim_matches(|c| c == '│' || c == '┌' || c == '┐' || c == '└' || c == '┘' || c == '├' || c == '┤');
        if trimmed.contains('─') || trimmed.contains('┼') {
            continue;
        }
        let cells: Vec<String> = trimmed
            .split('│')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return;
    }

    // Header row
    let header = &rows[0];
    println!("| {} |", header.join(" | "));

    // Separator row with alignment
    let separators: Vec<String> = header
        .iter()
        .map(|_| "---".to_string())
        .collect();
    println!("| {} |", separators.join(" | "));

    // Data rows
    for row in &rows[1..] {
        println!("| {} |", row.join(" | "));
    }
}

/// Print a single value as JSON or formatted text
pub fn print_value<T: serde::Serialize>(data: &T, format: &str) {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(data).unwrap_or_default();
            println!("{}", json);
        }
        _ => {
            let json = serde_json::to_string_pretty(data).unwrap_or_default();
            println!("{}", json);
        }
    }
}

/// Print success message
pub fn success(msg: &str) {
    println!("{} {}", "✓".green(), msg);
}

/// Print error message
pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}

/// Print warning message
pub fn warn(msg: &str) {
    eprintln!("{} {}", "!".yellow(), msg);
}

/// Print info message (suppressed in quiet mode)
pub fn info(msg: &str, quiet: bool) {
    if !quiet {
        println!("{}", msg);
    }
}
