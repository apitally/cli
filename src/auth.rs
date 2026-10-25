use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use regex::Regex;

use crate::utils::{ansi, api_post_without_key, auth_err};

const DEFAULT_API_BASE_URL: &str = "https://api.apitally.io";

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

#[derive(Deserialize)]
struct CliAuthSessionResponse {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct CliAuthTokenResponse {
    api_key: String,
}

pub fn auth_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".apitally").join("auth.json"))
}

fn load_auth_file(path: &Path) -> Result<Option<AuthConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let config: AuthConfig = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(config))
}

fn save_auth_file(path: &Path, config: &AuthConfig) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, &json).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
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
        "no API key configured.\n\n\
         Run `apitally auth` to set up authentication, or provide --api-key / APITALLY_API_KEY",
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

fn validate_api_key(api_key: &str) -> Result<()> {
    let re = Regex::new(r"^[a-zA-Z0-9]{7}\.[a-zA-Z0-9]{32}$").unwrap();
    if !re.is_match(api_key) {
        return Err(auth_err("invalid API key format"));
    }
    Ok(())
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
) -> Result<()> {
    let api_key = match api_key {
        Some(key) => key,
        None => browser_auth(&pick_api_base_url(api_base_url.as_deref(), None))?,
    };
    validate_api_key(&api_key)?;
    save_auth_file(
        auth_file_path,
        &AuthConfig {
            api_key,
            api_base_url,
        },
    )?;
    eprintln!("{}", ansi("1;32", "API key configured successfully."));
    Ok(())
}

fn browser_auth(api_base_url: &str) -> Result<String> {
    let url = format!("{api_base_url}/v1/cli-auth/sessions");
    let mut response = api_post_without_key(&url, &serde_json::json!({}))?;
    let session: CliAuthSessionResponse = response.body_mut().read_json()?;

    eprintln!("Opening browser with URL: {}\n", session.verification_url);
    eprintln!(
        "Confirm that the browser shows this code: {}\n",
        session.user_code
    );

    #[cfg(not(test))]
    let _ = open::that(&session.verification_url);

    let token_url = format!("{api_base_url}/v1/cli-auth/token");
    let body = serde_json::json!({ "device_code": session.device_code });
    let deadline = Instant::now() + Duration::from_secs(session.expires_in);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(session.interval));
        let Ok(mut response) = ureq::post(&token_url)
            .config()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .build()
            .send_json(&body)
        else {
            continue;
        };
        match response.status().as_u16() {
            200 => {
                return Ok(response
                    .body_mut()
                    .read_json::<CliAuthTokenResponse>()?
                    .api_key);
            }
            429 => {}
            400..=499 => {
                return Err(auth_err(
                    "authentication expired, run `apitally auth` again",
                ));
            }
            _ => {}
        }
    }
    Err(auth_err("authentication timed out"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_API_KEY: &str = "aBcDeFg.01234567890123456789012345678901";

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
    fn test_run_with_api_key_flag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        run(
            Some(TEST_API_KEY.into()),
            Some("https://custom.api".into()),
            &path,
        )
        .unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, TEST_API_KEY);
        assert_eq!(config.api_base_url.as_deref(), Some("https://custom.api"));
    }

    #[test]
    fn test_run_with_polling() {
        let mut server = mockito::Server::new();
        let sessions_mock = server
            .mock("POST", "/v1/cli-auth/sessions")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "device_code": "test-device-code",
                    "user_code": "WDJB-MJHT",
                    "verification_url": "https://app.apitally.io/cli-auth?code=WDJB-MJHT",
                    "expires_in": 600,
                    "interval": 0,
                })
                .to_string(),
            )
            .create();
        let pending_mock = server
            .mock("POST", "/v1/cli-auth/token")
            .match_body(mockito::Matcher::Json(
                serde_json::json!({"device_code": "test-device-code"}),
            ))
            .with_status(202)
            .with_body("{}")
            .expect(1)
            .create();
        let approved_mock = server
            .mock("POST", "/v1/cli-auth/token")
            .match_body(mockito::Matcher::Json(
                serde_json::json!({"device_code": "test-device-code"}),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"api_key":"{TEST_API_KEY}"}}"#))
            .expect(1)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        run(None, Some(server.url()), &path).unwrap();
        sessions_mock.assert();
        pending_mock.assert();
        approved_mock.assert();

        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, TEST_API_KEY);
        assert_eq!(config.api_base_url, Some(server.url()));
    }

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key(TEST_API_KEY).is_ok());
        assert!(validate_api_key("short.01234567890123456789012345678901").is_err());
        assert!(validate_api_key("aBcDeFg.short").is_err());
        assert!(validate_api_key("invalid-key").is_err());
        assert!(validate_api_key("").is_err());
        assert!(validate_api_key("abc!eFg.01234567890123456789012345678901").is_err());
        assert!(validate_api_key("aBcDeFg.0123456789012345678901234567890!").is_err());
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
