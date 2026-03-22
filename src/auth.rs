use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::utils::auth_err;

const DEFAULT_API_BASE_URL: &str = "https://api.apitally.io";

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

pub fn auth_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".apitally").join("auth.json"))
}

fn load_auth_file(path: &Path) -> Result<Option<AuthConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: AuthConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(config))
}

fn save_auth_file(path: &Path, config: &AuthConfig) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, &json).with_context(|| format!("Failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}

fn pick_api_key(api_key: Option<&str>, config: Option<&AuthConfig>) -> Result<String> {
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        return Ok(key.to_string());
    }
    if let Some(key) = config.map(|c| c.api_key.trim()).filter(|k| !k.is_empty()) {
        return Ok(key.to_string());
    }
    Err(auth_err(
        "No API key configured.\n\n\
         Run `apitally auth` to set up authentication, or provide --api-key / APITALLY_API_KEY.",
    ))
}

fn pick_api_base_url(api_base_url: Option<&str>, config: Option<&AuthConfig>) -> String {
    if let Some(url) = api_base_url.map(str::trim).filter(|u| !u.is_empty()) {
        return url.to_string();
    }
    if let Some(url) = config
        .and_then(|c| c.api_base_url.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        return url.to_string();
    }
    DEFAULT_API_BASE_URL.to_string()
}

/// Resolve API key with precedence: --api-key flag / APITALLY_API_KEY env (via clap) > auth.json
pub fn resolve_api_key(api_key: Option<&str>) -> Result<String> {
    let config = load_auth_file(&auth_file_path()?)?;
    pick_api_key(api_key, config.as_ref())
}

/// Resolve API base URL with precedence: --api-base-url flag / APITALLY_API_BASE_URL env (via clap) > auth.json > default
pub fn resolve_api_base_url(api_base_url: Option<&str>) -> String {
    let config = auth_file_path()
        .and_then(|p| load_auth_file(&p))
        .unwrap_or(None);
    pick_api_base_url(api_base_url, config.as_ref())
}

pub fn run(
    api_key: Option<String>,
    api_base_url: Option<String>,
    auth_file_path: &Path,
    input: &mut impl io::Read,
) -> Result<()> {
    let api_key = match api_key {
        Some(key) => key,
        None => prompt_api_key(input)?,
    };
    save_auth_file(
        auth_file_path,
        &AuthConfig {
            api_key,
            api_base_url,
        },
    )?;
    eprintln!("Authentication configured successfully.");
    Ok(())
}

fn prompt_api_key(input: &mut impl io::Read) -> Result<String> {
    eprintln!("To get your API key, go to https://app.apitally.io/settings/api-keys");
    eprintln!();
    eprint!("API key: ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::BufReader::new(input).read_line(&mut line)?;
    let key = line.trim().to_string();
    if key.is_empty() {
        return Err(auth_err("API key cannot be empty."));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");

        let config = AuthConfig {
            api_key: "test-key".into(),
            api_base_url: Some("https://custom.api".into()),
        };
        save_auth_file(&path, &config).unwrap();

        let loaded = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(loaded.api_key, "test-key");
        assert_eq!(loaded.api_base_url.as_deref(), Some("https://custom.api"));

        let config = AuthConfig {
            api_key: "test-key-2".into(),
            api_base_url: None,
        };
        save_auth_file(&path, &config).unwrap();

        let json = fs::read_to_string(&path).unwrap();
        assert!(!json.contains("api_base_url"));

        let loaded = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(loaded.api_key, "test-key-2");
        assert!(loaded.api_base_url.is_none());
    }

    #[test]
    fn test_load_config_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_auth_file(&dir.path().join("nonexistent.json")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_api_key() {
        let config = AuthConfig {
            api_key: "file-key".into(),
            api_base_url: None,
        };
        assert!(pick_api_key(None, None).is_err());
        assert_eq!(pick_api_key(None, Some(&config)).unwrap(), "file-key");
        assert_eq!(
            pick_api_key(Some("cli-key"), Some(&config)).unwrap(),
            "cli-key"
        );
    }

    #[test]
    fn test_run_with_provided_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        run(
            Some("provided-key".into()),
            Some("https://custom.api".into()),
            &path,
            &mut io::empty(),
        )
        .unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, "provided-key");
        assert_eq!(config.api_base_url.as_deref(), Some("https://custom.api"));
    }

    #[test]
    fn test_run_with_prompted_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut input = io::Cursor::new(b"prompted-key\n");
        run(None, None, &path, &mut input).unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, "prompted-key");
        assert!(config.api_base_url.is_none());
    }

    #[test]
    fn test_pick_api_base_url() {
        assert_eq!(
            pick_api_base_url(Some("https://custom.api"), None),
            "https://custom.api"
        );
        assert_eq!(pick_api_base_url(None, None), DEFAULT_API_BASE_URL);
    }
}
