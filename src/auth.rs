use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const DEFAULT_API_BASE_URL: &str = "https://api.apitally.io";

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

fn auth_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".apitally"))
}

fn auth_file_path() -> Result<PathBuf> {
    Ok(auth_dir()?.join("auth.json"))
}

pub fn load_auth_config() -> Result<Option<AuthConfig>> {
    let path = auth_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: AuthConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(config))
}

fn save_auth_config(config: &AuthConfig) -> Result<()> {
    let dir = auth_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let path = auth_file_path()?;
    let json = serde_json::to_string_pretty(config)?;
    fs::write(&path, &json).with_context(|| format!("Failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}

/// Resolve API key with precedence: --api-key flag > APITALLY_API_KEY env > auth.json
pub fn resolve_api_key(cli_api_key: Option<&str>) -> Result<String> {
    if let Some(key) = cli_api_key {
        return Ok(key.to_string());
    }
    if let Ok(key) = std::env::var("APITALLY_API_KEY")
        && !key.is_empty()
    {
        return Ok(key);
    }
    if let Some(config) = load_auth_config()? {
        return Ok(config.api_key);
    }
    bail!(
        "No API key configured.\n\n\
         Run `apitally auth` to set up authentication, or provide --api-key / APITALLY_API_KEY."
    );
}

/// Resolve API base URL with precedence: --api-base-url flag > APITALLY_API_BASE_URL env > auth.json > default
pub fn resolve_api_base_url(cli_url: Option<&str>) -> String {
    if let Some(url) = cli_url {
        return url.to_string();
    }
    if let Ok(url) = std::env::var("APITALLY_API_BASE_URL")
        && !url.is_empty()
    {
        return url;
    }
    if let Ok(Some(config)) = load_auth_config()
        && let Some(url) = config.api_base_url
    {
        return url;
    }
    DEFAULT_API_BASE_URL.to_string()
}

pub fn run(api_key: Option<String>, api_base_url: Option<String>) -> Result<()> {
    let api_key = match api_key {
        Some(key) => key,
        None => prompt_api_key()?,
    };
    save_auth_config(&AuthConfig {
        api_key,
        api_base_url,
    })?;
    eprintln!("Authentication configured successfully.");
    Ok(())
}

fn prompt_api_key() -> Result<String> {
    eprintln!("To get your API key, go to https://app.apitally.io/settings/api-keys");
    eprintln!();
    eprint!("API key: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let key = input.trim().to_string();
    if key.is_empty() {
        bail!("API key cannot be empty.");
    }
    Ok(key)
}
