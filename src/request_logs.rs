use std::io;
use std::path::Path;

use anyhow::Result;
use duckdb::arrow::ipc::reader::StreamReader;
use duckdb::vtab::arrow::{ArrowVTab, arrow_recordbatch_to_query_params};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_post, input_err, open_db, resolve_relative_datetime};

pub(crate) fn ensure_request_logs_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS request_logs (
            app_id INTEGER NOT NULL,
            timestamp TIMESTAMPTZ NOT NULL,
            request_uuid VARCHAR NOT NULL,
            env VARCHAR,
            method VARCHAR NOT NULL,
            path VARCHAR,
            url VARCHAR NOT NULL,
            consumer_id INTEGER,
            request_headers STRUCT(name VARCHAR, value VARCHAR)[],
            request_size_bytes BIGINT,
            request_body_json JSON,
            status_code INTEGER,
            response_time_ms INTEGER,
            response_headers STRUCT(name VARCHAR, value VARCHAR)[],
            response_size_bytes BIGINT,
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
    sample: Option<&str>,
    limit: Option<i64>,
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl io::Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let db = db.map(|p| open_db(p).map(|c| (p, c))).transpose()?;
    let since = resolve_relative_datetime(since);
    let until = until.map(resolve_relative_datetime);

    let format = if db.is_some() { "arrow" } else { "ndjson" };
    let mut body = serde_json::json!({
        "format": format,
        "since": since,
    });
    if let Some(ref until) = until {
        body["until"] = serde_json::json!(until);
    }
    if let Some(fields) = fields {
        let fields_value: serde_json::Value = serde_json::from_str(fields)
            .map_err(|e| input_err(format!("invalid JSON for --fields: {e}")))?;
        body["fields"] = fields_value;
    }
    if let Some(filters) = filters {
        let filters_value: serde_json::Value = serde_json::from_str(filters)
            .map_err(|e| input_err(format!("invalid JSON for --filters: {e}")))?;
        body["filters"] = filters_value;
    }
    if let Some(sample) = sample {
        if let Ok(n) = sample.parse::<i64>() {
            if n < 1 {
                return Err(input_err("--sample as integer must be greater than 0"));
            }
            body["sample"] = serde_json::json!(n);
        } else if let Ok(f) = sample.parse::<f64>() {
            if !f.is_finite() || f <= 0.0 || f > 0.5 {
                return Err(input_err(
                    "--sample as float must be between 0 (exclusive) and 0.5 (inclusive)",
                ));
            }
            body["sample"] = serde_json::json!(f);
        } else {
            return Err(input_err("--sample must be an integer or float"));
        }
    }
    if let Some(limit) = limit {
        body["limit"] = serde_json::json!(limit);
    }
    let url = format!("{api_base_url}/v1/apps/{app_id}/request-logs");
    let response = api_post(&url, &api_key, &body)?;

    if let Some((db_path, conn)) = &db {
        conn.register_table_function::<ArrowVTab>("arrow")?;
        ensure_request_logs_table(conn)?;

        // Arrow IPC serializes header tuples as structs with fields "1" and "2",
        // so the staging table must use those names to accept the Arrow data.
        // The final INSERT into request_logs renames them to "name" and "value".
        conn.execute_batch(
            "CREATE TEMPORARY TABLE request_logs_staging AS \
             SELECT * EXCLUDE (request_headers, response_headers) FROM request_logs LIMIT 0; \
             ALTER TABLE request_logs_staging ADD COLUMN request_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[]; \
             ALTER TABLE request_logs_staging ADD COLUMN response_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[];",
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
            "INSERT OR REPLACE INTO request_logs BY NAME \
             SELECT * REPLACE (\
                 [{'name': s.\"1\", 'value': s.\"2\"} FOR s IN request_headers] AS request_headers, \
                 [{'name': s.\"1\", 'value': s.\"2\"} FOR s IN response_headers] AS response_headers \
             ) FROM request_logs_staging; \
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

    use duckdb::arrow::array::{ListArray, StringArray, StructArray, TimestampMillisecondArray};
    use duckdb::arrow::datatypes::{DataType, Field, Fields, Schema, TimeUnit};
    use duckdb::arrow::ipc::writer::StreamWriter;
    use duckdb::arrow::record_batch::RecordBatch;

    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_request_logs_ndjson() -> &'static str {
        "{\"timestamp\":\"2025-01-01T00:00:00Z\",\"request_uuid\":\"abc\",\"method\":\"GET\",\"url\":\"https://api.example.com/test\",\"status_code\":200}\n\
         {\"timestamp\":\"2025-01-01T00:01:00Z\",\"request_uuid\":\"def\",\"method\":\"POST\",\"url\":\"https://api.example.com/test2\",\"status_code\":201}\n"
    }

    fn sample_request_logs_arrow_ipc() -> Vec<u8> {
        let header_fields = Fields::from(vec![
            Field::new("1", DataType::Utf8, false),
            Field::new("2", DataType::Utf8, false),
        ]);
        let header_struct_type = DataType::Struct(header_fields.clone());
        let headers_list_type =
            DataType::List(Arc::new(Field::new_list_field(header_struct_type, true)));
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
                false,
            ),
            Field::new("request_uuid", DataType::Utf8, false),
            Field::new("method", DataType::Utf8, false),
            Field::new("url", DataType::Utf8, false),
            Field::new("request_headers", headers_list_type.clone(), true),
        ]));
        let headers_struct = StructArray::from(vec![
            (
                Arc::new(Field::new("1", DataType::Utf8, false)),
                Arc::new(StringArray::from(vec!["content-type"])) as _,
            ),
            (
                Arc::new(Field::new("2", DataType::Utf8, false)),
                Arc::new(StringArray::from(vec!["application/json"])) as _,
            ),
        ]);
        let headers_list = ListArray::new(
            Arc::new(Field::new_list_field(DataType::Struct(header_fields), true)),
            duckdb::arrow::buffer::OffsetBuffer::from_lengths([1]),
            Arc::new(headers_struct),
            None,
        );
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(
                    TimestampMillisecondArray::from(vec![1_735_689_600_000i64])
                        .with_timezone("UTC"),
                ),
                Arc::new(StringArray::from(vec!["abc-123"])),
                Arc::new(StringArray::from(vec!["GET"])),
                Arc::new(StringArray::from(vec!["https://api.example.com/test"])),
                Arc::new(headers_list),
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
            .mock("POST", format!("/v1/apps/{app_id}/request-logs").as_str())
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
            None,
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
        assert_eq!(rows[1]["url"], "https://api.example.com/test2");
    }

    #[test]
    fn test_run_ndjson_with_sample() {
        for (sample, expected_json) in
            [("500", r#"{"sample":500}"#), ("0.25", r#"{"sample":0.25}"#)]
        {
            let mut server = mockito::Server::new();
            let mock = server
                .mock("POST", "/v1/apps/1/request-logs")
                .match_body(mockito::Matcher::PartialJsonString(expected_json.into()))
                .with_status(200)
                .with_body(sample_request_logs_ndjson())
                .create();

            run(
                1,
                "2025-01-01",
                None,
                None,
                None,
                Some(sample),
                None,
                None,
                Some("test-key"),
                Some(&server.url()),
                &mut Vec::new(),
            )
            .unwrap();
            mock.assert();
        }
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

        let (url, header_name): (String, Option<String>) = conn
            .query_row(
                "SELECT url, request_headers[1].name FROM request_logs WHERE app_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(url, "https://api.example.com/test");
        assert_eq!(header_name.as_deref(), Some("content-type"));
    }
}
