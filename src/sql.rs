use std::path::Path;

use anyhow::{Result, bail};
use duckdb::arrow::json::writer::{LineDelimited, WriterBuilder};

use crate::utils::open_db;

fn map_db_err(e: duckdb::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

pub fn run(query: &str, db: &str) -> Result<()> {
    if !Path::new(db).exists() {
        bail!("Database file not found: {db}");
    }

    let conn = open_db(db)?;
    let mut stmt = conn.prepare(query).map_err(map_db_err)?;
    let batches = stmt.query_arrow([]).map_err(map_db_err)?;

    let stdout = std::io::stdout();
    let mut writer = WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, LineDelimited>(stdout.lock());
    for batch in batches {
        writer.write(&batch)?;
    }
    writer.finish()?;

    Ok(())
}
