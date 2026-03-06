use anyhow::Result;

use crate::cli::output;
use crate::core::auth;
use crate::types::auth::{AuthMode, WhoamiInfo};
use crate::types::config::ResolvedConfig;

pub fn run(config: &ResolvedConfig) -> Result<()> {
    let creds = auth::load_credentials(&config.profile)?;

    let info = WhoamiInfo {
        email: match &creds.auth_mode {
            AuthMode::EmailPassword => creds
                .session
                .as_ref()
                .map(|s| s.email.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            AuthMode::ApiKey => "(API key mode)".to_string(),
        },
        auth_mode: match &creds.auth_mode {
            AuthMode::EmailPassword => "email/password".to_string(),
            AuthMode::ApiKey => "api_key".to_string(),
        },
        server: creds
            .server
            .clone()
            .unwrap_or_else(|| config.server.clone()),
        profile: config.profile.clone(),
        pcode: creds.pcode,
    };

    if config.json {
        output::print_value(&info, "json");
    } else {
        println!("Profile:   {}", info.profile);
        println!("Email:     {}", info.email);
        println!("Auth mode: {}", info.auth_mode);
        println!("Server:    {}", info.server);
        if let Some(pcode) = info.pcode {
            println!("Pcode:     {}", pcode);
        }
    }

    Ok(())
}
