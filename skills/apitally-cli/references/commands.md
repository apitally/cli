# Command Reference

All commands accept an `--api-key <key>` flag for authentication (except `sql`). API key resolution order: `--api-key` flag > `APITALLY_API_KEY` env var > `~/.apitally/auth.json`.

## `auth`

```
npx @apitally/cli auth [--api-key <key>]
```

Configure API key interactively or by providing a key directly. Saves API key to `~/.apitally/auth.json`.

## `whoami`

```
npx @apitally/cli whoami
```

Check authentication and show the team name.

Output: `{"team":{"id":1,"name":"My Team"}}`.

## `apps`

```
npx @apitally/cli apps [--db [<path>]]
```

List all apps in the team. Use this to get app IDs for other commands.

NDJSON output fields: `id`, `name`, `framework`, `client_id`, `envs` (array of `{id, name, created_at, last_sync_at}`), `created_at`.

With `--db`, writes to `apps` and `app_envs` tables in DuckDB instead of stdout. Defaults to `~/.apitally/data.duckdb` if no path is given.

## `consumers`

```
npx @apitally/cli consumers <app-id> [--requests-since <datetime>] [--db [<path>]]
```

List all consumers for an app. Use this to map consumer IDs in request logs to identifiers and names.

- `--requests-since`: Only return consumers active since this datetime (ISO 8601)
- `--db`: Write to `consumers` table in DuckDB instead of stdout. Defaults to `~/.apitally/data.duckdb` if no path is given.

NDJSON output fields: `id`, `identifier`, `name`, `group` (`{id, name}` or null), `created_at`, `last_request_at`.

## `request-logs`

```
npx @apitally/cli request-logs <app-id> --since <datetime> \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--limit <n>] [--db [<path>]]
```

Fetch request log data for an app.

- `--since`: Start of time range, inclusive (ISO 8601, required)
- `--until`: End of time range, exclusive (ISO 8601, defaults to now)
- `--fields`: JSON array of field names to include
- `--filters`: JSON array of filter objects
- `--limit`: Maximum number of rows (hard cap: 1,000,000)
- `--db`: Write to `request_logs` table in DuckDB instead of stdout. Defaults to `~/.apitally/data.duckdb` if no path is given.

Timestamps without timezone are treated as UTC. Results are ordered by timestamp ascending.

### Fields

| Field                     | Type                   | Default |
| ------------------------- | ---------------------- | ------- |
| `timestamp`               | string (datetime)      | yes     |
| `request_uuid`            | string (ID)            | yes     |
| `app_env`                 | string                 | yes     |
| `method`                  | string                 | yes     |
| `path`                    | string                 | yes     |
| `url`                     | string                 | yes     |
| `consumer_id`             | int (ID)               | yes     |
| `request_headers`         | array of string tuples | no      |
| `request_size`            | int                    | yes     |
| `request_body_json`       | string (JSON)          | no      |
| `status_code`             | int                    | yes     |
| `response_time_ms`        | int                    | yes     |
| `response_headers`        | array of string tuples | no      |
| `response_size`           | int                    | yes     |
| `response_body_json`      | string (JSON)          | no      |
| `client_ip`               | string                 | yes     |
| `client_country_iso_code` | string                 | yes     |
| `exception_type`          | string                 | no      |
| `exception_message`       | string                 | no      |
| `exception_stacktrace`    | string                 | no      |
| `sentry_event_id`         | string (ID)            | no      |
| `trace_id`                | string (ID)            | no      |

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

- **string**: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`
- **string (datetime)**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte` — value is an ISO 8601 datetime string
- **string (ID) / int (ID)**: `eq`, `neq`, `in`, `not_in`
- **array of string tuples**: `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `exists`, `not_exists` — requires `key`
- **int**: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`

#### Value rules

- `in`/`not_in`: value must be a JSON array (of strings or ints matching the field type)
- `exists`/`not_exists`: omit value entirely
- `like`/`ilike`: use `%` as wildcard

#### Filter examples

```json
[{"field": "consumer_id", "op": "eq", "value": 42}]
[{"field": "path", "op": "ilike", "value": "/users/%"}]
[{"field": "status_code", "op": "gte", "value": 400},{"field": "status_code", "op": "lt", "value": 500}]
[{"field": "request_headers", "key": "x-api-version", "op": "exists"}]
[{"field": "request_headers", "key": "content-type", "op": "eq", "value": "application/json"}]
[{"field": "response_body_json", "op": "ilike", "value": "%error%"}]
```

## `sql`

```
npx @apitally/cli sql "<query>" [--db <path>]
npx @apitally/cli sql < query.sql
echo "SELECT COUNT(*) FROM request_logs" | npx @apitally/cli sql
```

Run a SQL query against a local DuckDB database. The query can be passed as an argument or read from stdin. Output is NDJSON. Defaults to `~/.apitally/data.duckdb` if `--db` is omitted.

Available tables: `apps`, `app_envs`, `consumers`, `request_logs`. See [tables.md](tables.md) for schemas.

DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).
