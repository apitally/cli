use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use regex::Regex;

use crate::utils::{ansi, auth_err};

const DEFAULT_API_BASE_URL: &str = "https://api.apitally.io";
const MAX_CALLBACK_REQUEST_SIZE: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
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
    app_url: &str,
    auth_file_path: &Path,
    input: Option<Box<dyn Read + Send>>,
) -> Result<()> {
    let api_key = match api_key {
        Some(key) => key,
        None => browser_auth(app_url, input)?,
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

fn browser_auth(app_url: &str, input: Option<Box<dyn Read + Send>>) -> Result<String> {
    // Over SSH, the browser runs on another machine and cannot reach the callback server
    let ssh_session = ["SSH_CONNECTION", "SSH_TTY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
    if ssh_session && input.is_none() {
        return Err(auth_err(
            "browser auth over SSH requires an interactive terminal.\n\n\
             Provide the API key with --api-key or APITALLY_API_KEY instead",
        ));
    }

    let listener = if ssh_session {
        None
    } else {
        Some(TcpListener::bind("127.0.0.1:0").context("failed to start local callback server")?)
    };
    let url = match &listener {
        Some(listener) => {
            let port = listener.local_addr()?.port();
            format!("{app_url}/cli-auth?callback_port={port}&callback_mode=form")
        }
        None => format!("{app_url}/cli-auth"),
    };

    #[cfg(not(test))]
    let _ = open::that(&url);

    eprintln!("Opening browser with URL: {url}\n");
    eprintln!("Complete the auth flow in the browser.");
    if input.is_some() {
        eprint!("Or paste your API key and press Enter: ");
    }
    io::stderr().flush()?;

    let (tx, rx) = mpsc::channel();

    if let Some(input) = input {
        let tx_stdin = tx.clone();
        thread::spawn(move || read_stdin(tx_stdin, input));
    }

    if let Some(listener) = listener {
        let app_url = app_url.to_string();
        thread::spawn(move || run_callback_server(listener, tx, &app_url));
    }

    let api_key = rx.recv_timeout(Duration::from_secs(300)).map_err(|_| {
        eprintln!("\n");
        auth_err("authentication timed out")
    })?;
    Ok(api_key)
}

fn read_stdin(tx: mpsc::Sender<String>, input: Box<dyn Read + Send>) {
    let mut line = String::new();
    if io::BufReader::new(input).read_line(&mut line).is_ok() {
        let key = line.trim().to_string();
        if !key.is_empty() {
            eprintln!();
            let _ = tx.send(key);
        }
    }
}

fn run_callback_server(listener: TcpListener, tx: mpsc::Sender<String>, app_url: &str) {
    listener.set_nonblocking(false).ok();
    while let Ok((mut stream, _)) = listener.accept() {
        if let Some(api_key) = handle_callback_request(&mut stream, app_url) {
            eprintln!("\n");
            let _ = tx.send(api_key);
            return;
        }
    }
}

fn handle_callback_request(stream: &mut TcpStream, app_url: &str) -> Option<String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .ok();
    let mut reader = io::BufReader::new(Read::take(&*stream, MAX_CALLBACK_REQUEST_SIZE));

    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    if !request_line.starts_with("POST ") {
        return None;
    }

    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().ok()?;
        }
    }
    if content_length > MAX_CALLBACK_REQUEST_SIZE {
        return None;
    }
    let mut body = vec![0u8; content_length as usize];
    reader.read_exact(&mut body).ok()?;

    let api_key = form_urlencoded::parse(&body)
        .find(|(name, _)| name == "api_key")
        .map(|(_, value)| value.into_owned());
    let response = match api_key {
        Some(_) => format!(
            "HTTP/1.1 303 See Other\r\n\
            Location: {app_url}/cli-auth/success\r\n\
            Content-Length: 0\r\n\
            \r\n"
        ),
        None => "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string(),
    };
    stream.write_all(response.as_bytes()).ok();
    api_key
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
            "https://app.apitally.io",
            &path,
            None,
        )
        .unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, TEST_API_KEY);
        assert_eq!(config.api_base_url.as_deref(), Some("https://custom.api"));
    }

    #[test]
    fn test_run_with_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let input: Box<dyn Read + Send> =
            Box::new(io::Cursor::new(format!("{TEST_API_KEY}\n").into_bytes()));
        run(None, None, "https://app.apitally.io", &path, Some(input)).unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, TEST_API_KEY);
        assert!(config.api_base_url.is_none());
    }

    #[test]
    fn test_run_with_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel();
        let app_url = "https://app.apitally.io";
        let app_url_owned = app_url.to_string();
        thread::spawn(move || run_callback_server(listener, tx, &app_url_owned));

        // Send form POST without api_key
        let body = "other=value";
        let request = format!(
            "POST /callback HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).unwrap();
        let response_str = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_str.starts_with("HTTP/1.1 400"));

        // Send form POST with valid api_key, with headers and body written separately
        let body = format!("api_key={TEST_API_KEY}");
        let headers = format!(
            "POST /callback HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(headers.as_bytes()).unwrap();
        stream.flush().unwrap();
        thread::sleep(Duration::from_millis(50));
        stream.write_all(body.as_bytes()).unwrap();
        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).unwrap();
        let response_str = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_str.starts_with("HTTP/1.1 303"));
        assert!(response_str.contains(&format!("Location: {app_url}/cli-auth/success\r\n")));

        let api_key = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(api_key, TEST_API_KEY);
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
