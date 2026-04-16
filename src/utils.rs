use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use ureq::Body;
use ureq::http::Response;

#[derive(Debug)]
pub enum CliError {
    Auth(String),
    Input(String),
    Api(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(msg) | Self::Input(msg) | Self::Api(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for CliError {}

pub fn auth_err(msg: impl Into<String>) -> anyhow::Error {
    CliError::Auth(msg.into()).into()
}

pub fn input_err(msg: impl Into<String>) -> anyhow::Error {
    CliError::Input(msg.into()).into()
}

pub fn api_err(msg: impl Into<String>) -> anyhow::Error {
    CliError::Api(msg.into()).into()
}

pub fn ansi(code: &str, text: impl std::fmt::Display) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".apitally").join("data.duckdb"))
}

pub fn resolve_db(db: Option<Option<PathBuf>>) -> Result<Option<PathBuf>> {
    match db {
        None => Ok(None),
        Some(Some(p)) => Ok(Some(p)),
        Some(None) => default_db_path().map(Some),
    }
}

pub fn open_db(path: &Path) -> Result<duckdb::Connection> {
    duckdb::Connection::open(path)
        .with_context(|| format!("failed to open database {}", path.display()))
}

/// If `s` is a compact relative duration (`<digits><m|h|d|w>`, e.g. `24h`, `7d`), returns the
/// corresponding UTC instant as RFC 3339 (never naive). Otherwise returns `s` unchanged.
pub fn resolve_relative_datetime(s: &str) -> String {
    let b = s.as_bytes();
    if b.len() < 2 {
        return s.to_owned();
    }
    let unit = b[b.len() - 1];
    if !matches!(unit, b'm' | b'h' | b'd' | b'w') {
        return s.to_owned();
    }
    let prefix = &s[..s.len() - 1];
    if prefix.is_empty() || !prefix.bytes().all(|c| c.is_ascii_digit()) {
        return s.to_owned();
    }
    let Ok(n) = prefix.parse::<i64>() else {
        return s.to_owned();
    };
    let secs = match unit {
        b'm' => n.checked_mul(60),
        b'h' => n.checked_mul(3600),
        b'd' => n.checked_mul(86_400),
        b'w' => n.checked_mul(604_800),
        _ => None,
    };
    let Some(secs) = secs else {
        return s.to_owned();
    };
    let duration = chrono::Duration::seconds(secs);
    let Some(dt) = chrono::Utc::now().checked_sub_signed(duration) else {
        return s.to_owned();
    };
    dt.to_rfc3339()
}

pub fn api_get(url: &str, api_key: &str, query: &[(&str, &str)]) -> Result<Response<Body>> {
    let mut req = ureq::get(url)
        .header("Api-Key", api_key)
        .config()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_recv_response(Some(Duration::from_secs(15)))
        .build();
    for (key, value) in query {
        req = req.query(key, value);
    }
    let mut response = req.call().map_err(|e| api_err(e.to_string()))?;
    check_response(&mut response)?;
    Ok(response)
}

pub fn api_post(url: &str, api_key: &str, body: &serde_json::Value) -> Result<Response<Body>> {
    let mut response = ureq::post(url)
        .header("Api-Key", api_key)
        .config()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_recv_response(Some(Duration::from_secs(15)))
        .build()
        .send_json(body)
        .map_err(|e| api_err(e.to_string()))?;
    check_response(&mut response)?;
    Ok(response)
}

fn check_response(response: &mut Response<Body>) -> Result<()> {
    let status = response.status().as_u16();
    if status >= 400 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        let msg = format!("API returned {status} status:\n{body}");
        return Err(match status {
            401 | 403 => auth_err(msg),
            400 | 404 | 422 => input_err(msg),
            _ => api_err(msg),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(status: u16) -> Response<Body> {
        Response::builder()
            .status(status)
            .body(Body::builder().data(""))
            .unwrap()
    }

    #[test]
    fn test_resolve_db() {
        assert!(resolve_db(None).unwrap().is_none());

        let p = PathBuf::from("/tmp/my.db");
        assert_eq!(resolve_db(Some(Some(p.clone()))).unwrap().unwrap(), p);

        let resolved = resolve_db(Some(None)).unwrap().unwrap();
        assert!(resolved.ends_with("data.duckdb"));
        assert!(resolved.to_string_lossy().contains(".apitally"));
    }

    #[test]
    fn test_resolve_relative_datetime() {
        assert_eq!(
            resolve_relative_datetime("2025-01-01T00:00:00Z"),
            "2025-01-01T00:00:00Z"
        );

        fn assert_approximately_now_minus(out: &str, expected_secs: i64) {
            let t: chrono::DateTime<chrono::Utc> = out.parse().expect("parse rfc3339");
            let ago = (chrono::Utc::now() - t).num_seconds();
            assert!(
                (ago - expected_secs).abs() <= 3,
                "expected ~{expected_secs}s ago, got {ago}s ({out:?})"
            );
        }

        assert_approximately_now_minus(&resolve_relative_datetime("30m"), 30 * 60);
        assert_approximately_now_minus(&resolve_relative_datetime("2h"), 2 * 3600);
        assert_approximately_now_minus(&resolve_relative_datetime("3d"), 259_200);
        assert_approximately_now_minus(&resolve_relative_datetime("1w"), 604_800);
    }

    #[test]
    fn test_check_response() {
        assert!(check_response(&mut resp(200)).is_ok());

        for (status, expect_variant) in [
            (401u16, "Auth"),
            (403, "Auth"),
            (400, "Input"),
            (404, "Input"),
            (422, "Input"),
            (500, "Api"),
        ] {
            let err = check_response(&mut resp(status)).unwrap_err();
            let variant = match err.downcast_ref::<CliError>().unwrap() {
                CliError::Auth(_) => "Auth",
                CliError::Input(_) => "Input",
                CliError::Api(_) => "Api",
            };
            assert_eq!(variant, expect_variant, "status {status}");
        }
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use std::path::PathBuf;

    pub fn temp_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        (dir, db_path)
    }

    pub fn parse_ndjson(buf: &[u8]) -> Vec<serde_json::Value> {
        std::str::from_utf8(buf)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
