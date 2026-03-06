use anyhow::Result;
use std::io::{self, Write};

use crate::cli::output;
use crate::core::auth;
use crate::types::auth::{AuthMode, Credentials};
use crate::types::config::ResolvedConfig;

pub async fn run(
    config: &ResolvedConfig,
    email: Option<String>,
    password: Option<String>,
    api_key: Option<String>,
    pcode: Option<i64>,
    server: Option<String>,
) -> Result<()> {
    let server_url = server
        .as_deref()
        .unwrap_or(&config.server);

    // API Key mode
    if let Some(key) = api_key {
        let creds = Credentials {
            auth_mode: AuthMode::ApiKey,
            session: None,
            api_key: Some(key),
            pcode,
            server: Some(server_url.to_string()),
        };
        auth::save_credentials(&config.profile, &creds)?;
        output::success(&format!(
            "Logged in with API key (profile: {})",
            config.profile
        ));
        if let Some(pc) = pcode {
            output::info(&format!("  Project code: {}", pc), config.quiet);
        }
        return Ok(());
    }

    // Email/password mode
    let email = match email.or_else(|| std::env::var("WHATAP_EMAIL").ok()) {
        Some(e) => e,
        None => {
            print!("Email: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    let password = match password.or_else(|| std::env::var("WHATAP_PASSWORD").ok()) {
        Some(p) => p,
        None => rpassword::prompt_password("Password: ")?,
    };

    output::info(
        &format!("Logging in to {} ...", server_url),
        config.quiet,
    );

    let session = auth::login_email_password(server_url, &email, &password).await?;

    let creds = Credentials {
        auth_mode: AuthMode::EmailPassword,
        session: Some(session),
        api_key: None,
        pcode,
        server: Some(server_url.to_string()),
    };
    auth::save_credentials(&config.profile, &creds)?;

    output::success(&format!(
        "Logged in as {} (profile: {})",
        email, config.profile
    ));
    Ok(())
}
