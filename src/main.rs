mod apps;
mod auth;
mod consumers;
mod request_logs;
mod sql;
mod utils;

use std::io::Read;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "apitally", version, about = "Apitally CLI")]
struct Cli {
    /// API key for authentication
    #[arg(long, global = true, help_heading = "Authentication")]
    api_key: Option<String>,

    /// Base URL for the Apitally API
    #[arg(long, global = true, help_heading = "Authentication", hide = true)]
    api_base_url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate with the Apitally API
    Auth,

    /// List apps in your team
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `apps` and `app_envs` tables instead.
    Apps {
        /// Path to DuckDB database file for storing results
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// List consumers for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `consumers` table instead.
    Consumers {
        /// App ID
        app_id: i64,

        /// Filter to consumers that have made requests since this date/time (ISO 8601)
        #[arg(long)]
        requests_since: Option<String>,

        /// Path to DuckDB database file for storing results
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Retrieve request log data for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `request_logs` table instead.
    RequestLogs {
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
        /// Available fields: timestamp, request_uuid, app_env, method, path,
        /// url, consumer_id, request_headers, request_size, request_body_json,
        /// status_code, response_time_ms, response_headers, response_size,
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
        ///   string fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike
        ///   numeric fields: eq, neq, gt, gte, lt, lte, in, not_in
        ///   header fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike, exists, not_exists
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

        /// Path to DuckDB database file for storing results
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Run a SQL query against local DuckDB
    ///
    /// Available tables: apps, app_envs, consumers, request_logs.
    Sql {
        /// SQL query to execute (reads from stdin if omitted)
        query: Option<String>,

        /// Path to DuckDB database file
        #[arg(long)]
        db: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Auth => auth::run(
            cli.api_key,
            cli.api_base_url,
            &auth::auth_file_path()?,
            &mut std::io::stdin(),
        ),
        Command::Apps { db } => apps::run(
            db.as_deref(),
            cli.api_key.as_deref(),
            cli.api_base_url.as_deref(),
            std::io::stdout().lock(),
        ),
        Command::Consumers {
            app_id,
            requests_since,
            db,
        } => consumers::run(
            app_id,
            requests_since.as_deref(),
            db.as_deref(),
            cli.api_key.as_deref(),
            cli.api_base_url.as_deref(),
            std::io::stdout().lock(),
        ),
        Command::RequestLogs {
            app_id,
            since,
            until,
            fields,
            filters,
            limit,
            db,
        } => request_logs::run(
            app_id,
            &since,
            until.as_deref(),
            fields.as_deref(),
            filters.as_deref(),
            limit,
            db.as_deref(),
            cli.api_key.as_deref(),
            cli.api_base_url.as_deref(),
            std::io::stdout().lock(),
        ),
        Command::Sql { query, db } => {
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
