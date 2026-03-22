use std::path::Path;

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

pub fn open_db(path: &Path) -> Result<duckdb::Connection> {
    duckdb::Connection::open(path)
        .with_context(|| format!("failed to open database {}", path.display()))
}

pub fn api_get(url: &str, api_key: &str, query: &[(&str, &str)]) -> Result<Response<Body>> {
    let mut req = ureq::get(url)
        .header("Api-Key", api_key)
        .config()
        .http_status_as_error(false)
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
