use std::io::Write;
use std::path::Path;

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
    let url = format!("{api_base_url}/v1/apps");
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

pub fn run(
    db: Option<&Path>,
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let apps = fetch_apps(&api_key, &api_base_url)?;

    if let Some(db_path) = db {
        let conn = open_db(db_path)?;
        ensure_apps_tables(&conn)?;
        write_apps_to_db(&conn, &apps)?;
        eprintln!(
            "{} apps written to table 'apps' in {}...\nDone.",
            apps.len(),
            db_path.display(),
        );
    } else {
        for app in &apps {
            serde_json::to_writer(&mut writer, app)?;
            writeln!(writer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::open_db;
    use crate::utils::test_utils::{parse_ndjson, temp_db};

    fn sample_apps_json() -> &'static str {
        r#"{
            "data": [
                {
                    "id": 1,
                    "name": "Test App",
                    "framework": "FastAPI",
                    "client_id": "f57e1072-29d6-49be-a917-6eb8a2946832",
                    "envs": [
                        {
                            "id": 10,
                            "name": "prod",
                            "created_at": "2025-01-01T00:00:00Z",
                            "last_sync_at": "2025-06-01T12:00:00Z"
                        },
                        {
                            "id": 11,
                            "name": "dev",
                            "created_at": "2025-02-01T00:00:00Z",
                            "last_sync_at": null
                        }
                    ],
                    "created_at": "2025-01-01T00:00:00Z"
                }
            ]
        }"#
    }

    fn mock_apps_endpoint(server: &mut mockito::Server) -> mockito::Mock {
        server
            .mock("GET", "/v1/apps")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_apps_json())
            .create()
    }

    #[test]
    fn test_run_ndjson() {
        let mut server = mockito::Server::new();
        let mock = mock_apps_endpoint(&mut server);

        let mut buf = Vec::new();
        run(None, Some("test-key"), Some(&server.url()), &mut buf).unwrap();
        mock.assert();

        let rows = parse_ndjson(&buf);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "Test App");
        assert_eq!(rows[0]["framework"], "FastAPI");
        assert_eq!(rows[0]["envs"][0]["name"], "prod");
        assert!(rows[0]["envs"][1]["last_sync_at"].is_null());
    }

    #[test]
    fn test_run_with_db() {
        let mut server = mockito::Server::new();
        let mock = mock_apps_endpoint(&mut server);
        let (_dir, db_path) = temp_db();

        run(
            Some(&db_path),
            Some("test-key"),
            Some(&server.url()),
            Vec::new(),
        )
        .unwrap();
        mock.assert();

        let conn = open_db(&db_path).unwrap();

        let name: String = conn
            .query_row("SELECT name FROM apps WHERE app_id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "Test App");

        let env_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM app_envs WHERE app_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(env_count, 2);
    }
}
