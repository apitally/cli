use std::io;
use std::path::Path;

use anyhow::Result;
use duckdb::arrow::ipc::reader::StreamReader;
use duckdb::vtab::arrow::{ArrowVTab, arrow_recordbatch_to_query_params};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_post, input_err, open_db, parse_string_list, resolve_relative_datetime};

#[allow(clippy::too_many_arguments)]
pub fn run(
    app_id: i64,
    since: Option<&str>,
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

    let format = if db.is_some() { "arrow" } else { "ndjson" };
    let mut body = serde_json::json!({"format": format});
    if let Some(since) = since {
        body["since"] = resolve_relative_datetime(since).into();
    }
    if let Some(until) = until {
        body["until"] = resolve_relative_datetime(until).into();
    }
    if let Some(fields) = fields {
        body["fields"] = parse_string_list(fields)
            .map_err(|e| input_err(format!("invalid JSON for --fields: {e}")))?;
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
    let url = format!("{api_base_url}/v1/apps/{app_id}/traces");
    let response = api_post(&url, &api_key, &body)?;

    if let Some((db_path, conn)) = &db {
        conn.register_table_function::<ArrowVTab>("arrow")?;
        ensure_spans_table(conn)?;
        conn.execute_batch("CREATE TEMPORARY TABLE spans_staging AS SELECT * FROM spans LIMIT 0")?;

        let reader = StreamReader::try_new(response.into_body().into_reader(), None)?;
        let schema = reader.schema();
        let col_list = schema
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let insert_sql = format!(
            "INSERT INTO spans_staging (app_id, {col_list}) \
             SELECT {app_id}, {col_list} FROM arrow(?, ?)"
        );

        let mut total = 0usize;
        eprint!(
            "0 spans written to table 'spans' in {}...",
            db_path.display()
        );

        for batch in reader {
            let batch = batch?;
            total += batch.num_rows();
            let params = arrow_recordbatch_to_query_params(batch);
            conn.execute(&insert_sql, params)?;
            eprint!(
                "\r{total} spans written to table 'spans' in {}...",
                db_path.display()
            );
        }

        conn.execute_batch(
            "INSERT OR REPLACE INTO spans BY NAME SELECT * FROM spans_staging; \
             DROP TABLE spans_staging;",
        )?;
        eprintln!("\nDone.");
    } else {
        io::copy(&mut response.into_body().into_reader(), &mut writer)?;
    }

    Ok(())
}

pub(crate) fn ensure_spans_table(conn: &duckdb::Connection) -> Result<()> {
    let legacy: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
         WHERE table_schema = 'main' AND table_name = 'spans' AND column_name = 'request_uuid')",
        [],
        |row| row.get(0),
    )?;
    if legacy {
        return Err(input_err(
            "The spans table uses an incompatible schema. Run `apitally reset-db --db <path-to-this-database>` \
             and refetch, or use a new database file to keep existing data. Reset clears ALL tables. \
             Data beyond API retention may not be available to refetch.",
        ));
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS spans (
            app_id INTEGER NOT NULL,
            trace_id VARCHAR NOT NULL,
            span_id VARCHAR NOT NULL,
            parent_span_id VARCHAR,
            env VARCHAR,
            name VARCHAR,
            kind VARCHAR,
            status VARCHAR,
            start_time_ns BIGINT NOT NULL,
            end_time_ns BIGINT,
            duration_ns BIGINT,
            attributes JSON,
            events JSON,
            scope_name VARCHAR,
            scope_version VARCHAR,
            UNIQUE (app_id, trace_id, span_id)
        )",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use duckdb::arrow::array::{Int64Array, StringArray};
    use duckdb::arrow::compute::concat_batches;
    use duckdb::arrow::datatypes::{DataType, Field, Schema};
    use duckdb::arrow::ipc::writer::StreamWriter;
    use duckdb::arrow::record_batch::RecordBatch;
    use serde_json::json;

    use super::*;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    const ALL_FIELDS: &str = "trace_id,span_id,parent_span_id,env,name,kind,status,start_time_ns,end_time_ns,duration_ns,attributes,events,scope_name,scope_version";

    fn sample_traces_ndjson() -> &'static str {
        r#"{"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0000000000000001","start_time_ns":1767225600123456001,"parent_span_id":null,"env":"prod","name":"GET /books","kind":"SERVER","status":"UNSET","end_time_ns":1767225600373456001,"duration_ns":250000000,"attributes":{"http.route":"/books","retry.count":2,"cached":true,"details":{"key":null},"labels":["a",1]},"events":[{"timestamp":"2026-01-01T00:00:00.123456001Z","name":"exception","attributes":{"exception.type":"TimeoutError","code":503}}],"scope_name":"test.instrumentation","scope_version":"1.2.3"}
{"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0000000000000002","start_time_ns":1767225600123457001,"parent_span_id":"0000000000000001","env":null,"name":"db.query","kind":"CLIENT","status":"ERROR","end_time_ns":1767225600323457001,"duration_ns":200000000,"attributes":{},"events":[],"scope_name":null,"scope_version":null}
"#
    }

    fn sample_traces_batch(field_count: usize, row_count: usize) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("trace_id", DataType::Utf8, false),
            Field::new("span_id", DataType::Utf8, false),
            Field::new("start_time_ns", DataType::Int64, false),
            Field::new("parent_span_id", DataType::Utf8, true),
            Field::new("env", DataType::Utf8, true),
            Field::new("name", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, false),
            Field::new("status", DataType::Utf8, false),
            Field::new("end_time_ns", DataType::Int64, false),
            Field::new("duration_ns", DataType::Int64, false),
            Field::new("attributes", DataType::Utf8, false),
            Field::new("events", DataType::Utf8, false),
            Field::new("scope_name", DataType::Utf8, true),
            Field::new("scope_version", DataType::Utf8, true),
        ]));
        let rows = parse_ndjson(sample_traces_ndjson().as_bytes());
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec![
                    "0123456789abcdef0123456789abcdef",
                    "0123456789abcdef0123456789abcdef",
                ])),
                Arc::new(StringArray::from(vec![
                    "0000000000000001",
                    "0000000000000002",
                ])),
                Arc::new(Int64Array::from(vec![
                    1767225600123456001,
                    1767225600123457001,
                ])),
                Arc::new(StringArray::from(vec![None, Some("0000000000000001")])),
                Arc::new(StringArray::from(vec![Some("prod"), None])),
                Arc::new(StringArray::from(vec!["GET /books", "db.query"])),
                Arc::new(StringArray::from(vec!["SERVER", "CLIENT"])),
                Arc::new(StringArray::from(vec!["UNSET", "ERROR"])),
                Arc::new(Int64Array::from(vec![
                    1767225600373456001,
                    1767225600323457001,
                ])),
                Arc::new(Int64Array::from(vec![250000000, 200000000])),
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row["attributes"].to_string()),
                )),
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row["events"].to_string()),
                )),
                Arc::new(StringArray::from(vec![Some("test.instrumentation"), None])),
                Arc::new(StringArray::from(vec![Some("1.2.3"), None])),
            ],
        )
        .unwrap()
        .project(&(0..field_count).collect::<Vec<_>>())
        .unwrap()
        .slice(0, row_count)
    }

    #[test]
    fn test_run_ndjson() {
        let filters =
            r#"[{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]"#;
        let response = sample_traces_ndjson();
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/apps/42/traces")
            .match_header("api-key", "test-key")
            .match_body(mockito::Matcher::Json(json!({
                "format": "ndjson", "fields": ["attributes", "events"],
                "filters": serde_json::from_str::<serde_json::Value>(filters).unwrap()
            })))
            .with_body(response)
            .create();
        let mut output = Vec::new();
        run(
            42,
            None,
            None,
            Some("attributes,events"),
            Some(filters),
            None,
            None,
            None,
            Some("test-key"),
            Some(&server.url()),
            &mut output,
        )
        .unwrap();
        mock.assert();
        assert_eq!(output, response.as_bytes());

        for (sample, expected) in [("1000", json!(1000)), ("0.1", json!(0.1))] {
            let mock = server
                .mock("POST", "/v1/apps/42/traces")
                .match_body(mockito::Matcher::Json(json!({
                    "format": "ndjson", "since": "2026-01-01", "until": "2026-01-02",
                    "sample": expected, "limit": 100
                })))
                .with_body("")
                .create();
            run(
                42,
                Some("2026-01-01"),
                Some("2026-01-02"),
                None,
                None,
                Some(sample),
                Some(100),
                None,
                Some("test-key"),
                Some(&server.url()),
                Vec::new(),
            )
            .unwrap();
            mock.assert();
        }
    }

    #[test]
    fn test_run_input_errors() {
        for (fields, filters, sample, message) in [
            (Some("[invalid]"), None, None, "--fields"),
            (None, Some("[invalid]"), None, "--filters"),
            (None, None, Some("0"), "integer"),
            (None, None, Some("0.0"), "float"),
            (None, None, Some("0.6"), "float"),
            (None, None, Some("NaN"), "float"),
            (None, None, Some("many"), "integer or float"),
        ] {
            let err = run(
                42,
                None,
                None,
                fields,
                filters,
                sample,
                None,
                None,
                Some("test-key"),
                Some("http://127.0.0.1:1"),
                Vec::new(),
            )
            .unwrap_err();
            assert_eq!(crate::exit_code(&err), 4);
            assert!(err.to_string().contains(message), "{err}");
        }
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let (_dir, db_path) = temp_db();
        let batch = sample_traces_batch(14, 2);
        fetch_spans(&mut server, &db_path, &batch, ALL_FIELDS);
        let conn = open_db(&db_path).unwrap();
        let mut expected = parse_ndjson(sample_traces_ndjson().as_bytes());
        let stored: Vec<String> = conn
            .prepare("SELECT to_json(spans) FROM spans ORDER BY start_time_ns")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(stored.len(), expected.len());
        for (stored, expected) in stored.iter().zip(&mut expected) {
            let stored: serde_json::Value = serde_json::from_str(stored).unwrap();
            expected["app_id"] = json!(42);
            assert_eq!(&stored, expected);
        }
        conn.execute_batch(
            "INSERT INTO spans (app_id, trace_id, span_id, start_time_ns, name) VALUES \
            (42, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '0000000000000001', 1, 'other trace'), \
            (99, '0123456789abcdef0123456789abcdef', '0000000000000001', 1, 'other app')",
        )
        .unwrap();
        drop(conn);

        fetch_spans(&mut server, &db_path, &sample_traces_batch(3, 2), "[]");
        let conn = open_db(&db_path).unwrap();
        let cleared: i64 = conn.query_row("SELECT count(*) FROM spans WHERE app_id = 42 \
            AND name IS NULL AND parent_span_id IS NULL AND env IS NULL AND kind IS NULL AND status IS NULL \
            AND end_time_ns IS NULL AND duration_ns IS NULL AND attributes IS NULL AND events IS NULL \
            AND scope_name IS NULL AND scope_version IS NULL", [], |row| row.get(0)).unwrap();
        assert_eq!(cleared, 2);
        drop(conn);
        fetch_spans(
            &mut server,
            &db_path,
            &sample_traces_batch(14, 0),
            ALL_FIELDS,
        );
        let conn = open_db(&db_path).unwrap();
        let (total, retained): (i64, i64) = conn.query_row("SELECT count(*), count(*) FILTER (WHERE name IN ('other trace', 'other app')) FROM spans",
            [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!((total, retained), (4, 2));

        let (_empty_dir, empty_db) = temp_db();
        fetch_spans(
            &mut server,
            &empty_db,
            &sample_traces_batch(14, 0),
            ALL_FIELDS,
        );
        let count: i64 = open_db(&empty_db)
            .unwrap()
            .query_row("SELECT count(*) FROM spans", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_run_with_db_large_batch() {
        let first = sample_traces_batch(14, 1);
        let schema = first.schema();
        // Cross DuckDB's 2,048-row vector boundary.
        let batch = concat_batches(&schema, std::iter::repeat_n(&first, 2049)).unwrap();
        let mut columns = batch.columns().to_vec();
        columns[schema.index_of("span_id").unwrap()] = Arc::new(StringArray::from(
            (0..2049).map(|i| format!("{i:016x}")).collect::<Vec<_>>(),
        ));
        let batch = RecordBatch::try_new(schema, columns).unwrap();
        let mut server = mockito::Server::new();
        let (_dir, db_path) = temp_db();
        fetch_spans(&mut server, &db_path, &batch, ALL_FIELDS);
        let conn = open_db(&db_path).unwrap();
        let (count, timestamps): (i64, i64) = conn.query_row(
            "SELECT count(*), count(*) FILTER (WHERE start_time_ns = 1767225600123456001) FROM spans",
            [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!((count, timestamps), (2049, 2049));
    }

    fn fetch_spans(server: &mut mockito::Server, db: &Path, batch: &RecordBatch, fields: &str) {
        let mut ipc = Vec::new();
        let mut writer = StreamWriter::try_new(&mut ipc, &batch.schema()).unwrap();
        if batch.num_rows() > 0 {
            writer.write(batch).unwrap();
        }
        writer.finish().unwrap();
        let mock = server
            .mock("POST", "/v1/apps/42/traces")
            .match_header("api-key", "test-key")
            .match_body(mockito::Matcher::Json(json!({
                "format": "arrow", "since": "2026-01-01",
                "fields": parse_string_list(fields).unwrap()
            })))
            .with_body(ipc)
            .create();
        let mut output = Vec::new();
        run(
            42,
            Some("2026-01-01"),
            None,
            Some(fields),
            None,
            None,
            None,
            Some(db),
            Some("test-key"),
            Some(&server.url()),
            &mut output,
        )
        .unwrap();
        mock.assert();
        mock.remove();
        assert!(output.is_empty());
    }
}
