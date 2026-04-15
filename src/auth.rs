use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::utils::{ansi, auth_err};

const DEFAULT_API_BASE_URL: &str = "https://api.apitally.io";

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
    input: Box<dyn Read + Send>,
) -> Result<()> {
    let api_key = match api_key {
        Some(key) => key,
        None => browser_auth(app_url, input)?,
    };
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

fn browser_auth(app_url: &str, input: Box<dyn Read + Send>) -> Result<String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to start local callback server")?;
    let port = listener.local_addr()?.port();
    let url = format!("{app_url}/cli-auth?callback_port={port}");

    #[cfg(not(test))]
    let _ = open::that(&url);

    eprintln!("Opening browser with URL: {url}\n");
    eprintln!("Complete the auth flow in the browser.");
    eprint!("Or paste your API key and press Enter: ");
    io::stderr().flush()?;

    let (tx, rx) = mpsc::channel();

    let tx_server = tx.clone();
    thread::spawn(move || run_callback_server(listener, tx_server));
    thread::spawn(move || read_stdin(tx, input));

    let api_key = rx
        .recv_timeout(Duration::from_secs(300))
        .map_err(|_| auth_err("authentication timed out"))?;
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

fn run_callback_server(listener: TcpListener, tx: mpsc::Sender<String>) {
    listener.set_nonblocking(false).ok();
    while let Ok((mut stream, _)) = listener.accept() {
        if let Some(api_key) = handle_callback_request(&mut stream) {
            eprintln!("\n");
            let _ = tx.send(api_key);
            return;
        }
    }
}

fn handle_callback_request(stream: &mut TcpStream) -> Option<String> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).ok()?;
    let request = std::str::from_utf8(&buf[..n]).ok()?;

    if request.starts_with("OPTIONS ") {
        let response = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: POST\r\n\
            Access-Control-Allow-Headers: Content-Type\r\n\
            Content-Length: 0\r\n\
            \r\n";
        stream.write_all(response.as_bytes()).ok();
        return None;
    }

    if request.starts_with("POST ") {
        let api_key = request
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
            .and_then(|parsed| parsed["api_key"].as_str().map(String::from));

        if let Some(api_key) = api_key {
            let response = "HTTP/1.1 200 OK\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Content-Length: 0\r\n\
                \r\n";
            stream.write_all(response.as_bytes()).ok();
            return Some(api_key);
        }

        let response = "HTTP/1.1 400 Bad Request\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Content-Length: 0\r\n\
            \r\n";
        stream.write_all(response.as_bytes()).ok();
        return None;
    }

    None
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
    fn test_run_with_api_key_flag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        run(
            Some("provided-key".into()),
            Some("https://custom.api".into()),
            "https://app.apitally.io",
            &path,
            Box::new(io::empty()),
        )
        .unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, "provided-key");
        assert_eq!(config.api_base_url.as_deref(), Some("https://custom.api"));
    }

    #[test]
    fn test_run_with_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let input = Box::new(io::Cursor::new(b"stdin-key\n".to_vec()));
        run(None, None, "https://app.apitally.io", &path, input).unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, "stdin-key");
        assert!(config.api_base_url.is_none());
    }

    #[test]
    fn test_run_with_callback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_callback_server(listener, tx));

        // Send CORS preflight
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .write_all(b"OPTIONS /callback HTTP/1.1\r\nOrigin: https://app.apitally.io\r\n\r\n")
            .unwrap();
        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).unwrap();
        let response_str = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_str.contains("204"));
        assert!(response_str.contains("Access-Control-Allow-Origin: *"));

        // Send callback POST
        let body = r#"{"api_key":"callback-key"}"#;
        let request = format!(
            "POST /callback HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).unwrap();
        let response_str = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_str.contains("200"));

        let api_key = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(api_key, "callback-key");

        save_auth_file(
            &path,
            &AuthConfig {
                api_key,
                api_base_url: None,
            },
        )
        .unwrap();
        let config = load_auth_file(&path).unwrap().unwrap();
        assert_eq!(config.api_key, "callback-key");
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
