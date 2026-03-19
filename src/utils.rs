use anyhow::{Context, Result, bail};
use ureq::Body;
use ureq::http::Response;

pub fn open_db(path: &str) -> Result<duckdb::Connection> {
    duckdb::Connection::open(path).with_context(|| format!("Failed to open database {path}"))
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
    let mut response = req.call()?;
    check_response(&mut response)?;
    Ok(response)
}

pub fn api_post(url: &str, api_key: &str, body: &serde_json::Value) -> Result<Response<Body>> {
    let mut response = ureq::post(url)
        .header("Api-Key", api_key)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(body)?;
    check_response(&mut response)?;
    Ok(response)
}

fn check_response(response: &mut Response<Body>) -> Result<()> {
    let status = response.status().as_u16();
    if status >= 400 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        bail!("API returned {status} status:\n{body}");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_utils {
    pub fn temp_db() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db").to_str().unwrap().to_string();
        (dir, db_path)
    }

    pub fn parse_ndjson(buf: Vec<u8>) -> Vec<serde_json::Value> {
        String::from_utf8(buf)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
