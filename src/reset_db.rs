use std::path::Path;

use anyhow::Result;

use crate::utils::open_db;
use crate::{apps, consumers, endpoints, request_details, request_logs};

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
    request_logs::ensure_request_logs_table(&conn)?;
    request_details::ensure_application_logs_table(&conn)?;
    request_details::ensure_spans_table(&conn)?;

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

        // Seed with a pre-existing table and row
        let conn = open_db(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE apps (app_id INTEGER); INSERT INTO apps VALUES (1);")
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
    }
}
