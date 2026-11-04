mod apps;
mod auth;
mod consumers;
mod endpoints;
mod metrics;
mod request_details;
mod request_logs;
mod reset_db;
mod sql;
mod traces;
mod utils;
mod whoami;

use std::io::Read;
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

        /// Filter to consumers that have made requests since this datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
        #[arg(long)]
        requests_since: Option<String>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// List endpoints for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `endpoints` table instead.
    Endpoints {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Filter to HTTP method(s), comma-separated
        #[arg(long)]
        method: Option<String>,

        /// Filter to path pattern, supports wildcards (*)
        #[arg(long)]
        path: Option<String>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Retrieve aggregated metrics for an app
    ///
    /// Outputs newline-delimited JSON (one object per line).
    /// With --db, inserts rows into the `metrics` table instead.
    Metrics {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Since datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
        #[arg(long)]
        since: String,

        /// Until datetime (ISO 8601 or relative duration, e.g. 24h, 7d; defaults to now)
        #[arg(long)]
        until: Option<String>,

        /// JSON array or comma-separated list of metric names to include
        ///
        /// Available metrics: requests, requests_per_minute, consumers, bytes_received, bytes_sent, client_errors, server_errors,
        /// error_rate, response_time_p50, response_time_p75, response_time_p90, response_time_p95, response_time_p99,
        /// apdex_score.
        #[arg(long)]
        metrics: String,

        /// Interval length as an ISO 8601 duration, to group by interval
        ///
        /// Available intervals: PT1M, PT2M, PT3M, PT5M, PT10M, PT15M, PT20M, PT30M,
        /// PT1H, PT2H, PT3H, PT6H, PT12H, P1D, P1W, P1M.
        /// When omitted, returns a single row per group for the entire time range.
        #[arg(long)]
        interval: Option<String>,

        /// JSON array or comma-separated list of field names to group by (in addition to time interval)
        ///
        /// Available fields: env, consumer_id, method, path, status_code.
        #[arg(long)]
        group_by: Option<String>,

        /// JSON array of filter objects with "field", "op" and "value" keys, combined with AND
        ///
        /// Fields and operators:
        ///   env, method, path: eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains
        ///   status_code: eq, neq, gt, gte, lt, lte, in, not_in
        ///   consumer_id: eq, neq, in, not_in, is_null, is_not_null
        ///
        /// in/not_in take an array and not_in also matches null. Omit "value" for
        /// is_null and is_not_null.
        ///
        /// Examples:
        ///   [{"field":"method","op":"eq","value":"GET"}]
        ///   [{"field":"status_code","op":"gte","value":400}]
        #[arg(long, verbatim_doc_comment)]
        filters: Option<String>,

        /// Timezone for intervals and to interpret since/until if not tz-aware
        ///
        /// Defaults to system timezone. Example: America/New_York.
        #[arg(long)]
        timezone: Option<String>,

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

        /// Since datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
        #[arg(long)]
        since: String,

        /// Until datetime (ISO 8601 or relative duration, e.g. 24h, 7d; defaults to now)
        #[arg(long)]
        until: Option<String>,

        /// JSON array or comma-separated list of field names to include
        ///
        /// Available fields: timestamp, request_uuid, env, method, path,
        /// url, consumer_id, request_headers, request_size_bytes, request_body_json,
        /// status_code, response_time_ms, response_headers, response_size_bytes,
        /// response_body_json, client_ip, client_country_iso_code, custom_attributes,
        /// exception_type, exception_message, exception_stacktrace, sentry_event_id,
        /// trace_id.
        ///
        /// Always included: timestamp, request_uuid, method, url, trace_id.
        ///
        /// Defaults to all fields except request_headers, request_body_json,
        /// response_headers, response_body_json, custom_attributes, exception_type,
        /// exception_message, exception_stacktrace, sentry_event_id.
        /// With --db, refetching replaces whole rows; omitted fields become null.
        ///
        /// custom_attributes is an object mapping attribute names to JSON values
        /// ({} when none are set).
        #[arg(long)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op" and "value" keys, combined with AND
        ///
        /// Operators by field type:
        ///   int (request_size_bytes, status_code, response_time_ms, response_size_bytes): eq, neq, gt, gte, lt, lte, in, not_in
        ///   datetime (timestamp): eq, neq, gt, gte, lt, lte
        ///   ID (request_uuid, consumer_id, trace_id, sentry_event_id): eq, neq, in, not_in
        ///   headers (request_headers, response_headers): string operators, exists, not_exists
        ///   custom_attributes: operators of the attribute value's type, exists, not_exists
        ///   string (all other fields): eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains
        ///
        /// String and ID fields except request_uuid, env, method and url also accept
        /// is_null and is_not_null. Headers and custom_attributes require "key" (the
        /// header name or the case-sensitive attribute name).
        ///
        /// in/not_in take an array and not_in also matches null. Omit "value" for
        /// exists, not_exists, is_null and is_not_null. like/ilike use % and _
        /// wildcards; contains is case-insensitive. Attribute comparisons only match
        /// when the key exists with a value of the same JSON type, including neq and
        /// not_in.
        ///
        /// Examples:
        ///   [{"field":"status_code","op":"gte","value":400}]
        ///   [{"field":"request_headers","key":"content-type","op":"eq","value":"application/json"}]
        ///   [{"field":"custom_attributes","key":"tenant.id","op":"eq","value":"acme"}]
        #[arg(long, verbatim_doc_comment)]
        filters: Option<String>,

        /// Approximate sample size (integer) or sample rate (float > 0 and <= 0.5)
        ///
        /// Pass an integer for size-based sampling (e.g. 1000 for ~1000 rows).
        /// Pass a float between 0 (exclusive) and 0.5 (inclusive) for rate-based
        /// sampling (e.g. 0.1 for ~10% of rows).
        #[arg(long)]
        sample: Option<String>,

        /// Maximum number of rows to return
        #[arg(long)]
        limit: Option<i64>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Retrieve trace spans for an app
    ///
    /// Outputs newline-delimited JSON (one span per line), ordered by start time,
    /// trace ID and span ID ascending. Filters select spans, not complete traces.
    /// Sampling, limits and time bounds can leave traces incomplete. Matching
    /// request logs need not exist. To expand a discovered trace, fetch its trace ID
    /// without discovery filters, sampling or unnecessary time bounds.
    ///
    /// With --db, replaces matching rows in the shared `spans` table. Omitted
    /// fields become null, including when request-details later writes a span.
    /// Join request_logs by app_id and trace_id.
    Traces {
        #[command(flatten)]
        api: ApiArgs,

        /// App ID
        app_id: i64,

        /// Since datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
        ///
        /// Inclusive span start-time bound. Required unless filters contain a
        /// nonempty trace_id eq/in lookup. Omitted lookup bounds stay unbounded.
        /// Datetimes without a timezone are UTC. Must precede --until if supplied.
        /// The API validates this requirement (input errors exit with code 4).
        #[arg(long)]
        since: Option<String>,

        /// Until datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
        ///
        /// Exclusive span start-time bound. Defaults to now for discovery, but
        /// remains unbounded for a trace_id eq/in lookup, even with --since.
        #[arg(long)]
        until: Option<String>,

        /// JSON array or comma-separated list of field names to include
        ///
        /// Replaces defaults. Always included: trace_id, span_id, start_time_ns.
        /// An empty JSON array selects only these required fields.
        ///
        /// Default fields:
        ///   trace_id: 32-character hex ID
        ///   span_id: 16-character hex ID
        ///   parent_span_id: 16-character hex ID, or null for no parent
        ///   env: environment name, or null
        ///   name: span operation name
        ///   kind: UNSPECIFIED, INTERNAL, SERVER, CLIENT, PRODUCER, CONSUMER
        ///   status: UNSET, OK, ERROR
        ///   start_time_ns: int64 Unix epoch nanoseconds
        ///   end_time_ns: int64 Unix epoch nanoseconds
        ///   duration_ns: int64 nanoseconds (1000000 ns = 1 ms)
        ///
        /// Opt-in fields:
        ///   attributes: object mapping attribute names to JSON values
        ///   events: array of {timestamp, name, attributes}; timestamps have nanosecond precision
        ///   scope_name: instrumentation scope name, or null
        ///   scope_version: instrumentation scope version, or null
        ///
        /// Selected empty attributes/events are {} and [], not null. Output IDs
        /// are lowercase hex. Database event timestamps represent UTC.
        #[arg(long, verbatim_doc_comment)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op" and "value" keys, combined with AND
        ///
        /// Operators by field type:
        ///   string (env, name, scope_name, scope_version): eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains
        ///   int (start_time_ns, end_time_ns, duration_ns): eq, neq, gt, gte, lt, lte, in, not_in
        ///   enum (kind, status), ID (trace_id, span_id, parent_span_id): eq, neq, in, not_in
        ///   attributes, events: operators of the attribute value's type, exists, not_exists
        ///
        /// parent_span_id, env, scope_name and scope_version also accept is_null and
        /// is_not_null. attributes and events require "key" (the case-sensitive
        /// attribute name).
        ///
        /// in/not_in take an array and not_in also matches null. Omit "value" for
        /// exists, not_exists, is_null and is_not_null. like/ilike use % and _
        /// wildcards; contains is case-insensitive. Attribute comparisons only match
        /// when the key exists with a value of the same JSON type, including neq and
        /// not_in. An events filter matches if any event of the span matches, and
        /// separate filters may match different events.
        ///
        /// Examples:
        ///   [{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]
        ///   [{"field":"duration_ns","op":"gte","value":100000000}]
        ///   [{"field":"attributes","key":"db.system","op":"eq","value":"postgresql"}]
        ///   [{"field":"events","key":"exception.type","op":"exists"}]
        #[arg(long, verbatim_doc_comment)]
        filters: Option<String>,

        /// Approximate sample size (integer) or sample rate (float > 0 and <= 0.5)
        ///
        /// Integer counts (e.g. 1000) and float rates (e.g. 0.1) sample individual
        /// spans, not whole traces. The limit still applies after sampling.
        #[arg(long)]
        sample: Option<String>,

        /// Maximum number of spans to return (1 to 1000000; default 1000000)
        ///
        /// There is no pagination or continuation token.
        #[arg(long)]
        limit: Option<i64>,

        /// Store results in DuckDB instead of outputting NDJSON
        ///
        /// Defaults to ~/.apitally/data.duckdb if no path is given. Both traces
        /// and request-details replace matching (app_id, trace_id, span_id) rows
        /// in spans, clearing omitted fields. Other rows remain. Legacy schemas
        /// require explicit reset/refetch or a new file; reset-db clears ALL tables.
        #[arg(long, num_args = 0..=1)]
        db: Option<Option<PathBuf>>,
    },

    /// Get details for a specific request
    ///
    /// Outputs a JSON object with full request details including headers, body,
    /// application logs, and spans.
    /// With --db, upserts the request into `request_logs`, inserts application
    /// logs, and replaces matching (app_id, trace_id, span_id) rows in `spans`.
    /// Span fields absent from this response (env, events, scope_name and
    /// scope_version) become null, including values fetched by traces earlier.
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
    /// Available tables: apps, app_envs, consumers, endpoints, metrics,
    /// request_logs, application_logs, spans.
    Sql {
        /// SQL query to execute (reads from stdin if omitted)
        query: Option<String>,

        /// Path to DuckDB database file
        ///
        /// Defaults to ~/.apitally/data.duckdb.
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Drop and recreate all tables in the local DuckDB database
    ResetDb {
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
        eprintln!("{} {err:#}", utils::ansi("1;31", "error:"));
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
        Command::Auth { api } => auth::run(api.api_key, api.api_base_url, &auth::auth_file_path()?),
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
        Command::Endpoints {
            api,
            app_id,
            method,
            path,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            endpoints::run(
                app_id,
                method.as_deref(),
                path.as_deref(),
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::Metrics {
            api,
            app_id,
            since,
            until,
            metrics,
            interval,
            group_by,
            filters,
            timezone,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            metrics::run(
                app_id,
                &since,
                until.as_deref(),
                &metrics,
                interval.as_deref(),
                group_by.as_deref(),
                filters.as_deref(),
                timezone.as_deref(),
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
            sample,
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
                sample.as_deref(),
                limit,
                db.as_deref(),
                api.api_key.as_deref(),
                api.api_base_url.as_deref(),
                std::io::stdout().lock(),
            )
        }
        Command::Traces {
            api,
            app_id,
            since,
            until,
            fields,
            filters,
            sample,
            limit,
            db,
        } => {
            let db = utils::resolve_db(db)?;
            traces::run(
                app_id,
                since.as_deref(),
                until.as_deref(),
                fields.as_deref(),
                filters.as_deref(),
                sample.as_deref(),
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
        Command::ResetDb { db } => {
            let db = db.map_or_else(utils::default_db_path, Ok)?;
            reset_db::run(&db)
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
        assert!(Cli::try_parse_from(["apitally", "endpoints"]).is_err()); // missing app_id
        assert!(Cli::try_parse_from(["apitally", "metrics", "42"]).is_err()); // missing --since and --metrics
        assert!(
            Cli::try_parse_from(["apitally", "metrics", "42", "--since", "2025-01-01"]).is_err()
        ); // missing --metrics
        assert!(Cli::try_parse_from(["apitally", "request-logs", "42"]).is_err()); // missing --since
        assert!(Cli::try_parse_from(["apitally", "request-details", "42"]).is_err()); // missing request_uuid
        assert!(Cli::try_parse_from(["apitally", "traces"]).is_err()); // missing app_id
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
            Cli::try_parse_from(["apitally", "endpoints", "42"])
                .unwrap()
                .command,
            Command::Endpoints { app_id: 42, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "apitally",
                "metrics",
                "42",
                "--since",
                "2025-01-01",
                "--metrics",
                r#"["requests","error_rate"]"#
            ])
            .unwrap()
            .command,
            Command::Metrics { app_id: 42, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "request-logs", "42", "--since", "2025-01-01"])
                .unwrap()
                .command,
            Command::RequestLogs { app_id: 42, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "traces", "42", "--since", "24h"])
                .unwrap()
                .command,
            Command::Traces {
                app_id: 42,
                since: Some(_),
                ..
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "traces", "42"])
                .unwrap()
                .command,
            Command::Traces {
                app_id: 42,
                since: None,
                until: None,
                ..
            }
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
        assert!(matches!(
            Cli::try_parse_from(["apitally", "reset-db"])
                .unwrap()
                .command,
            Command::ResetDb { db: None }
        ));
        assert!(matches!(
            Cli::try_parse_from(["apitally", "reset-db", "--db", "test.duckdb"])
                .unwrap()
                .command,
            Command::ResetDb { db: Some(_) }
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
