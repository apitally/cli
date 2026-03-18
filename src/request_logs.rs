use std::io;

use anyhow::Result;
use duckdb::arrow::ipc::reader::StreamReader;
use duckdb::vtab::arrow::{ArrowVTab, arrow_recordbatch_to_query_params};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_post, open_db};

fn ensure_request_logs_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS request_logs (
            app_id INTEGER NOT NULL,
            timestamp TIMESTAMP NOT NULL,
            request_uuid VARCHAR NOT NULL,
            app_env VARCHAR,
            method VARCHAR NOT NULL,
            path VARCHAR,
            url VARCHAR NOT NULL,
            consumer_id INTEGER,
            request_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[],
            request_size BIGINT,
            request_body VARCHAR,
            status_code INTEGER,
            response_time FLOAT,
            response_headers STRUCT(\"1\" VARCHAR, \"2\" VARCHAR)[],
            response_size BIGINT,
            response_body VARCHAR,
            client_ip VARCHAR,
            client_country_name VARCHAR,
            client_country_iso_code VARCHAR,
            exception_type VARCHAR,
            exception_message VARCHAR,
            exception_stacktrace VARCHAR,
            sentry_event_id VARCHAR,
            trace_id HUGEINT,
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
    db: Option<&str>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let conn = db.map(open_db).transpose()?;

    let format = if conn.is_some() { "arrow" } else { "ndjson" };
    let mut body = serde_json::json!({
        "format": format,
        "since": since,
    });
    if let Some(until) = until {
        body["until"] = serde_json::json!(until);
    }
    if let Some(fields) = fields {
        let fields_value: serde_json::Value = serde_json::from_str(fields)
            .map_err(|e| anyhow::anyhow!("Invalid JSON for --fields: {e}"))?;
        body["fields"] = fields_value;
    }
    if let Some(filters) = filters {
        let filters_value: serde_json::Value = serde_json::from_str(filters)
            .map_err(|e| anyhow::anyhow!("Invalid JSON for --filters: {e}"))?;
        body["filters"] = filters_value;
    }
    if let Some(limit) = limit {
        body["limit"] = serde_json::json!(limit);
    }
    let url = format!("{api_base_url}/v1/apps/{app_id}/request-logs/stream");
    let response = api_post(&url, &api_key, &body)?;

    if let (Some(conn), Some(db_path)) = (&conn, db) {
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
        let mut total = 0usize;
        for batch in reader {
            let batch = batch?;
            total += batch.num_rows();
            let params = arrow_recordbatch_to_query_params(batch);
            conn.execute(&insert_sql, params)?;
        }

        conn.execute_batch(
            "INSERT OR REPLACE INTO request_logs \
             SELECT * FROM request_logs_staging; \
             DROP TABLE request_logs_staging;",
        )?;

        eprintln!("Wrote {total} request log(s) to table 'request_logs' in {db_path}.",);
    } else {
        let mut stdout = io::stdout().lock();
        io::copy(&mut response.into_body().into_reader(), &mut stdout)?;
    }

    Ok(())
}
