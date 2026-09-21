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

        /// URL of the Apitally dashboard (for browser-based auth)
        #[arg(
            long,
            env = "APITALLY_APP_URL",
            default_value = "https://app.apitally.io",
            hide = true
        )]
        app_url: String,
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
        /// Available metrics: requests, requests_per_minute, bytes_received, bytes_sent, client_errors, server_errors, error_rate,
        /// response_time_p50, response_time_p75, response_time_p90, response_time_p95, response_time_p99.
        #[arg(long)]
        metrics: String,

        /// Time interval for grouping
        ///
        /// Available intervals: month, day, hour, minute.
        /// When omitted, returns a single row per group for the entire time range.
        #[arg(long)]
        interval: Option<String>,

        /// JSON array or comma-separated list of field names to group by (in addition to time interval)
        ///
        /// Available fields: env, consumer_id, method, path, status_code.
        #[arg(long)]
        group_by: Option<String>,

        /// JSON array of filter objects with "field", "op", and "value" keys
        ///
        /// Supported fields: env, consumer_id, method, path, status_code.
        ///
        /// Supported operators:
        ///   string fields (env, method, path): eq, neq, in, not_in, like, not_like, contains, not_contains
        ///   numeric fields (consumer_id, status_code): eq, neq, gt, gte, lt, lte, in, not_in, is_null, is_not_null
        ///
        /// Examples:
        ///   [{"field":"method","op":"eq","value":"GET"}]
        ///   [{"field":"status_code","op":"gte","value":400}]
        #[arg(long)]
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
        /// response_body_json, client_ip, client_country_iso_code, exception_type,
        /// exception_message, exception_stacktrace, sentry_event_id, trace_id.
        ///
        /// Always included: timestamp, request_uuid, method, url, trace_id.
        ///
        /// Defaults to all fields except request_headers, request_body_json,
        /// response_headers, response_body_json, exception_type, exception_message,
        /// exception_stacktrace, sentry_event_id.
        /// With --db, refetching replaces whole rows; omitted fields become null.
        #[arg(long)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op", and "value" keys
        ///
        /// Supported operators:
        ///   string fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains, is_null, is_not_null
        ///   numeric fields: eq, neq, gt, gte, lt, lte, in, not_in
        ///   header fields: eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains, exists, not_exists
        ///   ID fields: eq, neq, in, not_in, is_null, is_not_null
        ///
        /// For "in" and "not_in", "value" must be a JSON array. For header fields,
        /// use "key" for the header name. For "exists", "not_exists", "is_null",
        /// and "is_not_null", omit "value".
        ///
        /// Examples:
        ///   [{"field":"status_code","op":"gte","value":400}]
        ///   [{"field":"path","op":"ilike","value":"/users/%"}]
        ///   [{"field":"request_headers","key":"content-type","op":"eq","value":"application/json"}]
        ///   [{"field":"response_body_json","op":"ilike","value":"%error%"}]
        #[arg(long)]
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
        ///   attributes: object mapping attribute names to JSON-encoded strings
        ///   events: array of {timestamp, name, attributes}; timestamps have nanosecond precision
        ///   scope_name: instrumentation scope name, or null
        ///   scope_version: instrumentation scope version, or null
        ///
        /// Event attributes use the same raw JSON-encoded string values as span
        /// attributes. Decode each value separately; filters use decoded scalars.
        /// Selected empty attributes/events are {} and [], not null. Output IDs
        /// are lowercase hex. Database event timestamps represent UTC.
        #[arg(long, verbatim_doc_comment)]
        fields: Option<String>,

        /// JSON array of filter objects with "field", "op", and "value" keys
        ///
        /// Clauses are ANDed. All fields are filterable without selecting them.
        /// Aliases: column/col for field, operator for op, val for value.
        /// Field/operator names are trimmed and lowercased; enum values,
        /// attribute keys and event names are case-sensitive. event_name is exact.
        ///
        /// Supported operators by field category:
        ///   strings (env, name, scope_name, scope_version): eq, neq, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains, is_null, is_not_null
        ///   integers (start_time_ns, end_time_ns, duration_ns): eq, neq, gt, gte, lt, lte, in, not_in
        ///   enums (kind, status): eq, neq, in, not_in
        ///   IDs (trace_id, span_id, parent_span_id): eq, neq, in, not_in, is_null, is_not_null
        ///   maps (attributes, events): eq, neq, gt, gte, lt, lte, in, not_in, like, not_like, ilike, not_ilike, contains, not_contains, exists, not_exists
        ///
        /// Null operators are not valid for name, trace_id or span_id. Trace IDs
        /// require 32 hex characters; span/parent IDs require 16. Hex case is
        /// normalized. Integer fields require JSON integers, not floats/strings.
        /// Enum values must be uppercase values listed under --fields.
        ///
        /// in/not_in require arrays; other comparisons require scalars. Scalar-field
        /// lists may be empty; JSON null is not a comparison value or list item. Omit
        /// value for exists, not_exists, is_null and is_not_null. like/ilike use
        /// SQL wildcards (% and _); contains/not_contains are case-insensitive
        /// substrings. not_in on nullable scalar fields also matches null rows.
        ///
        /// attributes requires "key". events requires "key", "event_name", or
        /// both; event_name alone permits only exists/not_exists. key is valid
        /// only for attributes/events; event_name only for events.
        ///
        /// Attribute value types restrict map operators: strings support string
        /// comparisons/patterns/membership (no null operators); numbers support
        /// equality/ordering/membership; booleans only eq/neq/in/not_in. Lists
        /// must be nonempty with one scalar type (integers and finite floats may
        /// mix). Compare decoded scalars, not JSON-encoded strings. Use existence
        /// checks for array/object attributes. Comparisons require a present key
        /// of the same JSON type; missing keys do not satisfy neq/not_in.
        ///
        /// Event clauses match any qualifying event; not_exists negates that
        /// existence. Event neq/not_in means any matching negative comparison,
        /// not that no event equals the value. Separate clauses may match
        /// different events. All events of selected spans are returned.
        ///
        /// Examples:
        ///   [{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]
        ///   [{"field":"env","op":"eq","value":"prod"},{"field":"duration_ns","op":"gte","value":100000000}]
        ///   [{"field":"kind","op":"in","value":["CLIENT","INTERNAL"]}]
        ///   [{"field":"status","op":"eq","value":"ERROR"}]
        ///   [{"field":"parent_span_id","op":"is_null"}]
        ///   [{"field":"name","op":"ilike","value":"%query%"}]
        ///   [{"field":"scope_name","op":"is_not_null"}]
        ///   [{"field":"attributes","key":"db.system","op":"eq","value":"postgresql"}]
        ///   [{"field":"attributes","key":"retry.count","op":"gte","value":2}]
        ///   [{"field":"attributes","key":"cached","op":"eq","value":true}]
        ///   [{"field":"attributes","key":"db.statement","op":"exists"}]
        ///   [{"field":"events","event_name":"exception","op":"exists"}]
        ///   [{"field":"events","event_name":"exception","key":"exception.type","op":"eq","value":"TimeoutError"}]
        ///   [{"field":"events","key":"code","op":"in","value":[500,503]}]
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
        Command::Auth { api, app_url } => {
            let input = if std::io::stdin().is_terminal() {
                Some(Box::new(std::io::stdin()) as Box<dyn std::io::Read + Send>)
            } else {
                None
            };
            auth::run(
                api.api_key,
                api.api_base_url,
                &app_url,
                &auth::auth_file_path()?,
                input,
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
    fn test_traces_cli_parsing() {
        for (db_args, expected_db) in [
            (vec![], None),
            (vec!["--db"], Some(None)),
            (
                vec!["--db", "traces.duckdb"],
                Some(Some(PathBuf::from("traces.duckdb"))),
            ),
        ] {
            let mut args = vec!["apitally", "traces", "42", "--since", "24h"];
            args.extend(db_args);
            let Command::Traces {
                app_id, since, db, ..
            } = Cli::try_parse_from(args).unwrap().command
            else {
                panic!("expected traces");
            };
            assert_eq!(app_id, 42);
            assert_eq!(since.as_deref(), Some("24h"));
            assert_eq!(db, expected_db);
        }

        let filters =
            r#"[{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]"#;
        let Command::Traces {
            since,
            until,
            fields,
            filters: parsed_filters,
            sample,
            limit,
            ..
        } = Cli::try_parse_from([
            "apitally",
            "traces",
            "42",
            "--filters",
            filters,
            "--fields",
            "attributes,events",
            "--sample",
            "0.1",
            "--limit",
            "100",
        ])
        .unwrap()
        .command
        else {
            panic!("expected traces");
        };
        assert!(since.is_none());
        assert!(until.is_none());
        assert_eq!(fields.as_deref(), Some("attributes,events"));
        assert_eq!(parsed_filters.as_deref(), Some(filters));
        assert_eq!(sample.as_deref(), Some("0.1"));
        assert_eq!(limit, Some(100));
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
