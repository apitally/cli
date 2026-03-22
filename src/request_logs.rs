use std::io;
use std::path::Path;

use anyhow::Result;
use duckdb::arrow::ipc::reader::StreamReader;
use duckdb::vtab::arrow::{ArrowVTab, arrow_recordbatch_to_query_params};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_post, input_err, open_db};

fn ensure_request_logs_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS request_logs (
            app_id INTEGER NOT NULL,
            timestamp TIMESTAMPTZ NOT NULL,
            request_uuid VARCHAR NOT NULL,
            app_env VARCHAR,
            method VARCHAR NOT NULL,
            path VARCHAR,
            url VARCHAR NOT NULL,
            consumer_id INTEGER,
            request_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[],
            request_size BIGINT,
            request_body_json JSON,
            status_code INTEGER,
            response_time_ms INTEGER,
            response_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[],
            response_size BIGINT,
            response_body_json JSON,
            client_ip VARCHAR,
            client_country_iso_code VARCHAR,
            exception_type VARCHAR,
            exception_message VARCHAR,
            exception_stacktrace VARCHAR,
            sentry_event_id VARCHAR,
            trace_id VARCHAR,
            UNIQUE (app_id, request_uuid)
        )",
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    app_id: i64,
    since: &str,
    until: Option<&str>,
    fields: Option<&str>,
    filters: Option<&str>,
    limit: Option<i64>,
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl io::Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let db = db.map(|p| open_db(p).map(|c| (p, c))).transpose()?;

    let format = if db.is_some() { "arrow" } else { "ndjson" };
    let mut body = serde_json::json!({
        "format": format,
        "since": since,
    });
    if let Some(until) = until {
        body["until"] = serde_json::json!(until);
    }
    if let Some(fields) = fields {
        let fields_value: serde_json::Value = serde_json::from_str(fields)
            .map_err(|e| input_err(format!("Invalid JSON for --fields: {e}")))?;
        body["fields"] = fields_value;
    }
    if let Some(filters) = filters {
        let filters_value: serde_json::Value = serde_json::from_str(filters)
            .map_err(|e| input_err(format!("Invalid JSON for --filters: {e}")))?;
        body["filters"] = filters_value;
    }
    if let Some(limit) = limit {
        body["limit"] = serde_json::json!(limit);
    }
    let url = format!("{api_base_url}/v1/apps/{app_id}/request-logs/stream");
    let response = api_post(&url, &api_key, &body)?;

    if let Some((db_path, conn)) = &db {
        conn.register_table_function::<ArrowVTab>("arrow")?;
        ensure_request_logs_table(conn)?;
        conn.execute_batch(
            "CREATE TEMPORARY TABLE request_logs_staging AS \
             SELECT * FROM request_logs LIMIT 0",
        )?;

        let reader = StreamReader::try_new(response.into_body().into_reader(), None)?;
        let col_list = reader
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let insert_sql = format!(
            "INSERT INTO request_logs_staging (app_id, {col_list}) \
             SELECT {app_id}, {col_list} FROM arrow(?, ?)"
        );

        const CHUNK_SIZE: usize = 2048; // DuckDB's vector size
        let mut total = 0usize;

        eprint!(
            "0 request logs written to table 'request_logs' in {}...",
            db_path.display()
        );

        for batch in reader {
            let batch = batch?;
            total += batch.num_rows();
            for offset in (0..batch.num_rows()).step_by(CHUNK_SIZE) {
                let chunk = batch.slice(offset, (batch.num_rows() - offset).min(CHUNK_SIZE));
                let params = arrow_recordbatch_to_query_params(chunk);
                conn.execute(&insert_sql, params)?;
            }
            eprint!(
                "\r{total} request logs written to table 'request_logs' in {}...",
                db_path.display()
            );
        }

        conn.execute_batch(
            "INSERT OR REPLACE INTO request_logs \
             SELECT * FROM request_logs_staging; \
             DROP TABLE request_logs_staging;",
        )?;

        eprintln!("\nDone.");
    } else {
        io::copy(&mut response.into_body().into_reader(), &mut writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use duckdb::arrow::array::{StringArray, TimestampMillisecondArray};
    use duckdb::arrow::datatypes::{DataType, Field, Schema, TimeUnit};
    use duckdb::arrow::ipc::writer::StreamWriter;
    use duckdb::arrow::record_batch::RecordBatch;

    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_request_logs_ndjson() -> &'static str {
        "{\"timestamp\":\"2025-01-01T00:00:00Z\",\"request_uuid\":\"abc\",\"method\":\"GET\",\"url\":\"/test\",\"status_code\":200}\n\
         {\"timestamp\":\"2025-01-01T00:01:00Z\",\"request_uuid\":\"def\",\"method\":\"POST\",\"url\":\"/test2\",\"status_code\":201}\n"
    }

    fn sample_request_logs_arrow_ipc() -> Vec<u8> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
            Field::new("request_uuid", DataType::Utf8, false),
            Field::new("method", DataType::Utf8, false),
            Field::new("url", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(TimestampMillisecondArray::from(vec![1_735_689_600_000i64])),
                Arc::new(StringArray::from(vec!["abc-123"])),
                Arc::new(StringArray::from(vec!["GET"])),
                Arc::new(StringArray::from(vec!["/test"])),
            ],
        )
        .unwrap();

        let mut buf = Vec::new();
        let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
        buf
    }

    fn mock_request_logs_endpoint(
        server: &mut mockito::Server,
        app_id: i64,
        body: impl AsRef<[u8]>,
    ) -> mockito::Mock {
        server
            .mock(
                "POST",
                format!("/v1/apps/{app_id}/request-logs/stream").as_str(),
            )
            .with_status(200)
            .with_body(body)
            .create()
    }

    #[test]
    fn test_run_ndjson() {
        let mut server = mockito::Server::new();
        let mock = mock_request_logs_endpoint(&mut server, 1, sample_request_logs_ndjson());

        let mut buf = Vec::new();
        run(
            1,
            "2025-01-01",
            Some("2025-01-02"),
            Some(r#"["method","url","status_code"]"#),
            Some(r#"{"status_code":200}"#),
            Some(10),
            None,
            Some("test-key"),
            Some(&server.url()),
            &mut buf,
        )
        .unwrap();
        mock.assert();

        let rows = parse_ndjson(&buf);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["method"], "GET");
        assert_eq!(rows[0]["status_code"], 200);
        assert_eq!(rows[1]["method"], "POST");
        assert_eq!(rows[1]["url"], "/test2");
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let mock = mock_request_logs_endpoint(&mut server, 1, sample_request_logs_arrow_ipc());
        let (_dir, db_path) = temp_db();

        run(
            1,
            "2025-01-01",
            None,
            None,
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
            .query_row("SELECT count(*) FROM request_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let method: String = conn
            .query_row(
                "SELECT method FROM request_logs WHERE app_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(method, "GET");
    }
}
