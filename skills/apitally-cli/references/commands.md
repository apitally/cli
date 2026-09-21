# Command Reference

API commands accept an `--api-key <key>` flag for authentication (`sql` and `reset-db` use only the local database). API key resolution order: `--api-key` flag > `APITALLY_API_KEY` env var > `~/.apitally/auth.json`.

Commands that accept a `--db` flag use `~/.apitally/data.duckdb` as the default database path if no other path is specified. If the database file doesn't exist, it will be created (except for the `sql` command). When writing to tables, existing records are updated (no duplicates are created).

Datetime flags (e.g. `--since`, `--until`, `--requests-since`) accept ISO 8601 strings or compact relative durations (e.g. `30m`, `24h`, `7d`, `2w`).

## `auth`

```
npx @apitally/cli auth [--api-key <key>]
```

Opens a browser-based auth flow where the user logs in to the Apitally dashboard and selects a team. A newly created API key is then passed back to the CLI. The key is saved to `~/.apitally/auth.json` and used by all subsequent commands unless overridden by the `--api-key` flag.

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
{"id":1,"identifier":"bob@example.com","name":"Bob","group":{"id":1,"name":"Admins"},"created_at":"2026-01-01T00:00:00Z","last_request_at":"2026-01-01T01:00:00Z"}
{"id":2,"identifier":"alice@example.com","name":"Alice","group":null,"created_at":"2026-01-02T00:00:00Z","last_request_at":"2026-01-02T02:00:00Z"}
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
- `--interval`: Time interval for grouping (`month`, `day`, `hour`, `minute`). When omitted, returns a single row per group for the entire time range
- `--group-by`: Comma-separated list or JSON array of field names to group by, in addition to time period
- `--filters`: JSON array of filter objects (see below)
- `--timezone`: Timezone for intervals and to interpret since/until if not tz-aware (defaults to system timezone)
- `--db`: Write to `metrics` table in DuckDB instead of outputting NDJSON to stdout

**Deduplication in DuckDB:** Deletes all existing rows for the same `app_id` within the fetched time range before inserting new data.

### Available metrics

| Metric                | Type    | Description                            |
| --------------------- | ------- | -------------------------------------- |
| `requests`            | integer | Total request count                    |
| `requests_per_minute` | float   | Requests per minute                    |
| `bytes_received`      | integer | Total bytes received                   |
| `bytes_sent`          | integer | Total bytes sent                       |
| `client_errors`       | integer | 4xx errors (excluding expected errors) |
| `server_errors`       | integer | 5xx errors (excluding expected errors) |
| `error_rate`          | float   | Ratio of errors to total requests      |
| `response_time_p50`   | integer | 50th percentile response time (ms)     |
| `response_time_p75`   | integer | 75th percentile response time (ms)     |
| `response_time_p90`   | integer | 90th percentile response time (ms)     |
| `response_time_p95`   | integer | 95th percentile response time (ms)     |
| `response_time_p99`   | integer | 99th percentile response time (ms)     |

### Group-by fields

`env`, `consumer_id`, `method`, `path`, `status_code`

### Filters

Pass `--filters` as a JSON array of filter objects. Supported fields and operators:

- **string fields** (`env`, `method`, `path`): `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `contains`, `not_contains`
- **numeric fields** (`consumer_id`, `status_code`): `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`, `is_null`, `is_not_null`

Filter examples:

```json
[{"field":"method","op":"eq","value":"GET"}]
[{"field":"status_code","op":"gte","value":400}]
[{"field":"path","op":"like","value":"/v1/users/%"}]
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
| `trace_id`                | string (ID)                      | yes     |
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

Default fields are included when `--fields` is omitted. Providing `--fields` replaces the defaults, but `timestamp`, `request_uuid`, `trace_id`, `method`, and `url` are always included. With `--db`, refetching replaces complete matching rows and sets omitted columns to `NULL`.

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

## `traces`

```
npx @apitally/cli traces <app-id> [--since <datetime>] \
  [--until <datetime>] [--fields <json>] [--filters <json>] \
  [--sample <n|rate>] [--limit <n>] [--db [<path>]]
```

Retrieve trace spans for an app. Each NDJSON row is one span, not a whole trace. Spans can exist without a corresponding request log; request-log endpoint exclusions do not apply.

- `--since`: Inclusive span start time (ISO 8601 or relative duration); required unless a nonempty positive `trace_id` filter uses `eq` or `in`
- `--until`: Exclusive span start time; defaults to now only without a positive trace-ID filter
- `--fields`: Comma-separated list or JSON array of field names to include
- `--filters`: JSON array of filter objects
- `--sample`: Approximate span count (positive integer, e.g. `1000`) or rate (float > 0 and <= 0.5, e.g. `0.1`)
- `--limit`: Maximum number of spans, from 1 to 1,000,000 (default: 1,000,000)
- `--db`: Write to the shared `spans` table instead of stdout; omit the path to use the default database

### Time bounds and result completeness

Without a positive trace-ID filter, `--since` is required and omitted `--until` defaults to now. With a nonempty `trace_id` `eq`/`in` filter, omitted bounds remain unbounded, even when only `--since` is supplied. Explicit bounds still constrain the lookup. `neq`, `not_in`, and `in` with an empty list do not permit omission of `--since`.

Datetimes without a timezone are UTC. If both bounds exist, `since` must precede `until`. Results are ordered by start time ascending, then trace ID and span ID ascending. There is no pagination or continuation token.

Filters return matching spans, not all spans in matching traces. Sampling is approximate and operates on `(trace_id, span_id)`, not whole traces; the limit still applies afterward. Sampling, filters, limits, or time bounds can leave traces incomplete. To expand a discovery result, fetch its trace IDs with only a positive trace-ID filter, removing discovery filters, sampling, and unnecessary bounds or lower limits. The 1,000,000-span cap still applies. Do not treat partial results as complete-trace statistics.

Span availability depends on ingestion, configuration, and span retention. The endpoint allows 1 request/second and 10/minute, with a 30-second query limit.

Without `--db`, the CLI streams NDJSON to stdout unchanged. With `--db`, it streams Arrow into DuckDB; stdout is empty and progress goes to stderr. Both `traces --db` and `request-details --db` populate **one `spans` table**, keyed by `(app_id, trace_id, span_id)`. Each fetch replaces complete matching rows, so omitted optional columns become `NULL`. Other spans remain, including on an empty response. See [legacy database recovery](#reset-db) if an old span schema is detected.

### Fields

| Field | Type | Default | Always included | Meaning |
| --- | --- | --- | --- | --- |
| `trace_id` | string (ID) | yes | yes | 32-character lowercase hex |
| `span_id` | string (ID) | yes | yes | 16-character lowercase hex |
| `parent_span_id` | string (ID) or null | yes | no | 16-character lowercase hex; null for no parent |
| `env` | string or null | yes | no | Environment name; empty storage value becomes null |
| `name` | string | yes | no | Span operation name |
| `kind` | enum string | yes | no | `UNSPECIFIED`, `INTERNAL`, `SERVER`, `CLIENT`, `PRODUCER`, `CONSUMER` |
| `status` | enum string | yes | no | `UNSET`, `OK`, `ERROR`; empty storage value becomes `UNSET` |
| `start_time_ns` | int64 | yes | yes | Unix epoch nanoseconds |
| `end_time_ns` | int64 | yes | no | Unix epoch nanoseconds |
| `duration_ns` | int64 | yes | no | Nanoseconds, not milliseconds |
| `attributes` | object | no | no | Attribute names mapped to JSON values (strings, numbers, booleans, arrays, objects, or null) |
| `events` | array of objects | no | no | Each event has `timestamp`, `name`, and `attributes` |
| `scope_name` | string or null | no | no | Instrumentation scope; empty becomes null |
| `scope_version` | string or null | no | no | Instrumentation scope version; empty becomes null |

Omitting `--fields` selects defaults. Providing it replaces the default set: the API prepends `trace_id`, `span_id`, and `start_time_ns`, removes duplicates, then includes requested fields. `--fields '[]'` returns only those three required fields. In DB mode, refetching required-only fields clears all optional columns of matching rows.

Span and event attributes contain native JSON values. Event timestamps are ISO 8601 UTC strings with nanosecond precision, also preserved in DuckDB JSON. Selected empty collections are `{}` and `[]`; omitted fields become SQL `NULL`. See [JSON extraction examples](duckdb_json_functions.md#span-attributes-and-events).

### Filters

All 14 fields can be filtered, even if not selected for output. Pass `--filters` as a JSON array; clauses are combined with AND.

Filter object keys:

- `field`: Field name (aliases: `column`, `col`)
- `op`: Operator (alias: `operator`)
- `value`: Comparison value (alias: `val`); omit for existence/null checks
- `key`: Case-sensitive attribute name; valid only for `attributes` and `events`
- `event_name`: Exact, case-sensitive event name; valid only for `events`

Field and operator names are trimmed and lowercased. Enum values are case-sensitive.

#### Operators by field type

| Category | Fields | Operators |
| --- | --- | --- |
| String | `env`, `name`, `scope_name`, `scope_version` | `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`, `is_null`, `is_not_null` |
| Integer | `start_time_ns`, `end_time_ns`, `duration_ns` | `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in` |
| Enum | `kind`, `status` | `eq`, `neq`, `in`, `not_in` |
| String ID | `trace_id`, `span_id`, `parent_span_id` | `eq`, `neq`, `in`, `not_in`, `is_null`, `is_not_null` |
| Attribute/event map | `attributes`, `events` | `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`, `exists`, `not_exists` (subject to value type below) |

#### Value rules

- String fields and IDs take JSON strings. `trace_id` requires exactly 32 hex characters; span IDs require exactly 16. Hex case is accepted and normalized to lowercase.
- `is_null`/`is_not_null` are rejected for `trace_id`, `span_id`, and `name`; use them for nullable `parent_span_id`, `env`, `scope_name`, or `scope_version`.
- Integer fields take JSON integers, not numeric strings, floats, or booleans.
- Enum values must use the uppercase values in the field table.
- `in`/`not_in` take arrays of the field's value type; other comparisons take scalars. Scalar-field membership arrays may be empty. JSON null is not a comparison value or array item.
- Omit `value` for `exists`, `not_exists`, `is_null`, and `is_not_null`.
- `like`/`not_like` and `ilike`/`not_ilike` use SQL patterns (`%` and `_`). `ilike` variants ignore case. `contains`/`not_contains` are case-insensitive substring comparisons without wildcard syntax.
- For nullable scalar fields, `not_in` can match rows whose field is null, unlike ordinary SQL `NOT IN`. This does not mean that JSON null is allowed in the supplied list.

#### Attribute and event rules

- `attributes` requires `key` for every operator.
- `events` accepts `key`, `event_name`, or both. Without `key`, supply `event_name` and use only `exists`/`not_exists`.
- String attributes support `eq`, `neq`, `in`, `not_in`, `like`, `not_like`, `ilike`, `not_ilike`, `contains`, `not_contains`.
- Numeric attributes support `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`. Integers and finite floats can be mixed in a membership list.
- Boolean attributes support only `eq`, `neq`, `in`, `not_in`.
- Attribute membership lists must be nonempty and use one scalar type, except the integer/float mixture. Arrays and objects are not comparison values; use existence checks for those attributes.
- Comparisons require the key to exist and its JSON type to match the scalar type. Missing keys do not satisfy `neq` or `not_in`.
- An event clause matches when **any event** satisfies its name/key/value conditions. `not_exists` negates that existence. Event `neq` still means any event with a different value, not that no event equals the value.
- Separate event clauses may match different events in one span. Selecting `events` returns all events for matching spans, not only matching events.

#### Filter examples

Each line is a separate valid `--filters` value:

```json
[{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]
[{"field":"span_id","op":"eq","value":"0123456789abcdef"}]
[{"field":"env","op":"eq","value":"prod"},{"field":"duration_ns","op":"gte","value":100000000}]
[{"field":"kind","op":"in","value":["CLIENT","INTERNAL"]}]
[{"field":"status","op":"eq","value":"ERROR"}]
[{"field":"parent_span_id","op":"is_null"}]
[{"field":"name","op":"ilike","value":"%query%"}]
[{"field":"scope_name","op":"is_not_null"}]
[{"field":"attributes","key":"db.system","op":"eq","value":"postgresql"}]
[{"field":"attributes","key":"retry.count","op":"gte","value":2}]
[{"field":"attributes","key":"cached","op":"eq","value":true}]
[{"field":"attributes","key":"db.statement","op":"exists"}]
[{"field":"events","event_name":"exception","op":"exists"}]
[{"field":"events","event_name":"exception","key":"exception.type","op":"eq","value":"TimeoutError"}]
[{"field":"events","key":"code","op":"in","value":[500,503]}]
```

### Output examples

Default NDJSON:

```json
{"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0123456789abcdef","start_time_ns":1767225600000000000,"parent_span_id":null,"env":"prod","name":"GET /users","kind":"SERVER","status":"OK","end_time_ns":1767225600250000000,"duration_ns":250000000}
```

With `--fields attributes,events` (required fields are still included):

```json
{"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0123456789abcdef","start_time_ns":1767225600000000000,"attributes":{"db.system":"postgresql","retry.count":2,"cached":true},"events":[{"timestamp":"2026-01-01T00:00:00.123456001Z","name":"exception","attributes":{"exception.type":"TimeoutError"}}]}
```

### Command examples

These examples use app `1` and a sample trace ID; substitute IDs from your investigation.

```bash
# Discover slow spans in a time range (100 ms = 100000000 ns).
npx @apitally/cli traces 1 --since 24h \
  --filters '[{"field":"env","op":"eq","value":"prod"},{"field":"duration_ns","op":"gte","value":100000000}]'

# Fetch all available spans for a known trace without time bounds.
npx @apitally/cli traces 1 \
  --filters '[{"field":"trace_id","op":"eq","value":"0123456789abcdef0123456789abcdef"}]'

# Store all fields for discovered trace IDs, without discovery restrictions.
npx @apitally/cli traces 1 \
  --filters '[{"field":"trace_id","op":"in","value":["0123456789abcdef0123456789abcdef"]}]' \
  --fields 'trace_id,span_id,parent_span_id,env,name,kind,status,start_time_ns,end_time_ns,duration_ns,attributes,events,scope_name,scope_version' \
  --db ./trace-investigation.duckdb
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
{"timestamp":"2026-01-01T00:15:00.000Z","request_uuid":"2fbc1df6-3124-4ed1-a376-7d2c64e4d5cf","env":"prod","method":"GET","path":"/test/1","url":"https://api.example.com/test/1","consumer_id":1,"request_headers":[["content-type","application/json"]],"request_size_bytes":0,"request_body_json":null,"status_code":200,"response_time_ms":122,"response_headers":[["x-request-id","abc"]],"response_size_bytes":66,"response_body_json":"{\"ok\":true}","client_ip":"203.0.113.10","client_country_iso_code":"DE","trace_id":"0123456789abcdef0123456789abcdef","exception":null,"logs":[{"timestamp":"2026-01-01T00:15:00.100Z","message":"handling request","level":"INFO","logger":"app","file":"main.py","line":42}],"spans":[{"span_id":"0123456789abcdef","parent_span_id":null,"name":"GET /test/1","kind":"SERVER","start_time_ns":1767226500000000000,"end_time_ns":1767226500050000000,"duration_ns":50000000,"status":"OK","attributes":{"http.method":"GET"}}]}
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

DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview). The bundled DuckDB has no ICU extension, so `TIMESTAMPTZ` columns cannot be cast directly to `DATE`. Use `(timestamp AT TIME ZONE 'UTC')::DATE` or `date_trunc('day', timestamp AT TIME ZONE 'UTC')` for date conversion and grouping.

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
