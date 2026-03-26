mod apps;
mod auth;
mod consumers;
mod request_details;
mod request_logs;
mod sql;
mod utils;
mod whoami;

use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use utils::CliError;

#[derive(Parser)]
#[command(name = "apitally", version, about = "Apitally CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args)]
struct ApiArgs {
    /// API key for authentication
    #[arg(long, env = "APITALLY_API_KEY", help_heading = "Authentication")]
    api_key: Option<String>,

    /// Base URL for the Apitally API
    #[arg(
        long,
        env = "APITALLY_API_BASE_URL",
        help_heading = "Authentication",
        hide = true
    )]
    api_base_url: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate with the Apitally API
    Auth {
        #[command(flatten)]
        api: ApiArgs,
    },

    /// Show the authenticated team
    Whoami {
        #[command(flatten)]
        api: ApiArgs,
    },

    /// List apps in your team
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `apps` and `app_envs` tables instead.
    Apps {
        #[command(flatten)]
        api: ApiArgs,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// List consumers for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `consumers` table instead.
    Consumers {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Filter to consumers that have made requests since this date/time (ISO 8601)
        #[arg(long)]
        requests_since: Option<String>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Retrieve request log data for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `request_logs` table instead.
    RequestLogs {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Since date/time (ISO 8601)
        #[arg(long)]
        since: String,

        /// Until date/time (ISO 8601, defaults to now)
        #[arg(long)]
        until: Option<String>,

        /// JSON array of field names to include
        ///
        /// Available fields: timestamp, request_uuid, env, method, path,
        /// url, consumer_id, request_headers, request_size_bytes, request_body_json,
        /// status_code, response_time_ms, response_headers, response_size_bytes,
        /// response_body_json, client_ip, client_country_iso_code, exception_type,
        /// exception_message, exception_stacktrace, sentry_event_id, trace_id.
        ///
        /// Always included: timestamp, request_uuid, method, url.
        ///
        /// Defaults to all fields except request_headers, request_body_json,
        /// response_headers, response_body_json, exception_type, exception_message,
        /// exception_stacktrace, sentry_event_id, trace_id.
        #[arg(long)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op", and "value" keys
        ///
        /// Supported operators:
        ///   string fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains
        ///   numeric fields: eq, neq, gt, gte, lt, lte, in, not_in
        ///   header fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains, exists, not_exists
        ///   ID fields: eq, neq, in, not_in
        ///
        /// For "in" and "not_in", "value" must be a JSON array. For header fields,
        /// use "key" for the header name. For "exists" and "not_exists", omit "value".
        ///
        /// Examples:
        ///   [{"field":"status_code","op":"gte","value":400}]
        ///   [{"field":"path","op":"ilike","value":"/users/%"}]
        ///   [{"field":"request_headers","key":"content-type","op":"eq","value":"application/json"}]
        ///   [{"field":"response_body","op":"ilike","value":"%error%"}]
        #[arg(long)]
        filters: Option<String>,

        /// Maximum number of rows to return
        #[arg(long)]
        limit: Option<i64>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Get details for a specific request
    ///
    /// Outputs a JSON object with full request details including headers, body,
    /// application logs, and spans.
    /// With --db, upserts the request into the `request_logs` table and inserts
    /// rows into the `application_logs` and `spans` tables.
    RequestDetails {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Request UUID
        request_uuid: String,

        /// Store results in DuckDB instead of outputting JSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Run a SQL query against local DuckDB
    ///
    /// Available tables: apps, app_envs, consumers, request_logs,
    /// application_logs, spans.
    Sql {
        /// SQL query to execute (reads from stdin if omitted)
        query: Option<String>,

        /// Path to DuckDB database file
        ///
        /// Defaults to ~/.apitally/data.duckdb.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        if std::io::stderr().is_terminal() {
            eprintln!("\x1b[1;31merror:\x1b[0m {err:#}");
        } else {
            eprintln!("error: {err:#}");
        }
        std::process::exit(exit_code(&err));
    }
}

fn exit_code(err: &anyhow::Error) -> i32 {
    for cause in err.chain() {
        if let Some(cli_err) = cause.downcast_ref::<CliError>() {
            return match cli_err {
                CliError::Auth(_) => 3,
                CliError::Input(_) => 4,
                CliError::Api(_) => 5,
            };
        }
    }
    1
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Auth { api } => {
            if api.api_key.is_none() && !std::io::stdin().is_terminal() {
                return Err(utils::auth_err(
                    "no API key provided. Use --api-key or set APITALLY_API_KEY",
                ));
            }
            auth::run(
                api.api_key,
                api.api_base_url,
                &auth::auth_file_path()?,
                &mut std::io::stdin(),
            )
        }
        Command::Whoami { api } => whoami::run(
            api.api_key.as_deref(),
            api.api_base_url.as_deref(),
            std::io::stdout().lock(),
        ),
        Command::Apps { api, db } => {
            let db = utils::resolve_db(db)?;
            apps::run(
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::Consumers {
            api,
            app_id,
            requests_since,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            consumers::run(
                app_id,
                requests_since.as_deref(),
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::RequestLogs {
            api,
            app_id,
            since,
            until,
            fields,
            filters,
            limit,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            request_logs::run(
                app_id,
                &since,
                until.as_deref(),
                fields.as_deref(),
                filters.as_deref(),
                limit,
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::RequestDetails {
            api,
            app_id,
            request_uuid,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            request_details::run(
                app_id,
                &request_uuid,
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::Sql { query, db } => {
            let db = db.map_or_else(utils::default_db_path, Ok)?;
            let query = match query {
                Some(q) => q,
                None => {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            sql::run(&query, &db, std::io::stdout().lock())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code() {
        // Each CliError variant maps to a specific exit code
        assert_eq!(exit_code(&utils::auth_err("test")), 3);
        assert_eq!(exit_code(&utils::input_err("test")), 4);
        assert_eq!(exit_code(&utils::api_err("test")), 5);

        // Non-CliError falls back to 1
        assert_eq!(exit_code(&anyhow::anyhow!("generic")), 1);
    }

    #[test]
    fn test_cli_parsing() {
        // Missing required args should fail
        assert!(Cli::try_parse_from(["apitally"]).is_err()); // missing command
        assert!(Cli::try_parse_from(["apitally", "consumers"]).is_err()); // missing app_id
        assert!(Cli::try_parse_from(["apitally", "request-logs", "42"]).is_err()); // missing --since
        assert!(Cli::try_parse_from(["apitally", "request-details", "42"]).is_err()); // missing request_uuid
        assert!(Cli::try_parse_from(["apitally", "sql", "SELECT 1", "--db"]).is_err()); // missing db path

        // Valid subcommands should parse correctly
        assert!(matches!(
            Cli::try_parse_from(["apitally", "auth"]).unwrap().command,
            Command::Auth { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "whoami"]).unwrap().command,
            Command::Whoami { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "apps"]).unwrap().command,
            Command::Apps { db: None, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "consumers", "42"])
                .unwrap()
                .command,
            Command::Consumers { app_id: 42, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "request-logs", "42", "--since", "2025-01-01"])
                .unwrap()
                .command,
            Command::RequestLogs { app_id: 42, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "apitally",
                "request-details",
                "42",
                "f328bb2a-93e1-4c4a-a263-47be6a1bcb15"
            ])
            .unwrap()
            .command,
            Command::RequestDetails {
                app_id: 42,
                ref request_uuid,
                ..
            } if request_uuid == "f328bb2a-93e1-4c4a-a263-47be6a1bcb15"
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "sql", "--db", "test.duckdb"])
                .unwrap()
                .command,
            Command::Sql { query: None, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "sql", "SELECT 1"])
                .unwrap()
                .command,
            Command::Sql { db: None, .. }
        ));

        // --db without a value should parse to Some(None)
        assert!(matches!(
            Cli::try_parse_from(["apitally", "apps", "--db"])
                .unwrap()
                .command,
            Command::Apps { db: Some(None), .. }
        ));
    }
}
