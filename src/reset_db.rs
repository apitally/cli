use std::path::Path;

use anyhow::Result;

use crate::utils::open_db;
use crate::{apps, consumers, endpoints, metrics, request_details, request_logs, traces};

pub fn run(db: &Path) -> Result<()> {
    let conn = open_db(db)?;
    let tables: Vec<String> = conn
        .prepare("SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let drops: String = tables
        .iter()
        .map(|t| format!("DROP TABLE \"{t}\";"))
        .collect();
    conn.execute_batch(&drops)?;

    apps::ensure_apps_tables(&conn)?;
    consumers::ensure_consumers_table(&conn)?;
    endpoints::ensure_endpoints_table(&conn)?;
    metrics::ensure_metrics_table(&conn)?;
    request_logs::ensure_request_logs_table(&conn)?;
    request_details::ensure_application_logs_table(&conn)?;
    traces::ensure_spans_table(&conn)?;

    eprintln!("Database reset: {}", db.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;

    #[test]
    fn test_run() {
        let (_dir, db_path) = test_utils::temp_db();

        let conn = open_db(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (app_id INTEGER);
             INSERT INTO apps VALUES (1);
             CREATE TABLE spans (app_id INTEGER, request_uuid VARCHAR, span_id VARCHAR);
             INSERT INTO spans VALUES (1, 'abc-123', '00000000000000aa');",
        )
        .unwrap();
        drop(conn);

        run(&db_path).unwrap();

        let conn = open_db(&db_path).unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_schema = 'main' ORDER BY table_name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            tables,
            vec![
                "app_envs",
                "application_logs",
                "apps",
                "consumers",
                "endpoints",
                "metrics",
                "request_logs",
                "spans"
            ]
        );

        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM apps")
            .unwrap()
            .query_row([], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let columns: Vec<(String, String, String)> = conn
            .prepare(
                "SELECT column_name, data_type, is_nullable FROM information_schema.columns
                 WHERE table_name = 'spans' ORDER BY ordinal_position",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            columns,
            [
                ("app_id", "INTEGER", "NO"),
                ("trace_id", "VARCHAR", "NO"),
                ("span_id", "VARCHAR", "NO"),
                ("parent_span_id", "VARCHAR", "YES"),
                ("env", "VARCHAR", "YES"),
                ("name", "VARCHAR", "YES"),
                ("kind", "VARCHAR", "YES"),
                ("status", "VARCHAR", "YES"),
                ("start_time_ns", "BIGINT", "NO"),
                ("end_time_ns", "BIGINT", "YES"),
                ("duration_ns", "BIGINT", "YES"),
                ("attributes", "JSON", "YES"),
                ("events", "JSON", "YES"),
                ("scope_name", "VARCHAR", "YES"),
                ("scope_version", "VARCHAR", "YES"),
            ]
            .map(|(name, data_type, nullable)| (
                name.into(),
                data_type.into(),
                nullable.into()
            ))
        );
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM spans", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        conn.execute_batch(
            "INSERT OR REPLACE INTO spans (app_id, trace_id, span_id, start_time_ns) VALUES
                (1, '0000000000000000aaaaaaaaaaaaaaaa', '00000000000000aa', 1);
             INSERT OR REPLACE INTO spans (app_id, trace_id, span_id, start_time_ns) VALUES
                (1, '0000000000000000aaaaaaaaaaaaaaaa', '00000000000000aa', 2);",
        )
        .unwrap();
        let times: Vec<i64> = conn
            .prepare("SELECT start_time_ns FROM spans")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(times, [2]);
    }
}
