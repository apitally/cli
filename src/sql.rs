use std::io::Write;
use std::path::Path;

use anyhow::{Result, bail};
use duckdb::arrow::json::writer::{LineDelimited, WriterBuilder};

use crate::utils::open_db;

fn map_db_err(e: duckdb::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

pub fn run(query: &str, db: &str, writer: impl Write) -> Result<()> {
    if !Path::new(db).exists() {
        bail!("Database file not found: {db}");
    }

    let conn = open_db(db)?;
    let mut stmt = conn.prepare(query).map_err(map_db_err)?;
    let batches = stmt.query_arrow([]).map_err(map_db_err)?;

    let mut json_writer = WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, LineDelimited>(writer);
    for batch in batches {
        json_writer.write(&batch)?;
    }
    json_writer.finish()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::open_db;
    use serde_json::Value;

    fn create_test_db() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db").to_str().unwrap().to_string();
        let conn = open_db(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE test (id INTEGER, name TEXT);
             INSERT INTO test VALUES (1, 'hello'), (2, NULL);",
        )
        .unwrap();
        (dir, db_path)
    }

    fn run_query(query: &str, db: &str) -> anyhow::Result<Vec<Value>> {
        let mut buf = Vec::new();
        run(query, db, &mut buf)?;
        let rows = String::from_utf8(buf)?
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        Ok(rows)
    }

    #[test]
    fn test_missing_db() {
        let err = run("SELECT 1", "/nonexistent/path.db", Vec::new()).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/path.db"));
    }

    #[test]
    fn test_invalid_query() {
        let (_dir, db_path) = create_test_db();
        assert!(run("SELECT * FROM nonexistent", &db_path, Vec::new()).is_err());
    }

    #[test]
    fn test_select_rows() {
        let (_dir, db_path) = create_test_db();
        let rows = run_query("SELECT id, name FROM test ORDER BY id", &db_path).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], 1);
        assert_eq!(rows[0]["name"], "hello");
        assert_eq!(rows[1]["id"], 2);
        assert!(rows[1]["name"].is_null());
    }

    #[test]
    fn test_empty_result() {
        let (_dir, db_path) = create_test_db();
        let rows = run_query("SELECT * FROM test WHERE false", &db_path).unwrap();
        assert!(rows.is_empty());
    }
}
