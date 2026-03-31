use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_get, open_db};

#[derive(Deserialize)]
struct EndpointsResponse {
    data: Vec<EndpointItem>,
}

#[derive(Deserialize, Serialize)]
struct EndpointItem {
    id: i64,
    method: String,
    path: String,
}

fn fetch_endpoints(
    api_key: &str,
    api_base_url: &str,
    app_id: i64,
    method: Option<&str>,
    path: Option<&str>,
) -> Result<Vec<EndpointItem>> {
    let url = format!("{api_base_url}/v1/apps/{app_id}/endpoints");
    let mut query: Vec<(&str, &str)> = Vec::new();
    if let Some(m) = method {
        query.push(("method", m));
    }
    if let Some(p) = path {
        query.push(("path", p));
    }
    let mut response = api_get(&url, api_key, &query)?;
    let endpoints: EndpointsResponse = response.body_mut().read_json()?;
    Ok(endpoints.data)
}

pub(crate) fn ensure_endpoints_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS endpoints (
            app_id INTEGER NOT NULL,
            endpoint_id INTEGER NOT NULL,
            method TEXT NOT NULL,
            path TEXT NOT NULL,
            UNIQUE (app_id, endpoint_id)
        )",
    )?;
    Ok(())
}

fn write_endpoints_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    endpoints: &[EndpointItem],
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO endpoints (
            app_id, endpoint_id, method, path
        ) VALUES (?, ?, ?, ?)",
    )?;
    for endpoint in endpoints {
        stmt.execute(duckdb::params![
            app_id,
            endpoint.id,
            endpoint.method,
            endpoint.path,
        ])?;
    }
    Ok(())
}

pub fn run(
    app_id: i64,
    method: Option<&str>,
    path: Option<&str>,
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let endpoints = fetch_endpoints(&api_key, &api_base_url, app_id, method, path)?;

    if let Some(db_path) = db {
        let conn = open_db(db_path)?;
        ensure_endpoints_table(&conn)?;
        write_endpoints_to_db(&conn, app_id, &endpoints)?;
        eprintln!(
            "{} endpoints written to table 'endpoints' in {}.\nDone.",
            endpoints.len(),
            db_path.display(),
        );
    } else {
        for endpoint in &endpoints {
            serde_json::to_writer(&mut writer, endpoint)?;
            writeln!(writer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_endpoints_json() -> &'static str {
        r#"{
            "data": [
                {
                    "id": 1,
                    "method": "POST",
                    "path": "/v1/users"
                },
                {
                    "id": 2,
                    "method": "GET",
                    "path": "/v1/users/{user_id}"
                }
            ]
        }"#
    }

    fn mock_endpoints_endpoint(server: &mut mockito::Server, app_id: i64) -> mockito::Mock {
        let path = format!("/v1/apps/{app_id}/endpoints");
        server
            .mock("GET", path.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_endpoints_json())
            .create()
    }

    #[test]
    fn test_run_ndjson() {
        let mut server = mockito::Server::new();
        let mock = mock_endpoints_endpoint(&mut server, 1);

        let mut buf = Vec::new();
        run(
            1,
            None,
            None,
            None,
            Some("test-key"),
            Some(&server.url()),
            &mut buf,
        )
        .unwrap();
        mock.assert();

        let rows = parse_ndjson(&buf);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["method"], "POST");
        assert_eq!(rows[0]["path"], "/v1/users");
        assert_eq!(rows[1]["method"], "GET");
        assert_eq!(rows[1]["path"], "/v1/users/{user_id}");
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let mock = mock_endpoints_endpoint(&mut server, 1);
        let (_dir, db_path) = temp_db();

        run(
            1,
            None,
            None,
            Some(&db_path),
            Some("test-key"),
            Some(&server.url()),
            Vec::new(),
        )
        .unwrap();
        mock.assert();

        let conn = open_db(&db_path).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM endpoints WHERE app_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        let method: String = conn
            .query_row(
                "SELECT method FROM endpoints WHERE endpoint_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(method, "POST");
    }
}
