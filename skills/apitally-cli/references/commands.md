# Command Reference

All commands accept `--api-key <key>` and `--api-base-url <url>` flags for authentication. API key resolution order: `--api-key` flag > `APITALLY_API_KEY` env var > `~/.apitally/auth.json`.

## auth

```
npx @apitally/cli auth [--api-key <key>]
```

Configure authentication interactively or by providing a key directly. Saves credentials to `~/.apitally/auth.json`.

## whoami

```
npx @apitally/cli whoami
```

Check authentication and show the team name. Output: `{"team":{"id":1,"name":"My Team"}}`.

## apps

```
npx @apitally/cli apps [--db [<path>]]
```

List all apps in the team. Use this to get app IDs for other commands.

NDJSON output fields: `id`, `name`, `framework`, `client_id`, `envs` (array of `{id, name, created_at, last_sync_at}`), `created_at`.

With `--db`, writes to `apps` and `app_envs` tables. Defaults to `~/.apitally/data.duckdb` if no path is given.

## consumers

```
npx @apitally/cli consumers <app-id> [--requests-since <datetime>] [--db [<path>]]
```

List all consumers for an app. Use this to map consumer IDs in request logs to identifiers and names.

| Flag               | Description                                                                                                        |
| ------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `<app-id>`         | App ID (required, positional)                                                                                      |
| `--requests-since` | Only return consumers active since this datetime (ISO 8601)                                                        |
| `--db`             | Write to `consumers` table in DuckDB instead of stdout. Defaults to `~/.apitally/data.duckdb` if no path is given. |

NDJSON output fields: `id`, `identifier`, `name`, `group` (`{id, name}` or null), `created_at`, `last_request_at`.

## request-logs

```
npx @apitally/cli request-logs <app-id> --since <datetime> \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--limit <n>] [--db [<path>]]
```

Fetch request log data for an app.

| Flag        | Description                                                                                                           |
| ----------- | --------------------------------------------------------------------------------------------------------------------- |
| `<app-id>`  | App ID (required, positional)                                                                                         |
| `--since`   | Start of time range, inclusive (ISO 8601, required)                                                                   |
| `--until`   | End of time range, exclusive (ISO 8601, defaults to now)                                                              |
| `--fields`  | JSON array of field names to include                                                                                  |
| `--filters` | JSON array of filter objects                                                                                          |
| `--limit`   | Maximum number of rows (hard cap: 1,000,000)                                                                          |
| `--db`      | Write to `request_logs` table in DuckDB instead of stdout. Defaults to `~/.apitally/data.duckdb` if no path is given. |

Timestamps without timezone are treated as UTC. Results are ordered by timestamp ascending.

### Fields

| Field                     | Type            | Default |
| ------------------------- | --------------- | ------- |
| `timestamp`               | datetime        | yes     |
| `request_uuid`            | ID              | yes     |
| `app_env`                 | string          | yes     |
| `method`                  | string          | yes     |
| `path`                    | string          | yes     |
| `url`                     | string          | yes     |
| `consumer_id`             | int             | yes     |
| `request_headers`         | array of tuples | no      |
| `request_size`            | int             | yes     |
| `request_body_json`       | JSON string     | no      |
| `status_code`             | int             | yes     |
| `response_time_ms`        | int             | yes     |
| `response_headers`        | array of tuples | no      |
| `response_size`           | int             | yes     |
| `response_body_json`      | JSON string     | no      |
| `client_ip`               | string          | yes     |
| `client_country_iso_code` | string          | yes     |
| `exception_type`          | string          | no      |
| `exception_message`       | string          | no      |
| `exception_stacktrace`    | string          | no      |
| `sentry_event_id`         | ID              | no      |
| `trace_id`                | ID              | no      |

Fields marked "yes" are included by default. Use `--fields` to request non-default fields. Fields `timestamp`, `request_uuid`, `method`, and `url` are always included regardless.

### Filters

Pass `--filters` as a JSON array of filter objects. Each object has `field`, `op`, and `value` keys. Multiple filters are combined with AND.

```json
[{ "field": "status_code", "op": "gte", "value": 400 }]
```

Operators by field type:

| Field type                                                                                                                                    | Operators                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| string (`app_env`, `method`, `path`, `url`, `client_ip`, `client_country_iso_code`, `request_body_json`, `response_body_json`, `exception_*`) | `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`                         |
| int (`consumer_id`, `request_size`, `status_code`, `response_time_ms`, `response_size`)                                                       | `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`                                         |
| ID (`request_uuid`, `sentry_event_id`, `trace_id`)                                                                                            | `eq`, `neq`, `in`, `not_in`                                                                   |
| header (`request_headers`, `response_headers`)                                                                                                | `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `exists`, `not_exists` |

For `in` / `not_in`, `value` must be a JSON array. For header fields, add `key` for the header name. For `exists` / `not_exists`, omit `value`.

#### Filter examples

Errors only:

```json
[{ "field": "status_code", "op": "gte", "value": 400 }]
```

Specific endpoint pattern:

```json
[{ "field": "path", "op": "ilike", "value": "/users/%" }]
```

Specific consumer:

```json
[{ "field": "consumer_id", "op": "eq", "value": 42 }]
```

Requests with a specific header:

```json
[{ "field": "request_headers", "key": "x-api-version", "op": "exists" }]
```

Header value match:

```json
[
  {
    "field": "request_headers",
    "key": "content-type",
    "op": "eq",
    "value": "application/json"
  }
]
```

Response body containing an error:

```json
[{ "field": "response_body_json", "op": "ilike", "value": "%error%" }]
```

Multiple filters (AND):

```json
[
  { "field": "status_code", "op": "gte", "value": 500 },
  { "field": "method", "op": "in", "value": ["POST", "PUT", "PATCH"] }
]
```

## sql

```
npx @apitally/cli sql "<query>" [--db [<path>]]
npx @apitally/cli sql --db < query.sql
echo "SELECT COUNT(*) FROM request_logs" | npx @apitally/cli sql --db
```

Run a SQL query against a local DuckDB database. The query can be passed as an argument or read from stdin. Output is NDJSON. Defaults to `~/.apitally/data.duckdb` if `--db` is omitted or given without a path.

Available tables: `apps`, `app_envs`, `consumers`, `request_logs`.

DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).
