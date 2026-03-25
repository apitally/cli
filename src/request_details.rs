use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::request_logs::ensure_request_logs_table;
use crate::utils::{api_get, open_db};

#[derive(Deserialize, Serialize)]
struct RequestDetailsResponse {
    timestamp: String,
    request_uuid: String,
    env: Option<String>,
    method: String,
    path: Option<String>,
    url: String,
    request_headers: Vec<(String, String)>,
    request_size_bytes: i64,
    request_body_json: Option<String>,
    status_code: i32,
    response_time_ms: i32,
    response_headers: Vec<(String, String)>,
    response_size_bytes: i64,
    response_body_json: Option<String>,
    client_ip: Option<String>,
    client_country_iso_code: Option<String>,
    trace_id: Option<String>,
    exception: Option<ExceptionItem>,
    logs: Vec<ApplicationLogItem>,
    spans: Vec<SpanItem>,
}

#[derive(Deserialize, Serialize)]
struct ExceptionItem {
    r#type: Option<String>,
    message: Option<String>,
    stacktrace: Option<String>,
    sentry_event_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ApplicationLogItem {
    timestamp: String,
    message: String,
    level: Option<String>,
    logger: Option<String>,
    file: Option<String>,
    line: Option<i32>,
}

#[derive(Deserialize, Serialize)]
struct SpanItem {
    span_id: String,
    parent_span_id: Option<String>,
    name: String,
    kind: String,
    start_time_ns: i64,
    end_time_ns: i64,
    duration_ns: i64,
    status: String,
    attributes: serde_json::Value,
}

fn ensure_application_logs_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS application_logs (
            app_id INTEGER NOT NULL,
            request_uuid VARCHAR NOT NULL,
            timestamp TIMESTAMPTZ NOT NULL,
            message VARCHAR NOT NULL,
            level VARCHAR,
            logger VARCHAR,
            file VARCHAR,
            line INTEGER
        )",
    )?;
    Ok(())
}

fn ensure_spans_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS spans (
            app_id INTEGER NOT NULL,
            request_uuid VARCHAR NOT NULL,
            span_id VARCHAR NOT NULL,
            parent_span_id VARCHAR,
            name VARCHAR NOT NULL,
            kind VARCHAR NOT NULL,
            start_time_ns BIGINT NOT NULL,
            end_time_ns BIGINT NOT NULL,
            duration_ns BIGINT NOT NULL,
            status VARCHAR NOT NULL,
            attributes JSON
        )",
    )?;
    Ok(())
}

fn write_request_details_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    data: &RequestDetailsResponse,
) -> Result<()> {
    let headers_to_json = |headers: &[(String, String)]| -> Result<String> {
        let arr: Vec<_> = headers
            .iter()
            .map(|(n, v)| serde_json::json!({"name": n, "value": v}))
            .collect();
        Ok(serde_json::to_string(&arr)?)
    };
    let request_headers = headers_to_json(&data.request_headers)?;
    let response_headers = headers_to_json(&data.response_headers)?;
    let exception = data.exception.as_ref();

    conn.execute(
        "INSERT INTO request_logs (
            app_id, timestamp, request_uuid, env, method, path, url,
            request_headers, request_size_bytes, request_body_json,
            status_code, response_time_ms, response_headers, response_size_bytes,
            response_body_json, client_ip, client_country_iso_code,
            exception_type, exception_message, exception_stacktrace,
            sentry_event_id, trace_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (app_id, request_uuid) DO UPDATE SET
            timestamp = excluded.timestamp,
            env = excluded.env,
            method = excluded.method,
            path = excluded.path,
            url = excluded.url,
            request_headers = excluded.request_headers,
            request_size_bytes = excluded.request_size_bytes,
            request_body_json = excluded.request_body_json,
            status_code = excluded.status_code,
            response_time_ms = excluded.response_time_ms,
            response_headers = excluded.response_headers,
            response_size_bytes = excluded.response_size_bytes,
            response_body_json = excluded.response_body_json,
            client_ip = excluded.client_ip,
            client_country_iso_code = excluded.client_country_iso_code,
            exception_type = excluded.exception_type,
            exception_message = excluded.exception_message,
            exception_stacktrace = excluded.exception_stacktrace,
            sentry_event_id = excluded.sentry_event_id,
            trace_id = excluded.trace_id",
        duckdb::params![
            app_id,
            &data.timestamp,
            &data.request_uuid,
            &data.env,
            &data.method,
            &data.path,
            &data.url,
            &request_headers,
            data.request_size_bytes,
            &data.request_body_json,
            data.status_code,
            data.response_time_ms,
            &response_headers,
            data.response_size_bytes,
            &data.response_body_json,
            &data.client_ip,
            &data.client_country_iso_code,
            exception.and_then(|e| e.r#type.as_deref()),
            exception.and_then(|e| e.message.as_deref()),
            exception.and_then(|e| e.stacktrace.as_deref()),
            exception.and_then(|e| e.sentry_event_id.as_deref()),
            &data.trace_id,
        ],
    )?;
    Ok(())
}

fn write_application_logs_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    request_uuid: &str,
    logs: &[ApplicationLogItem],
) -> Result<()> {
    conn.execute(
        "DELETE FROM application_logs WHERE app_id = ? AND request_uuid = ?",
        duckdb::params![app_id, request_uuid],
    )?;
    let mut stmt = conn.prepare("INSERT INTO application_logs VALUES (?, ?, ?, ?, ?, ?, ?, ?)")?;
    for log in logs {
        stmt.execute(duckdb::params![
            app_id,
            request_uuid,
            &log.timestamp,
            &log.message,
            &log.level,
            &log.logger,
            &log.file,
            log.line,
        ])?;
    }
    Ok(())
}

fn write_spans_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    request_uuid: &str,
    spans: &[SpanItem],
) -> Result<()> {
    conn.execute(
        "DELETE FROM spans WHERE app_id = ? AND request_uuid = ?",
        duckdb::params![app_id, request_uuid],
    )?;
    let mut stmt = conn.prepare("INSERT INTO spans VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?;
    for span in spans {
        let attributes = serde_json::to_string(&span.attributes)?;
        stmt.execute(duckdb::params![
            app_id,
            request_uuid,
            &span.span_id,
            &span.parent_span_id,
            &span.name,
            &span.kind,
            span.start_time_ns,
            span.end_time_ns,
            span.duration_ns,
            &span.status,
            &attributes,
        ])?;
    }
    Ok(())
}

pub fn run(
    app_id: i64,
    request_uuid: &str,
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let db = db.map(|p| open_db(p).map(|c| (p, c))).transpose()?;

    let url = format!("{api_base_url}/v1/apps/{app_id}/request-logs/{request_uuid}");
    let mut response = api_get(&url, &api_key, &[])?;
    let data: RequestDetailsResponse = response.body_mut().read_json()?;

    if let Some((db_path, conn)) = &db {
        ensure_request_logs_table(conn)?;
        ensure_application_logs_table(conn)?;
        ensure_spans_table(conn)?;

        write_request_details_to_db(conn, app_id, &data)?;
        write_application_logs_to_db(conn, app_id, &data.request_uuid, &data.logs)?;
        write_spans_to_db(conn, app_id, &data.request_uuid, &data.spans)?;

        eprintln!(
            "Request details written to tables 'request_logs', 'application_logs', 'spans' in {}.\nDone.",
            db_path.display(),
        );
    } else {
        serde_json::to_writer(&mut writer, &data)?;
        writeln!(writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::temp_db;

    fn sample_request_details_json() -> &'static str {
        r#"{
            "timestamp": "2025-01-01T00:00:00Z",
            "request_uuid": "abc-123",
            "env": "prod",
            "consumer": "user-1",
            "method": "GET",
            "path": "/test",
            "url": "https://example.com/test",
            "status_code": 200,
            "request_size_bytes": 0,
            "response_size_bytes": 1234,
            "response_time_ms": 50,
            "client_ip": "1.2.3.4",
            "client_country_iso_code": "US",
            "request_headers": [["Content-Type", "application/json"]],
            "response_headers": [["X-Request-Id", "abc"]],
            "request_body_json": null,
            "response_body_json": "{\"ok\":true}",
            "trace_id": "0000000000000000aaaaaaaaaaaaaaaa",
            "exception": {
                "type": "ValueError",
                "message": "bad value",
                "stacktrace": "Traceback ...",
                "sentry_event_id": null
            },
            "logs": [
                {
                    "timestamp": "2025-01-01T00:00:00.100Z",
                    "message": "handling request",
                    "level": "INFO",
                    "logger": "app",
                    "file": "main.py",
                    "line": 42
                }
            ],
            "spans": [
                {
                    "span_id": "00000000000000aa",
                    "parent_span_id": null,
                    "name": "GET /test",
                    "kind": "SERVER",
                    "start_time_ns": 1735689600000000000,
                    "end_time_ns": 1735689600050000000,
                    "duration_ns": 50000000,
                    "status": "OK",
                    "attributes": {"http.method": "GET"}
                }
            ]
        }"#
    }

    fn mock_request_details_endpoint(
        server: &mut mockito::Server,
        app_id: i64,
        request_uuid: &str,
    ) -> mockito::Mock {
        server
            .mock(
                "GET",
                format!("/v1/apps/{app_id}/request-logs/{request_uuid}").as_str(),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_request_details_json())
            .create()
    }

    #[test]
    fn test_run_json() {
        let mut server = mockito::Server::new();
        let mock = mock_request_details_endpoint(&mut server, 1, "abc-123");

        let mut buf = Vec::new();
        run(
            1,
            "abc-123",
            None,
            Some("test-key"),
            Some(&server.url()),
            &mut buf,
        )
        .unwrap();
        mock.assert();

        let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(output["method"], "GET");
        assert_eq!(output["status_code"], 200);
        assert_eq!(output["trace_id"], "0000000000000000aaaaaaaaaaaaaaaa");
        assert_eq!(output["exception"]["type"], "ValueError");
        assert_eq!(output["logs"][0]["message"], "handling request");
        assert_eq!(output["spans"][0]["name"], "GET /test");
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let mock = mock_request_details_endpoint(&mut server, 1, "abc-123");
        let (_dir, db_path) = temp_db();

        // Pre-insert a row with consumer_id to verify the upsert preserves it
        let conn = open_db(&db_path).unwrap();
        ensure_request_logs_table(&conn).unwrap();
        conn.execute(
            "INSERT INTO request_logs (app_id, request_uuid, timestamp, method, url, consumer_id) \
             VALUES (1, 'abc-123', '2025-01-01T00:00:00Z', 'GET', '/old', 42)",
            [],
        )
        .unwrap();
        drop(conn);

        run(
            1,
            "abc-123",
            Some(&db_path),
            Some("test-key"),
            Some(&server.url()),
            Vec::new(),
        )
        .unwrap();
        mock.assert();

        let conn = open_db(&db_path).unwrap();

        let (method, url, trace_id, consumer_id): (String, String, Option<String>, Option<i32>) =
            conn.query_row(
                "SELECT method, url, trace_id, consumer_id FROM request_logs \
                 WHERE app_id = 1 AND request_uuid = 'abc-123'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(method, "GET");
        assert_eq!(url, "https://example.com/test");
        assert_eq!(
            trace_id.as_deref(),
            Some("0000000000000000aaaaaaaaaaaaaaaa")
        );
        assert_eq!(consumer_id, Some(42));

        let log_message: String = conn
            .query_row("SELECT message FROM application_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(log_message, "handling request");

        let span_name: String = conn
            .query_row("SELECT name FROM spans", [], |row| row.get(0))
            .unwrap();
        assert_eq!(span_name, "GET /test");
    }
}
