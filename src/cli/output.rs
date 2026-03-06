use colored::Colorize;
use tabled::{Table, Tabled};

/// Print data as table, JSON, or CSV based on format
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
