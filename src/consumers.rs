use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_get, open_db};

#[derive(Deserialize)]
struct ConsumersResponse {
    data: Vec<ConsumerItem>,
    has_more: bool,
    next_token: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ConsumerItem {
    id: i64,
    identifier: String,
    name: String,
    group: Option<ConsumerGroupItem>,
    created_at: String,
    last_request_at: String,
}

#[derive(Deserialize, Serialize)]
struct ConsumerGroupItem {
    id: i64,
    name: String,
}

fn fetch_consumers_page(
    api_key: &str,
    api_base_url: &str,
    app_id: i64,
    requests_since: Option<&str>,
    next_token: Option<&str>,
) -> Result<ConsumersResponse> {
    let url = format!("{}/v1/apps/{}/consumers", api_base_url, app_id);
    let mut query = vec![("limit", "1000")];
    if let Some(since) = requests_since {
        query.push(("requests_since", since));
    }
    if let Some(token) = next_token {
        query.push(("next_token", token));
    }
    let mut response = api_get(&url, api_key, &query)?;
    let page: ConsumersResponse = response.body_mut().read_json()?;
    Ok(page)
}

fn ensure_consumers_table(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS consumers (
            app_id INTEGER NOT NULL,
            consumer_id INTEGER NOT NULL,
            identifier TEXT NOT NULL,
            name TEXT NOT NULL,
            \"group\" TEXT,
            created_at TIMESTAMPTZ NOT NULL,
            last_request_at TIMESTAMPTZ,
            UNIQUE (app_id, consumer_id)
        )",
    )?;
    Ok(())
}

fn write_consumers_to_db(
    conn: &duckdb::Connection,
    app_id: i64,
    consumers: &[ConsumerItem],
) -> Result<()> {
    let mut stmt = conn.prepare("INSERT OR REPLACE INTO consumers VALUES (?, ?, ?, ?, ?, ?, ?)")?;
    for consumer in consumers {
        let group_name = consumer.group.as_ref().map(|g| g.name.as_str());
        stmt.execute(duckdb::params![
            app_id,
            consumer.id,
            consumer.identifier,
            consumer.name,
            group_name,
            consumer.created_at,
            consumer.last_request_at,
        ])?;
    }
    Ok(())
}

pub fn run(
    app_id: i64,
    requests_since: Option<&str>,
    db: Option<&str>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let conn = db.map(open_db).transpose()?;

    if let Some(conn) = &conn {
        ensure_consumers_table(conn)?;
    }

    let mut next_token: Option<String> = None;
    let mut total = 0usize;

    loop {
        let page = fetch_consumers_page(
            &api_key,
            &api_base_url,
            app_id,
            requests_since,
            next_token.as_deref(),
        )?;
        total += page.data.len();

        if let Some(conn) = &conn {
            write_consumers_to_db(conn, app_id, &page.data)?;
        } else {
            for consumer in &page.data {
                println!("{}", serde_json::to_string(consumer)?);
            }
        }

        if page.has_more {
            next_token = page.next_token;
        } else {
            break;
        }
    }

    if let Some(db_path) = db {
        eprintln!("Wrote {total} consumer(s) to table 'consumers' in {db_path}.");
    }

    Ok(())
}
