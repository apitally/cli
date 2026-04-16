use std::io;
use std::path::Path;

use anyhow::Result;
use duckdb::arrow::ipc::reader::StreamReader;
use duckdb::vtab::arrow::{ArrowVTab, arrow_recordbatch_to_query_params};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_post, input_err, open_db, parse_string_list, resolve_relative_datetime};

pub(crate) fn ensure_metrics_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics (
            app_id              INTEGER NOT NULL,
            period_start        TIMESTAMPTZ NOT NULL,
            period_end          TIMESTAMPTZ NOT NULL,
            env                 VARCHAR,
            consumer_id         BIGINT,
            method              VARCHAR,
            path                VARCHAR,
            status_code         INTEGER,
            requests            BIGINT,
            requests_per_minute DOUBLE,
            bytes_received      BIGINT,
            bytes_sent          BIGINT,
            client_errors       BIGINT,
            server_errors       BIGINT,
            error_rate          DOUBLE,
            response_time_p50   INTEGER,
            response_time_p75   INTEGER,
            response_time_p95   INTEGER
        )",
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    app_id: i64,
    since: &str,
    until: Option<&str>,
    metrics: &str,
    interval: Option<&str>,
    group_by: Option<&str>,
    filters: Option<&str>,
    timezone: Option<&str>,
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
        "metrics": parse_string_list(metrics).map_err(|e| input_err(format!("invalid JSON for --metrics: {e}")))?,
    });
    if let Some(ref until) = until {
        body["until"] = serde_json::json!(until);
    }
    if let Some(interval) = interval {
        body["interval"] = serde_json::json!(interval);
    }
    if let Some(group_by) = group_by {
        body["group_by"] = parse_string_list(group_by)
            .map_err(|e| input_err(format!("invalid JSON for --group-by: {e}")))?;
    }
    if let Some(filters) = filters {
        let filters_value: serde_json::Value = serde_json::from_str(filters)
            .map_err(|e| input_err(format!("invalid JSON for --filters: {e}")))?;
        body["filters"] = filters_value;
    }
    let timezone = match timezone {
        Some(tz) => tz.to_owned(),
        None => iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_owned()),
    };
    body["timezone"] = serde_json::json!(timezone);
    let url = format!("{api_base_url}/v1/apps/{app_id}/metrics");
    let response = api_post(&url, &api_key, &body)?;

    if let Some((db_path, conn)) = &db {
        conn.register_table_function::<ArrowVTab>("arrow")?;
        ensure_metrics_table(conn)?;

        conn.execute_batch(
            "CREATE TEMPORARY TABLE metrics_staging AS SELECT * FROM metrics LIMIT 0",
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
            "INSERT INTO metrics_staging (app_id, {col_list}) \
             SELECT {app_id}, {col_list} FROM arrow(?, ?)"
        );

        const CHUNK_SIZE: usize = 2048; // DuckDB's vector size
        let mut total = 0usize;

        eprint!(
            "0 metrics rows written to table 'metrics' in {}...",
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
                "\r{total} metrics rows written to table 'metrics' in {}...",
                db_path.display()
            );
        }

        // Delete existing rows that overlap with the time range being inserted,
        // then move staged data into the main table.
        conn.execute_batch(&format!(
            "DELETE FROM metrics WHERE app_id = {app_id} \
                 AND period_start >= (SELECT MIN(period_start) FROM metrics_staging) \
                 AND period_end <= (SELECT MAX(period_end) FROM metrics_staging); \
             INSERT INTO metrics BY NAME SELECT * FROM metrics_staging; \
             DROP TABLE metrics_staging;"
        ))?;

        eprintln!("\nDone.");
    } else {
        io::copy(&mut response.into_body().into_reader(), &mut writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use duckdb::arrow::array::{Float64Array, Int64Array, StringArray, TimestampMillisecondArray};
    use duckdb::arrow::datatypes::{DataType, Field, Schema, TimeUnit};
    use duckdb::arrow::ipc::writer::StreamWriter;
    use duckdb::arrow::record_batch::RecordBatch;

    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_metrics_ndjson() -> &'static str {
        "{\"period_start\":\"2025-01-01T00:00:00Z\",\"period_end\":\"2025-01-01T01:00:00Z\",\"method\":\"GET\",\"requests\":100,\"error_rate\":0.05}\n\
         {\"period_start\":\"2025-01-01T01:00:00Z\",\"period_end\":\"2025-01-01T02:00:00Z\",\"method\":\"POST\",\"requests\":50,\"error_rate\":0.1}\n"
    }

    fn sample_metrics_arrow_ipc() -> Vec<u8> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "period_start",
                DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
                false,
            ),
            Field::new(
                "period_end",
                DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
                false,
            ),
            Field::new("method", DataType::Utf8, true),
            Field::new("requests", DataType::Int64, false),
            Field::new("error_rate", DataType::Float64, false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(
                    TimestampMillisecondArray::from(vec![1_735_689_600_000i64])
                        .with_timezone("UTC"),
                ),
                Arc::new(
                    TimestampMillisecondArray::from(vec![1_735_693_200_000i64])
                        .with_timezone("UTC"),
                ),
                Arc::new(StringArray::from(vec![Some("GET")])),
                Arc::new(Int64Array::from(vec![100])),
                Arc::new(Float64Array::from(vec![0.05])),
            ],
        )
        .unwrap();

        let mut buf = Vec::new();
        let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
        buf
    }

    fn mock_metrics_endpoint(
        server: &mut mockito::Server,
        app_id: i64,
        body: impl AsRef<[u8]>,
    ) -> mockito::Mock {
        server
            .mock("POST", format!("/v1/apps/{app_id}/metrics").as_str())
            .with_status(200)
            .with_body(body)
            .create()
    }

    #[test]
    fn test_run_ndjson() {
        let mut server = mockito::Server::new();
        let mock = mock_metrics_endpoint(&mut server, 1, sample_metrics_ndjson());

        let mut buf = Vec::new();
        run(
            1,
            "2025-01-01",
            Some("2025-01-02"),
            r#"["requests","error_rate"]"#,
            Some("hour"),
            Some(r#"["method"]"#),
            None,
            Some("Australia/Brisbane"),
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
        assert_eq!(rows[0]["requests"], 100);
        assert_eq!(rows[1]["method"], "POST");
        assert_eq!(rows[1]["requests"], 50);
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let mock = mock_metrics_endpoint(&mut server, 1, sample_metrics_arrow_ipc());
        let (_dir, db_path) = temp_db();

        run(
            1,
            "2025-01-01",
            None,
            r#"["requests","error_rate"]"#,
            Some("hour"),
            Some(r#"["method"]"#),
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
            .query_row("SELECT count(*) FROM metrics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (method, requests, error_rate): (String, i64, f64) = conn
            .query_row(
                "SELECT method, requests, error_rate FROM metrics WHERE app_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(method, "GET");
        assert_eq!(requests, 100);
        assert!((error_rate - 0.05).abs() < f64::EPSILON);
    }
}
