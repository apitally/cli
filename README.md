# Apitally CLI

[![Tests](https://github.com/apitally/cli/actions/workflows/tests.yaml/badge.svg?event=push)](https://github.com/apitally/cli/actions)
[![Codecov](https://codecov.io/gh/apitally/cli/graph/badge.svg?token=O3VWKH6DH9)](https://codecov.io/gh/apitally/cli)
[![Release](https://img.shields.io/github/v/release/apitally/cli?color=informational)](https://github.com/apitally/cli/releases/latest)
[![npm](https://img.shields.io/npm/v/@apitally/cli?logo=npm&color=%23cb0000)](https://www.npmjs.com/package/@apitally/cli)

A command-line interface for Apitally, built for humans and agents.

Apitally is a simple API monitoring and analytics tool that makes it easy to understand API usage, monitor performance, and troubleshoot issues.

Learn more about Apitally on our 🌎 [website](https://apitally.io) or check out
the 📚 [documentation](https://docs.apitally.io).

## Highlights

- Stream API request logs from Apitally to a local [DuckDB](https://github.com/duckdb/duckdb) database
- Run arbitrary SQL queries against that database to analyze API request data
- Bundled DuckDB, no runtime dependencies, written in Rust (it's fast)

## Installation

The CLI can be used with `npx`, no installation required:

```shell
npx @apitally/cli <command>
```

If you wish to install the binary directly, use the standalone installer script:

```shell
# On macOS and Linux
curl -fsSL https://apitally.io/cli/install.sh | sh
```

```shell
# On Windows
powershell -ExecutionPolicy Bypass -c "irm https://apitally.io/cli/install.ps1 | iex"
```

You can also download the binary for your platform from the [latest release](https://github.com/apitally/cli/releases/latest) on GitHub.

## Authentication

To use the CLI, you need an API key. You can create one in the [Apitally dashboard](https://app.apitally.io/settings/api-keys) under _Settings → API keys_.

Then run the `auth` command to configure your API key interactively:

```bash
apitally auth
```

Or provide the key directly:

```bash
apitally auth --api-key "your-api-key"
```

The API key is saved to `~/.apitally/auth.json`.

You can also set the API key via the `APITALLY_API_KEY` environment variable or pass the `--api-key` flag to any command.

## Commands

Run `apitally --help` to see all commands and options.

### `whoami`

```
apitally whoami
```

Check authentication and show the authenticated team.

Example command:

```shell
apitally whoami
```

Example output:

<!-- prettier-ignore -->
```json
{"team":{"id":1,"name":"My Team"}}
```

### `apps`

```
apitally apps [--db [<path>]]
```

List all apps in your team. Use this to get app IDs for other commands.

Outputs newline-delimited JSON (one object per line). With `--db [<path>]`, data is written to the `apps` table in a DuckDB database instead. Defaults to `~/.apitally/data.duckdb` if no path is given. Existing records will be updated. If the database file doesn't exist, it will be created.

Example command:

```shell
apitally apps
```

Example output (without `--db` flag):

```json
{"id":1,"name":"Example API 1","framework":"FastAPI","client_id":"76bf09e2-8996-4dd0-bdb5-ccdc3a48f64c","envs":[{"id":1,"name":"prod","created_at":"2026-01-01T00:00:00.000000Z","last_sync_at":"2026-01-01T01:00:00.000000Z"}],"created_at":"2026-01-01T00:00:00.000000Z"}
{"id":2,"name":"Example API 2","framework":"FastAPI","client_id":"339c08bb-5e88-4cba-a24d-be9d80fbd096","envs":[{"id":2,"name":"prod","created_at":"2026-01-02T00:00:00.000000Z","last_sync_at":"2026-01-02T01:00:00.000000Z"}],"created_at":"2026-01-02T00:00:00.000000Z"}
```

### `consumers`

```
apitally consumers <app-id> [--requests-since <datetime>] [--db [<path>]]
```

List all consumers for an app. Use this to get consumer details to combine with request log data, which only includes consumer IDs.

Use the `--requests-since` flag to only return consumers that have made requests since a specific date/time (ISO 8601 format).

Outputs newline-delimited JSON (one object per line). With `--db [<path>]`, data is written to the `consumers` table in a DuckDB database instead. Defaults to `~/.apitally/data.duckdb` if no path is given. Existing records will be updated. If the database file doesn't exist, it will be created.

Example command:

```shell
apitally consumers 1 --requests-since "2026-01-01T00:00:00Z"
```

Example output (without `--db` flag):

```json
{"id":1,"identifier":"bob@example.com","name":"Bob","group":{"id":1,"name":"Admins"},"created_at":"2026-01-01T00:00:00Z","last_request_at":"2026-01-01T01:00:00Z"}
{"id":2,"identifier":"alice@example.com","name":"Alice","group":null,"created_at":"2026-01-02T00:00:00Z","last_request_at":"2026-01-02T02:00:00Z"}
```

### `request-logs`

```
apitally request-logs <app-id> \
  --since <datetime> [--until <datetime>] \
  [--fields <json>] [--filters <json>] [--limit <n>] \
  [--db [<path>]]
```

Retrieve request log data for an app.

The time range is `--since` inclusive and `--until` exclusive. If `--until` is not provided, it defaults to now. If a timestamp does not include a timezone, UTC is assumed.

Outputs newline-delimited JSON (one object per line). With `--db`, data is written to the `request_logs` table in a DuckDB database instead. Defaults to `~/.apitally/data.duckdb` if no path is given. Existing records will be updated. If the database file doesn't exist, it will be created.

Results are ordered by `timestamp` ascending and capped at 1,000,000 records. Requests to endpoints marked as excluded in the Apitally dashboard are not returned.

Use the `--fields` flag to pass a JSON array of fields to include. If omitted, default fields are returned.

| Field                     | Type              | Default |
| ------------------------- | ----------------- | ------- |
| `timestamp`               | string (datetime) | ✅      |
| `request_uuid`            | string (ID)       | ✅      |
| `app_env`                 | string            | ✅      |
| `method`                  | string            | ✅      |
| `path`                    | string            | ✅      |
| `url`                     | string            | ✅      |
| `consumer_id`             | int (ID)          | ✅      |
| `request_headers`         | array of tuples   |         |
| `request_size`            | int               | ✅      |
| `request_body_json`       | string (JSON)     |         |
| `status_code`             | int               | ✅      |
| `response_time_ms`        | int               | ✅      |
| `response_headers`        | array of tuples   |         |
| `response_size`           | int               | ✅      |
| `response_body_json`      | string (JSON)     |         |
| `client_ip`               | string            | ✅      |
| `client_country_iso_code` | string            | ✅      |
| `exception_type`          | string            |         |
| `exception_message`       | string            |         |
| `exception_stacktrace`    | string            |         |
| `sentry_event_id`         | string (ID)       |         |
| `trace_id`                | string (ID)       |         |

Use the `--filters` flag to pass a JSON array of filter objects with `field`, `op`, and `value` keys. Multiple filters are combined with a logical `AND`. Supported operators are:

- String fields: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`
- Numeric fields: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`
- Header fields: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `exists`, `not_exists`
- ID fields: `eq`, `neq`, `in`, `not_in`

For `in` and `not_in`, `value` must be a JSON array. For header fields, also provide `key` for the header name. For `exists` and `not_exists`, omit `value`.

Example command:

```shell
apitally request-logs 1 \
  --since "2026-01-01T00:00:00Z" \
  --filters '[{"field":"status_code","op":"gte","value":400}]' \
  --limit 2
```

Example output (without `--db` flag):

```json
{"timestamp":"2026-01-01T00:15:00.000Z","request_uuid":"2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf","app_env":"prod","method":"GET","path":"/test/1","url":"https://api.example.com/test/1","consumer_id":1,"request_size":0,"status_code":404,"response_time_ms":122,"response_size":66,"client_ip":"203.0.113.10","client_country_iso_code":"DE"}
{"timestamp":"2026-01-01T00:16:00.000Z","request_uuid":"c6d32f8a-0bc1-43c1-b6c5-7d04363dc97c","app_env":"prod","method":"GET","path":"/test/2","url":"https://api.example.com/test/2","consumer_id":1,"request_size":0,"status_code":500,"response_time_ms":68,"response_size":66,"client_ip":"198.51.100.22","client_country_iso_code":"US"}
```

### `sql`

```shell
apitally sql [<query>] [--db [<path>]]
```

Run a SQL query against a local DuckDB database and output the result as newline-delimited JSON (one object per line). If the query argument is omitted, the query is read from stdin. Defaults to `~/.apitally/data.duckdb` if `--db` is omitted or given without a path.

Available tables are `apps`, `app_envs`, `consumers`, and `request_logs`.

DuckDB's [SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview) closely matches PostgreSQL's semantics.

Example commands:

```shell
apitally sql "SELECT timestamp, method, path, status_code FROM request_logs WHERE status_code >= 400"
```

```shell
echo "SELECT COUNT(*) FROM request_logs" | apitally sql
```

Example output:

```json
{"timestamp":"2026-01-01T00:16:00.000Z","method":"POST","path":"/users","status_code":500}
{"timestamp":"2026-01-01T00:15:00.000Z","method":"GET","path":"/users/{userId}","status_code":404}
```

## Exit codes

| Code | Meaning                                                 |
| ---- | ------------------------------------------------------- |
| 0    | Success                                                 |
| 1    | General / unknown error                                 |
| 2    | Usage error (invalid arguments, missing required flags) |
| 3    | Authentication error (missing or invalid API key)       |
| 4    | Input error (invalid argument values)                   |
| 5    | API / network error                                     |

## Getting help

If you need help please
[create a new discussion](https://github.com/orgs/apitally/discussions/categories/q-a)
on GitHub or email us at [support@apitally.io](mailto:support@apitally.io). We'll get back to you as soon as possible.
