use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::{api_get, open_db};

#[derive(Deserialize)]
struct AppsResponse {
    data: Vec<AppItem>,
}

#[derive(Deserialize, Serialize)]
struct AppItem {
    id: i64,
    name: String,
    framework: String,
    client_id: String,
    envs: Vec<AppEnvItem>,
    created_at: String,
}

#[derive(Deserialize, Serialize)]
struct AppEnvItem {
    id: i64,
    name: String,
    created_at: String,
    last_sync_at: Option<String>,
}

fn fetch_apps(api_key: &str, api_base_url: &str) -> Result<Vec<AppItem>> {
    let url = format!("{}/v1/apps", api_base_url);
    let mut response = api_get(&url, api_key, &[])?;
    let apps: AppsResponse = response.body_mut().read_json()?;
    Ok(apps.data)
}

fn ensure_apps_tables(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS apps (
            app_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            framework TEXT NOT NULL,
            client_id TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL,
            UNIQUE (app_id)
        );
        CREATE TABLE IF NOT EXISTS app_envs (
            app_id INTEGER NOT NULL,
            app_env_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL,
            last_sync_at TIMESTAMPTZ,
            UNIQUE (app_id, app_env_id)
        );",
    )?;
    Ok(())
}

fn write_apps_to_db(conn: &duckdb::Connection, apps: &[AppItem]) -> Result<()> {
    let mut app_stmt = conn.prepare("INSERT OR REPLACE INTO apps VALUES (?, ?, ?, ?, ?)")?;
    let mut env_stmt = conn.prepare("INSERT OR REPLACE INTO app_envs VALUES (?, ?, ?, ?, ?)")?;
    for app in apps {
        app_stmt.execute(duckdb::params![
            app.id,
            app.name,
            app.framework,
            app.client_id,
            app.created_at,
        ])?;
        for env in &app.envs {
            env_stmt.execute(duckdb::params![
                app.id,
                env.id,
                env.name,
                env.created_at,
                env.last_sync_at,
            ])?;
        }
    }
    Ok(())
}

pub fn run(db: Option<&str>, api_key: Option<&str>, api_base_url: Option<&str>) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let apps = fetch_apps(&api_key, &api_base_url)?;

    if let Some(db_path) = db {
        let conn = open_db(db_path)?;
        ensure_apps_tables(&conn)?;
        write_apps_to_db(&conn, &apps)?;
        eprintln!("Wrote {} app(s) to table 'apps' in {db_path}.", apps.len(),);
    } else {
        for app in &apps {
            println!("{}", serde_json::to_string(app)?);
        }
    }

    Ok(())
}
