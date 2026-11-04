# Command Reference

All commands except `sql` and `reset-db` accept `--api-key <key>` for authentication. API key resolution order: `--api-key` flag > `APITALLY_API_KEY` env var > `~/.apitally/auth.json`.

Commands that accept a `--db` flag use `~/.apitally/data.duckdb` as the default database path if no other path is specified. If the database file doesn't exist, it will be created (except for the `sql` command). When writing to tables, existing records are updated (no duplicates are created).

Datetime flags (e.g. `--since`, `--until`, `--requests-since`) accept ISO 8601 strings, compact relative durations (e.g. `30m`, `24h`, `7d`, `2w`), or ISO 8601 durations with a single component (e.g. `P1M`, `P14D`, `PT30M`; a month is 30 days).

## Filters

`metrics`, `request-logs` and `traces` take `--filters` as a JSON array of `{"field", "op", "value"}` objects, combined with AND. Operators by field type:

- **string**: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`
- **int**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`
- **datetime**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte` (ISO 8601 value)
- **ID, enum**: `eq`, `neq`, `in`, `not_in`
- **nullable** fields also accept `is_null`, `is_not_null`
- **headers, attributes, events**: require `key`; operators follow the value type, plus `exists`, `not_exists`. Attribute comparisons only match when the key exists with a value of the same JSON type, including `neq` and `not_in`. An `events` filter matches if any event of the span matches, and separate filters may match different events.

`in`/`not_in` take an array, and `not_in` also matches null. Omit `value` for `exists`, `not_exists`, `is_null`, `is_not_null`. `like`/`ilike` use `%` and `_` wildcards; `contains` is case-insensitive.

## `auth`

```
npx @apitally/cli auth [--api-key <key>]
```

Opens a browser-based auth flow where the user logs in to the Apitally dashboard, confirms the code shown in the terminal, and picks a team and a scope (read-only or read & write). The CLI then fetches the newly created API key. The key is saved to `~/.apitally/auth.json` and used by all subsequent commands unless overridden by the `--api-key` flag.

If `--api-key` is provided, the key is saved directly without opening the browser.

## `whoami`

```
npx @apitally/cli whoami
```

Check authentication and show the team name. Outputs JSON to stdout. Exits with code 3 if not authenticated.

Example output:

<!-- prettier-ignore -->
```json
{"team":{"id":1,"name":"My Team"}}
```

## `apps`

```
npx @apitally/cli apps [--db [<path>]]
```

List all apps in the team. Use this to get app IDs for other commands. Outputs NDJSON to stdout by default.

- `--db`: Write to `apps` and `app_envs` tables in DuckDB instead of outputting NDJSON to stdout

Example NDJSON output (without `--db`):

```json
{"id":1,"name":"Example API 1","framework":"FastAPI","client_id":"76bf09e2-8996-4dd0-bdb5-ccdc3a48f64c","envs":[{"id":1,"name":"prod","created_at":"2026-01-01T00:00:00.000000Z","last_sync_at":"2026-01-01T01:00:00.000000Z"}],"created_at":"2026-01-01T00:00:00.000000Z"}
{"id":2,"name":"Example API 2","framework":"FastAPI","client_id":"339c08bb-5e88-4cba-a24d-be9d80fbd096","envs":[{"id":2,"name":"prod","created_at":"2026-01-02T00:00:00.000000Z","last_sync_at":"2026-01-02T01:00:00.000000Z"}],"created_at":"2026-01-02T00:00:00.000000Z"}
```

## `consumers`

```
npx @apitally/cli consumers <app-id> [--requests-since <datetime>] [--db [<path>]]
```

List all consumers for an app. Use this to map consumer IDs in request logs to identifiers and names. Outputs NDJSON to stdout by default.

- `--requests-since`: Only return consumers active since this datetime (ISO 8601 or relative duration, e.g. 24h, 7d)
- `--db`: Write to `consumers` table in DuckDB instead of outputting NDJSON to stdout

Example NDJSON output (without `--db`):

```json
{"id":1,"identifier":"bob@example.com","name":"Bob","group":{"id":1,"name":"Admins"},"attributes":{"plan":"pro"},"created_at":"2026-01-01T00:00:00Z","last_request_at":"2026-01-01T01:00:00Z"}
{"id":2,"identifier":"alice@example.com","name":"Alice","group":null,"attributes":{},"created_at":"2026-01-02T00:00:00Z","last_request_at":"2026-01-02T02:00:00Z"}
```

## `endpoints`

```
npx @apitally/cli endpoints <app-id> [--method <methods>] [--path <pattern>] [--db [<path>]]
```

List API endpoints for an app, ordered by path and method. Use this to see which endpoints exist for an app. Outputs NDJSON to stdout by default.

- `--method`: Filter to HTTP method(s), comma-separated (e.g. `GET,POST`)
- `--path`: Filter to path pattern, supports wildcards (e.g. `/v1/*`)
- `--db`: Write to `endpoints` table in DuckDB instead of outputting NDJSON to stdout

Example NDJSON output (without `--db`):

```json
{"id":1,"method":"POST","path":"/v1/users"}
{"id":2,"method":"GET","path":"/v1/users/{user_id}"}
```

## `metrics`

```
npx @apitally/cli metrics <app-id> --since <datetime> --metrics <json> \
  [--until <datetime>] [--interval <interval>] [--group-by <json>] \
  [--filters <json>] [--timezone <tz>] [--db [<path>]]
```

Fetch aggregated metrics for an app. Outputs NDJSON to stdout by default.

- `--since`: Start of time range, inclusive (ISO 8601 or relative duration, required)
- `--until`: End of time range, exclusive (ISO 8601 or relative duration, defaults to now)
- `--metrics`: Comma-separated list or JSON array of metric names to include (required)
- `--interval`: Interval length as an ISO 8601 duration (`PT1M`, `PT2M`, `PT3M`, `PT5M`, `PT10M`, `PT15M`, `PT20M`, `PT30M`, `PT1H`, `PT2H`, `PT3H`, `PT6H`, `PT12H`, `P1D`, `P1W`, `P1M`). When omitted, returns a single row per group for the entire time range
- `--group-by`: Comma-separated list or JSON array of field names to group by, in addition to time period
- `--filters`: JSON array of filter objects (see [Filters](#filters))
- `--timezone`: Timezone for intervals and to interpret since/until if not tz-aware (defaults to system timezone)
- `--db`: Write to `metrics` table in DuckDB instead of outputting NDJSON to stdout

**Deduplication in DuckDB:** Deletes all existing rows for the same `app_id` within the fetched time range before inserting new data.

### Available metrics

| Metric                | Type    | Description                                                                                 |
| --------------------- | ------- | ------------------------------------------------------------------------------------------- |
| `requests`            | integer | Total request count                                                                         |
| `requests_per_minute` | float   | Requests per minute                                                                         |
| `consumers`           | integer | Number of distinct consumers                                                                |
| `bytes_received`      | integer | Total bytes received                                                                        |
| `bytes_sent`          | integer | Total bytes sent                                                                            |
| `client_errors`       | integer | 4xx errors (excluding expected errors)                                                      |
| `server_errors`       | integer | 5xx errors (excluding expected errors)                                                      |
| `error_rate`          | float   | Ratio of errors to total requests                                                           |
| `response_time_p50`   | integer | 50th percentile response time (ms)                                                          |
| `response_time_p75`   | integer | 75th percentile response time (ms)                                                          |
| `response_time_p90`   | integer | 90th percentile response time (ms)                                                          |
| `response_time_p95`   | integer | 95th percentile response time (ms)                                                          |
| `response_time_p99`   | integer | 99th percentile response time (ms)                                                          |
| `apdex_score`         | float   | Apdex score from 0 to 1, based on each endpoint's target response time set in the dashboard |

### Group-by fields

`env`, `consumer_id`, `method`, `path`, `status_code`

### Filters

Filter fields: `env`, `method`, `path` (string), `status_code` (int), `consumer_id` (ID, nullable). See [Filters](#filters) for operators.

```json
[{"field":"method","op":"eq","value":"GET"}]
[{"field":"path","op":"like","value":"/v1/users/%"}]
[{"field":"status_code","op":"gte","value":400}]
```

Example NDJSON output (without `--db`):

```json
{"period_start":"2026-01-01T00:00:00Z","period_end":"2026-01-01T01:00:00Z","env":"prod","requests":1234,"error_rate":0.02}
{"period_start":"2026-01-01T01:00:00Z","period_end":"2026-01-01T02:00:00Z","env":"prod","requests":987,"error_rate":0.01}
```

## `request-logs`

```
npx @apitally/cli request-logs <app-id> --since <datetime> \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--sample <n|rate>] [--limit <n>] [--db [<path>]]
```

Fetch request log data for an app. Outputs NDJSON to stdout by default.

- `--since`: Start of time range, inclusive (ISO 8601 or relative duration, required)
- `--until`: End of time range, exclusive (ISO 8601 or relative duration, defaults to now)
- `--fields`: Comma-separated list or JSON array of field names to include
- `--filters`: JSON array of filter objects
- `--sample`: Approximate sample size (integer, e.g. `1000`) or sample rate (float > 0 and <= 0.5, e.g. `0.1` for ~10%)
- `--limit`: Maximum number of rows (hard cap: 1,000,000)
- `--db`: Write to `request_logs` table in DuckDB instead of outputting NDJSON to stdout

Timestamps without timezone are treated as UTC. Results are ordered by timestamp ascending.

### Fields

| Field                     | Type                             | Default |
| ------------------------- | -------------------------------- | ------- |
| `timestamp`               | string (datetime)                | yes     |
| `request_uuid`            | string (ID)                      | yes     |
| `trace_id`                | string (ID), nullable            | yes     |
| `env`                     | string                           | yes     |
| `method`                  | string                           | yes     |
| `path`                    | string, nullable                 | yes     |
| `url`                     | string                           | yes     |
| `consumer_id`             | int (ID), nullable               | yes     |
| `request_headers`         | array of string tuples (headers) | no      |
| `request_size_bytes`      | int                              | yes     |
| `request_body_json`       | string (JSON), nullable          | no      |
| `status_code`             | int                              | yes     |
| `response_time_ms`        | int                              | yes     |
| `response_headers`        | array of string tuples (headers) | no      |
| `response_size_bytes`     | int                              | yes     |
| `response_body_json`      | string (JSON), nullable          | no      |
| `client_ip`               | string, nullable                 | yes     |
| `client_country_iso_code` | string, nullable                 | yes     |
| `custom_attributes`       | object                           | no      |
| `exception_type`          | string, nullable                 | no      |
| `exception_message`       | string, nullable                 | no      |
| `exception_stacktrace`    | string, nullable                 | no      |
| `sentry_event_id`         | string (ID), nullable            | no      |

Default fields are included when `--fields` is omitted. Providing `--fields` replaces the defaults, but `timestamp`, `request_uuid`, `trace_id`, `method`, and `url` are always included. With `--db`, refetching replaces complete matching rows and sets omitted columns to `NULL`.

### Filters

See [Filters](#filters). `key` is the header name for `request_headers` and `response_headers`, and the case-sensitive attribute name for `custom_attributes`.

```json
[{"field":"consumer_id","op":"is_null"}]
[{"field":"url","op":"ilike","value":"%/users/123%"}]
[{"field":"status_code","op":"gte","value":400}]
[{"field":"request_headers","key":"x-api-version","op":"exists"}]
[{"field":"request_headers","key":"content-type","op":"eq","value":"application/json"}]
[{"field":"response_body_json","op":"contains","value":"error"}]
[{"field":"custom_attributes","key":"tenant.id","op":"eq","value":"acme"}]
```

Example NDJSON output (without `--db`):

```json
{"timestamp":"2026-01-01T00:15:00.000Z","request_uuid":"2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf","env":"prod","method":"GET","path":"/test/1","url":"https://api.example.com/test/1","consumer_id":1,"request_size_bytes":0,"status_code":404,"response_time_ms":122,"response_size_bytes":66,"client_ip":"203.0.113.10","client_country_iso_code":"DE"}
{"timestamp":"2026-01-01T00:16:00.000Z","request_uuid":"c6d32f8a-0bc1-43c1-b6c5-7d04363dc97c","env":"prod","method":"GET","path":"/test/2","url":"https://api.example.com/test/2","consumer_id":1,"request_size_bytes":0,"status_code":500,"response_time_ms":68,"response_size_bytes":66,"client_ip":"198.51.100.22","client_country_iso_code":"US"}
```

## `request-details`

```
npx @apitally/cli request-details <app-id> <request-uuid> [--db [<path>]]
```

Get full details for a specific request identified by its UUID, including headers, request/response body, exception info, application logs, and spans. Outputs a JSON object to stdout by default.

- `--db`: Write to `request_logs`, `application_logs`, and `spans` tables in DuckDB instead of outputting JSON to stdout

Correlate spans to requests on both `app_id` and `trace_id`. Multiple requests can share a trace and each trace can have many spans, so joins can multiply counts. See [relationships](duckdb_tables.md#relationships).

To store full details for a request found in request logs (substitute its app ID and UUID):

```bash
npx @apitally/cli request-details 1 2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf \
  --db ./trace-investigation.duckdb
```

Example JSON output (without `--db`):

<!-- prettier-ignore -->
```json
{"timestamp":"2026-01-01T00:15:00.000Z","request_uuid":"2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf","env":"prod","method":"GET","path":"/test/1","url":"https://api.example.com/test/1","consumer_id":1,"request_headers":[["content-type","application/json"]],"request_size_bytes":0,"request_body_json":null,"status_code":200,"response_time_ms":122,"response_headers":[["x-request-id","abc"]],"response_size_bytes":66,"response_body_json":"{\"ok\":true}","client_ip":"203.0.113.10","client_country_iso_code":"DE","custom_attributes":{"tenant.id":"acme"},"trace_id":"0123456789abcdef0123456789abcdef","exception":null,"logs":[{"timestamp":"2026-01-01T00:15:00.100Z","message":"handling request","level":"INFO","logger":"app","file":"main.py","line":42}],"spans":[{"span_id":"0123456789abcdef","parent_span_id":null,"name":"GET /test/1","kind":"SERVER","start_time_ns":1767226500000000000,"end_time_ns":1767226500050000000,"duration_ns":50000000,"status":"OK","attributes":{"http.method":"GET"}}]}
```

## `traces`

```
npx @apitally/cli traces <app-id> [--since <datetime>] \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--sample <n|rate>] [--limit <n>] [--db [<path>]]
```

Fetch trace spans for an app. Outputs NDJSON to stdout by default, with one span per row.

- `--since`: Span start time, inclusive (ISO 8601 or relative duration); required unless a nonempty `trace_id` filter uses `eq` or `in`
- `--until`: Span start time, exclusive; defaults to now
- `--fields`: Comma-separated list or JSON array of field names to include
- `--filters`: JSON array of filter objects
- `--sample`: Approximate sample size (positive integer, e.g. `1000`) or sample rate (float > 0 and <= 0.5, e.g. `0.1`)
- `--limit`: Maximum number of spans, from 1 to 1,000,000 (default: 1,000,000)
- `--db`: Write to `spans` table in DuckDB instead of outputting NDJSON to stdout

Timestamps without timezone are treated as UTC. Results are ordered by start time.

Filters and sampling select individual spans, not whole traces. To fetch related spans, query their trace IDs without other filters, sampling, or time bounds; the limit still applies.

### Fields

| Field            | Type                  | Default | Notes                                                                 |
| ---------------- | --------------------- | ------- | --------------------------------------------------------------------- |
| `trace_id`       | string (ID)           | yes     | 32-character lowercase hex                                            |
| `span_id`        | string (ID)           | yes     | 16-character lowercase hex                                            |
| `parent_span_id` | string (ID), nullable | yes     | 16-character lowercase hex; null for no parent                        |
| `env`            | string, nullable      | yes     | Environment name                                                      |
| `name`           | string                | yes     | Span operation name                                                   |
| `kind`           | enum string           | yes     | `UNSPECIFIED`, `INTERNAL`, `SERVER`, `CLIENT`, `PRODUCER`, `CONSUMER` |
| `status`         | enum string           | yes     | `UNSET`, `OK`, `ERROR`                                                |
| `start_time_ns`  | int64                 | yes     | Unix epoch nanoseconds                                                |
| `end_time_ns`    | int64                 | yes     | Unix epoch nanoseconds                                                |
| `duration_ns`    | int64                 | yes     | Nanoseconds                                                           |
| `attributes`     | object                | no      | Attribute names mapped to JSON values                                 |
| `events`         | array of objects      | no      | Each event object has `timestamp`, `name`, and `attributes` keys      |
| `scope_name`     | string, nullable      | no      | Instrumentation scope                                                 |
| `scope_version`  | string, nullable      | no      | Instrumentation scope version                                         |

Default fields are included when `--fields` is omitted. Providing `--fields` replaces the defaults, but `trace_id`, `span_id`, and `start_time_ns` are always included.

Event timestamps are ISO 8601 UTC strings with nanosecond precision. See [JSON extraction examples](duckdb_json_functions.md#examples) for querying attributes and events.

### Filters

See [Filters](#filters). `key` is the case-sensitive attribute name for `attributes` and `events`.

```json
[{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]
[{"field":"kind","op":"in","value":["CLIENT","INTERNAL"]}]
[{"field":"name","op":"ilike","value":"%query%"}]
[{"field":"env","op":"eq","value":"prod"},{"field":"duration_ns","op":"gte","value":100000000}]
[{"field":"attributes","key":"db.system","op":"eq","value":"postgresql"}]
[{"field":"events","key":"exception.type","op":"exists"}]
```

Example NDJSON output (without `--db`):

```json
{"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0123456789abcdef","start_time_ns":1767225600000000000,"parent_span_id":null,"env":"prod","name":"GET /users","kind":"SERVER","status":"OK","end_time_ns":1767225600250000000,"duration_ns":250000000}
```

## `sql`

```
npx @apitally/cli sql "<query>" [--db <path>]
npx @apitally/cli sql [--db <path>] < query.sql
echo "<query>" | npx @apitally/cli sql [--db <path>]
```

Run a SQL query against a local DuckDB database. The query can be passed as an argument or read from stdin. Outputs NDJSON to stdout.

- `--db`: Path to DuckDB database

Available tables: `apps`, `app_envs`, `consumers`, `endpoints`, `metrics`, `request_logs`, `application_logs`, `spans`. See [duckdb_tables.md](duckdb_tables.md) for schemas.

**Important:** The database may contain data from previous sessions. Always filter queries by `app_id` and the current investigation scope: `timestamp` for request logs, `period_start`/`period_end` for metrics, and exact `trace_id` values or integer `start_time_ns` bounds for spans.

DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview). `TIMESTAMPTZ` values are output, truncated, and cast to `DATE` in the system time zone. Use `timestamp AT TIME ZONE 'UTC'` for UTC.

Example output:

```json
{"timestamp":"2026-01-01T00:16:00.000Z","method":"POST","path":"/users","status_code":500}
{"timestamp":"2026-01-01T00:15:00.000Z","method":"GET","path":"/users/{userId}","status_code":404}
```

## `reset-db`

```
npx @apitally/cli reset-db [--db <path>]
```

Drop and recreate all tables in the local DuckDB database. Use this to clear all stored data and start fresh.

- `--db`: Path to DuckDB database
