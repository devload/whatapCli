use anyhow::Result;

use crate::cli::output;
use crate::core::auth;
use crate::types::config::ResolvedConfig;

pub fn run(config: &ResolvedConfig, all: bool) -> Result<()> {
    if all {
        let count = auth::remove_all_credentials()?;
        if count > 0 {
            output::success(&format!("Removed {} credential(s)", count));
        } else {
            output::info("No credentials found.", config.quiet);
        }
    } else {
        let removed = auth::remove_credentials(&config.profile)?;
        if removed {
            output::success(&format!(
                "Logged out (profile: {})",
                config.profile
            ));
        } else {
            output::info(
                &format!("No credentials for profile '{}'", config.profile),
                config.quiet,
            );
        }
    }
    Ok(())
}
