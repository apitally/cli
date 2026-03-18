mod apps;
mod auth;
mod consumers;
mod request_logs;
mod sql;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "apitally", about = "Apitally CLI")]
struct Cli {
    /// API key for authentication
    #[arg(long, global = true, help_heading = "Authentication")]
    api_key: Option<String>,

    /// Base URL for the Apitally API
    #[arg(long, global = true, help_heading = "Authentication")]
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
        db: Option<String>,
    },

    /// List consumers for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `consumers` table instead.
    Consumers {
        /// App ID
        #[arg(long)]
        app_id: i64,

        /// Filter to consumers that have made requests since this date/time (ISO 8601)
        #[arg(long)]
        requests_since: Option<String>,

        /// Path to DuckDB database file for storing results
        #[arg(long)]
        db: Option<String>,
    },

    /// Retrieve request log data for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `request_logs` table instead.
    RequestLogs {
        /// App ID
        #[arg(long)]
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
        /// url, consumer_id, request_headers, request_size, request_body,
        /// status_code, response_time, response_headers, response_size,
        /// response_body, client_ip, client_country_name,
        /// client_country_iso_code, exception_type, exception_message,
        /// exception_stacktrace, sentry_event_id, trace_id.
        ///
        /// Always included: timestamp, request_uuid, method, url.
        ///
        /// Defaults to all fields except request_headers, request_body,
        /// response_headers, response_body, exception_type,
        /// exception_message, exception_stacktrace.
        #[arg(long)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op", and "value" keys
        ///
        /// Operators by field type:
        ///   string fields: eq, neq, in, not_in, like, not_like, ilike,
        ///     not_ilike
        ///   int fields: eq, neq, gt, gte, lt, lte, in, not_in
        ///   float/datetime fields: eq, neq, gt, gte, lt, lte
        ///   header fields (request_headers, response_headers): eq, neq, in,
        ///     not_in, like, not_like, ilike, not_ilike, exists, not_exists
        ///
        /// For header fields, use "key" to specify the header name.
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
        db: Option<String>,
    },

    /// Run a SQL query against local DuckDB
    ///
    /// Available tables: apps, app_envs, consumers, request_logs.
    Sql {
        /// SQL query to execute
        query: String,

        /// Path to DuckDB database file
        #[arg(long)]
        db: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Auth => auth::run(cli.api_key, cli.api_base_url),
        Command::Apps { db } => apps::run(
            db.as_deref(),
            cli.api_key.as_deref(),
            cli.api_base_url.as_deref(),
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
        ),
        Command::Sql { query, db } => sql::run(&query, &db),
    }
}
