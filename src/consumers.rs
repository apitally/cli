use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_get, open_db};

#[derive(Deserialize)]
struct ConsumersResponse {
    data: Vec<ConsumerItem>,
    has_more: bool,
    next_token: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ConsumerItem {
    id: i64,
    identifier: String,
    name: String,
    group: Option<ConsumerGroupItem>,
    created_at: String,
    last_request_at: String,
}

#[derive(Deserialize, Serialize)]
struct ConsumerGroupItem {
    id: i64,
    name: String,
}

fn fetch_consumers_page(
    api_key: &str,
    api_base_url: &str,
    app_id: i64,
    requests_since: Option<&str>,
    next_token: Option<&str>,
) -> Result<ConsumersResponse> {
    let url = format!("{api_base_url}/v1/apps/{app_id}/consumers");
    let mut query = vec![("limit", "1000")];
    if let Some(since) = requests_since {
        query.push(("requests_since", since));
    }
    if let Some(token) = next_token {
        query.push(("next_token", token));
    }
    let mut response = api_get(&url, api_key, &query)?;
    let page: ConsumersResponse = response.body_mut().read_json()?;
    Ok(page)
}

fn ensure_consumers_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS consumers (
            app_id INTEGER NOT NULL,
            consumer_id INTEGER NOT NULL,
            identifier TEXT NOT NULL,
            name TEXT NOT NULL,
            \"group\" TEXT,
            created_at TIMESTAMPTZ NOT NULL,
            last_request_at TIMESTAMPTZ,
            UNIQUE (app_id, consumer_id)
        )",
    )?;
    Ok(())
}

fn write_consumers_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    consumers: &[ConsumerItem],
) -> Result<()> {
    let mut stmt = conn.prepare("INSERT OR REPLACE INTO consumers VALUES (?, ?, ?, ?, ?, ?, ?)")?;
    for consumer in consumers {
        let group_name = consumer.group.as_ref().map(|g| g.name.as_str());
        stmt.execute(duckdb::params![
            app_id,
            consumer.id,
            consumer.identifier,
            consumer.name,
            group_name,
            consumer.created_at,
            consumer.last_request_at,
        ])?;
    }
    Ok(())
}

pub fn run(
    app_id: i64,
    requests_since: Option<&str>,
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let db = db.map(|p| open_db(p).map(|c| (p, c))).transpose()?;

    if let Some((_, conn)) = &db {
        ensure_consumers_table(conn)?;
    }

    let mut next_token: Option<String> = None;
    let mut total = 0usize;

    if let Some((db_path, _)) = &db {
        eprint!(
            "0 consumers written to table 'consumers' in {}...",
            db_path.display()
        );
    }

    loop {
        let page = fetch_consumers_page(
            &api_key,
            &api_base_url,
            app_id,
            requests_since,
            next_token.as_deref(),
        )?;
        total += page.data.len();

        if let Some((db_path, conn)) = &db {
            write_consumers_to_db(conn, app_id, &page.data)?;
            eprint!(
                "\r{total} consumers written to table 'consumers' in {}...",
                db_path.display()
            );
        } else {
            for consumer in &page.data {
                serde_json::to_writer(&mut writer, consumer)?;
                writeln!(writer)?;
            }
        }

        if page.has_more {
            next_token = page.next_token;
        } else {
            break;
        }
    }

    if db.is_some() {
        eprintln!("\nDone.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_consumers_page1_json() -> &'static str {
        r#"{
            "data": [
                {
                    "id": 1,
                    "identifier": "user-1",
                    "name": "User One",
                    "group": null,
                    "created_at": "2025-01-01T00:00:00Z",
                    "last_request_at": "2025-06-01T12:00:00Z"
                }
            ],
            "has_more": true,
            "next_token": "token123"
        }"#
    }

    fn sample_consumers_page2_json() -> &'static str {
        r#"{
            "data": [
                {
                    "id": 2,
                    "identifier": "user-2",
                    "name": "User Two",
                    "group": {"id": 1, "name": "Admins"},
                    "created_at": "2025-02-01T00:00:00Z",
                    "last_request_at": "2025-06-15T12:00:00Z"
                }
            ],
            "has_more": false,
            "next_token": null
        }"#
    }

    fn mock_consumers_endpoint(
        server: &mut mockito::Server,
        app_id: i64,
    ) -> (mockito::Mock, mockito::Mock) {
        let path = format!("/v1/apps/{app_id}/consumers");
        let mock1 = server
            .mock("GET", path.as_str())
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_consumers_page1_json())
            .create();
        let mock2 = server
            .mock("GET", path.as_str())
            .match_query(mockito::Matcher::UrlEncoded(
                "next_token".into(),
                "token123".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_consumers_page2_json())
            .create();
        (mock1, mock2)
    }

    #[test]
    fn test_run_ndjson() {
        let mut server = mockito::Server::new();
        let (mock1, mock2) = mock_consumers_endpoint(&mut server, 1);

        let mut buf = Vec::new();
        run(
            1,
            None,
            None,
            Some("test-key"),
            Some(&server.url()),
            &mut buf,
        )
        .unwrap();
        mock1.assert();
        mock2.assert();

        let rows = parse_ndjson(&buf);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["identifier"], "user-1");
        assert!(rows[0]["group"].is_null());
        assert_eq!(rows[1]["identifier"], "user-2");
        assert_eq!(rows[1]["group"]["name"], "Admins");
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let (mock1, mock2) = mock_consumers_endpoint(&mut server, 1);
        let (_dir, db_path) = temp_db();

        run(
            1,
            None,
            Some(&db_path),
            Some("test-key"),
            Some(&server.url()),
            Vec::new(),
        )
        .unwrap();
        mock1.assert();
        mock2.assert();

        let conn = open_db(&db_path).unwrap();

        let count: i64 = conn
            .query_row("SELECT count(*) FROM consumers", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let group: Option<String> = conn
            .query_row(
                "SELECT \"group\" FROM consumers WHERE consumer_id = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group.as_deref(), Some("Admins"));
    }
}
