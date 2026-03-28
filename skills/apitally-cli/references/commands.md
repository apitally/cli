# Command Reference

All commands accept an `--api-key <key>` flag for authentication (except `sql`). API key resolution order: `--api-key` flag > `APITALLY_API_KEY` env var > `~/.apitally/auth.json`.

Commands that accept a `--db` flag use `~/.apitally/data.duckdb` as the default database path if no other path is specified. If the database file doesn't exist, it will be created (except for the `sql` command). When writing to tables, existing records are updated (no duplicates are created).

## `auth`

```
npx @apitally/cli auth [--api-key <key>]
```

Configure API key interactively or by providing a key directly. Saves API key to `~/.apitally/auth.json`.

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

- `--requests-since`: Only return consumers active since this datetime (ISO 8601)
- `--db`: Write to `consumers` table in DuckDB instead of outputting NDJSON to stdout

Example NDJSON output (without `--db`):

```json
{"id":1,"identifier":"bob@example.com","name":"Bob","group":{"id":1,"name":"Admins"},"created_at":"2026-01-01T00:00:00Z","last_request_at":"2026-01-01T01:00:00Z"}
{"id":2,"identifier":"alice@example.com","name":"Alice","group":null,"created_at":"2026-01-02T00:00:00Z","last_request_at":"2026-01-02T02:00:00Z"}
```

## `request-logs`

```
npx @apitally/cli request-logs <app-id> --since <datetime> \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--limit <n>] [--db [<path>]]
```

Fetch request log data for an app. Outputs NDJSON to stdout by default.

- `--since`: Start of time range, inclusive (ISO 8601, required)
- `--until`: End of time range, exclusive (ISO 8601, defaults to now)
- `--fields`: JSON array of field names to include
- `--filters`: JSON array of filter objects
- `--limit`: Maximum number of rows (hard cap: 1,000,000)
- `--db`: Write to `request_logs` table in DuckDB instead of outputting NDJSON to stdout

Timestamps without timezone are treated as UTC. Results are ordered by timestamp ascending.

### Fields

| Field                     | Type                             | Default |
| ------------------------- | -------------------------------- | ------- |
| `timestamp`               | string (datetime)                | yes     |
| `request_uuid`            | string (ID)                      | yes     |
| `env`                     | string                           | yes     |
| `method`                  | string                           | yes     |
| `path`                    | string                           | yes     |
| `url`                     | string                           | yes     |
| `consumer_id`             | int (ID)                         | yes     |
| `request_headers`         | array of string tuples (headers) | no      |
| `request_size_bytes`      | int                              | yes     |
| `request_body_json`       | string (JSON)                    | no      |
| `status_code`             | int                              | yes     |
| `response_time_ms`        | int                              | yes     |
| `response_headers`        | array of string tuples (headers) | no      |
| `response_size_bytes`     | int                              | yes     |
| `response_body_json`      | string (JSON)                    | no      |
| `client_ip`               | string                           | yes     |
| `client_country_iso_code` | string                           | yes     |
| `exception_type`          | string                           | no      |
| `exception_message`       | string                           | no      |
| `exception_stacktrace`    | string                           | no      |
| `sentry_event_id`         | string (ID)                      | no      |
| `trace_id`                | string (ID)                      | no      |

Default fields are included when `--fields` is omitted. When `--fields` is provided, it replaces the default set and only the specified fields are returned. `timestamp`, `request_uuid`, `method`, and `url` are always included regardless.

### Filters

Pass `--filters` as a JSON array of filter objects. Multiple filters are combined with AND.

Filter object keys:

- `field`: field name to filter on
- `op`: comparison operator
- `value`: comparison value (omit for `exists`/`not_exists`)
- `key`: header name, required only for `request_headers` and `response_headers`

#### Operators by field type

All fields can be used in filters. Available operators depend on the field type:

- **string / string (JSON)**: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`, `is_null`, `is_not_null`
- **string (datetime)**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte` — value is an ISO 8601 datetime string
- **string (ID) / int (ID)**: `eq`, `neq`, `in`, `not_in`, `is_null`, `is_not_null`
- **array of string tuples (headers)**: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`, `exists`, `not_exists` — requires `key`
- **int**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`

#### Value rules

- `in`/`not_in`: value must be a JSON array (of strings or ints matching the field type)
- `exists`/`not_exists`/`is_null`/`is_not_null`: omit value entirely
- `like`/`ilike`/`not_like`/`not_ilike`: use `%` as wildcard
- `contains`/`not_contains`: case-insensitive substring match (no wildcards needed)

#### Filter examples

```json
[{"field": "consumer_id", "op": "eq", "value": 42}]
[{"field": "consumer_id", "op": "is_null"}]
[{"field": "path", "op": "eq", "value": "/v1/users/{user_id}"}]
[{"field": "url", "op": "ilike", "value": "%/users/123%"}]
[{"field": "status_code", "op": "gte", "value": 400},{"field": "status_code", "op": "lt", "value": 500}]
[{"field": "request_headers", "key": "x-api-version", "op": "exists"}]
[{"field": "request_headers", "key": "content-type", "op": "eq", "value": "application/json"}]
[{"field": "response_body_json", "op": "contains", "value": "error"}]
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

Example JSON output (without `--db`):

<!-- prettier-ignore -->
```json
{"timestamp":"2026-01-01T00:15:00.000Z","request_uuid":"2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf","env":"prod","method":"GET","path":"/test/1","url":"https://api.example.com/test/1","consumer":"bob@example.com","request_headers":[["content-type","application/json"]],"request_size_bytes":0,"request_body_json":null,"status_code":200,"response_time_ms":122,"response_headers":[["x-request-id","abc"]],"response_size_bytes":66,"response_body_json":"{\"ok\":true}","client_ip":"203.0.113.10","client_country_iso_code":"DE","trace_id":"0000000000000000aaaaaaaaaaaaaaaa","exception":null,"logs":[{"timestamp":"2026-01-01T00:15:00.100Z","message":"handling request","level":"INFO","logger":"app","file":"main.py","line":42}],"spans":[{"span_id":"00000000000000aa","parent_span_id":null,"name":"GET /test/1","kind":"SERVER","start_time_ns":1735689600000000000,"end_time_ns":1735689600050000000,"duration_ns":50000000,"status":"OK","attributes":{"http.method":"GET"}}]}
```

## `sql`

```
npx @apitally/cli sql "<query>" [--db <path>]
npx @apitally/cli sql [--db <path>] < query.sql
echo "<query>" | npx @apitally/cli sql [--db <path>]
```

Run a SQL query against a local DuckDB database. The query can be passed as an argument or read from stdin. Outputs NDJSON to stdout.

- `--db`: Path to DuckDB database

Available tables: `apps`, `app_envs`, `consumers`, `request_logs`, `application_logs`, `spans`. See [duckdb_tables.md](duckdb_tables.md) for schemas.

**Important:** The database may contain data from previous sessions. Always filter queries by `app_id`, `timestamp`, and other relevant fields to avoid including unrelated data.

DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).

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
