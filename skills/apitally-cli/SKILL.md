---
name: apitally-cli
description: >
  Retrieve and investigate API metrics, request logs, traces, and spans from Apitally.
  Fetches metrics, request logs, trace spans, consumers, and app metadata via the CLI,
  stores data in a local DuckDB database, and runs SQL queries to investigate issues
  or answer questions. Use when the user mentions Apitally, the Apitally CLI, API
  metrics, API request logs, API consumers, slow database or external calls,
  trace IDs, span errors, or instrumentation events.
---

# Apitally CLI

The Apitally CLI retrieves data from [Apitally](https://apitally.io) and optionally stores it in a local DuckDB database for investigation with SQL. Three main data sources:

- **Metrics** — pre-aggregated data (request counts, error rates, response time percentiles, throughput). Retention: **30 days** at 1-minute intervals, **13 months** at 30-minute intervals.
- **Request logs** - individual API requests with method, URL, status code, response time, consumer, headers, payloads, exceptions, and trace IDs. Retention: **15 days**.
- **Trace spans** - individual operations such as database queries and external calls, with duration, status, attributes, and events. Availability depends on ingestion, configuration, and span retention.

Run commands with `npx` (no install needed):

```
npx @apitally/cli <command> [--api-key <key>]
```

A team-scoped API key is required to use the CLI. The `auth` command saves an API key to `~/.apitally/auth.json`, which is then used by all subsequent commands unless overridden by the `--api-key` flag. If any command exits with code 3 (auth error), ask the user to run `npx @apitally/cli auth` to authenticate, then continue.

List results and SQL queries output NDJSON to stdout. `request-details` and `whoami` return one JSON object; setup/reset status messages go to stderr. With `--db`, fetching commands write to DuckDB instead (`~/.apitally/data.duckdb` by default), enabling queries via `sql`.

## Key Concepts

- **App and environment** — An app is a monitored API application, identified by a numeric `app_id`. Each app has one or more environments (e.g. "prod", "dev"). The `env` field on request logs is a string matching the environment name.
- **Consumer** — An API client or user tracked by Apitally. `consumer_id` is a numeric internal ID (surrogate key, used in request log filters and JOINs). `identifier` is a string set by the application (e.g. email, username) to uniquely identify the consumer. `name` is a display name (auto-generated from `identifier` if not explicitly set). `group` is an optional group name.
- **Path vs URL** — `path` is the parameterized route template (e.g. `/users/{user_id}`), good for grouping by endpoint. `url` is the full request URL with actual values and query parameters (e.g. `https://api.example.com/users/123?limit=10`).
- **Application logs** — Server-side log entries emitted by application code during request handling. Only available via `request-details` as the `logs` field.
- **Spans** - OpenTelemetry units of work retrieved through `traces` or `request-details`. Identity is `(app_id, trace_id, span_id)`; parent links use `parent_span_id` within the same app and trace. Spans can exist without request logs. Multiple requests can share a trace.
- **Shared span storage** - Both fetching commands write to one `spans` table. The latest fetch replaces complete matching rows, clearing omitted columns. Request-details clears span `env`, `events`, `scope_name`, and `scope_version`. Other spans remain, including on an empty response.

## Command Quick Reference

All commands are run via `npx @apitally/cli <command>`. For full details, see [references/commands.md](references/commands.md).

- `auth [--api-key <key>]` -- configure API key
- `whoami` -- check auth, show team
- `apps [--db [<path>]]` -- list apps (get app IDs)
- `consumers <app-id> [--requests-since <dt>] [--db [<path>]]` -- list consumers for an app (get consumer IDs)
- `endpoints <app-id> [--method <methods>] [--path <pattern>] [--db [<path>]]` -- list endpoints for an app
- `metrics <app-id> --since <dt> [--until <dt>] --metrics <json> [--interval <interval>] [--group-by <json>] [--filters <json>] [--timezone <tz>] [--db [<path>]]` -- fetch aggregated metrics
- `request-logs <app-id> --since <dt> [--until <dt>] [--fields <json>] [--filters <json>] [--sample <n|rate>] [--limit <n>] [--db [<path>]]` -- fetch request logs (max 1,000,000 rows at once)
- `traces <app-id> [--since <dt>] [--until <dt>] [--fields <json>] [--filters <json>] [--sample <n|rate>] [--limit <n>] [--db [<path>]]` -- fetch individual spans; `--since` is required unless a nonempty positive `trace_id` filter uses `eq` or `in`
- `request-details <app-id> <request-uuid> [--db [<path>]]` -- fetch full details for a single request (including headers, payloads, exception info, application logs, and spans)
- `sql "<query>" [--db <path>]` -- run SQL against local DuckDB
- `reset-db [--db <path>]` -- drop and recreate all tables in local DuckDB

## Investigation Workflow

1. **Identify the app** — run `npx @apitally/cli apps` to list apps and get their IDs. If there is more than one app, and the correct app can't be inferred from the user's messages, ask the user which app they mean. Use the app ID consistently for all commands and SQL `WHERE` conditions throughout the investigation.

2. **Determine the time range** - use the user's range or default to the last 7 days for discovery. Keep fetch flags and SQL scope consistent. For known trace IDs, omit unnecessary time bounds to include available sibling spans outside the discovery window.

3. **Fetch supporting data if needed** — skip unless you need endpoint discovery or consumer identification.
   - **Endpoints**: use `endpoints` to discover available method/path combinations for filtering. Use `--method` and/or `--path` to filter (e.g. `--path '*users*'`).

     ```
     npx @apitally/cli endpoints <app-id> [--method <methods>] [--path <pattern>]
     ```

   - **Consumers**: use `consumers` to map identifiers (emails, usernames, groups) to `consumer_id` values and vice versa, if the question involves consumers.

     ```
     npx @apitally/cli consumers <app-id> [--requests-since "<since>"] --db
     ```

     ```
     npx @apitally/cli sql "SELECT consumer_id, identifier, name, \"group\" FROM consumers WHERE app_id = <app-id> AND identifier ILIKE '%@example.com'"
     ```

4. **Fetch data** — choose based on the question. Always read the [command reference](references/commands.md) for available options.
   - **Metrics** — for questions that can be answered with aggregated metrics: traffic volume, error rates, response time trends, throughput, endpoint comparisons. Use `--group-by` and `--interval` to break down by environment, endpoint, consumer, status code, or time period.

     ```
     npx @apitally/cli metrics <app-id> --since "<since>" \
       --metrics '["requests","error_rate","response_time_p50","response_time_p95"]' \
       --group-by '["method","path"]' --interval day --db
     ```

   - **Request logs** - for individual requests: errors, exceptions, headers, payloads, or requests to correlate with traces. Explicitly include `trace_id` in `--fields` for correlation; it is not a default field. Narrow fields and filters to avoid unnecessary volume. Refetching replaces existing records in DuckDB (no duplicates).

     ```
     npx @apitally/cli request-logs <app-id> --since "<since>" \
       --fields '<json-array-of-field-names>' \
       --filters '<json-array-of-filter-objects>' \
       --db
     ```

     Filter by endpoint: `--filters '[{"field":"method","op":"eq","value":"GET"},{"field":"path","op":"eq","value":"/v1/users/{user_id}"}]'`
     Filter by consumer: `--filters '[{"field":"consumer_id","op":"in","value":[1,2,3]}]'`

   - **Trace spans** - for slow database or external calls, instrumentation scopes, span errors, or events. Use `traces` to discover matching spans, then fetch discovered trace IDs without the discovery filters or sampling to inspect sibling spans. See the trace patterns below and the [full field/filter contract](references/commands.md#traces).

   - **Combine sources** - start broad investigations with metrics, then drill into request logs or spans. Use `request-details` for the full single-request view, including application logs.

5. **Query DuckDB** using `sql` - the database persists across fetches and sessions. Filter every query by `app_id` and the investigation scope: `period_start` for metrics, `timestamp` for request logs, and exact `trace_id` values or integer `start_time_ns` bounds for spans. Include relevant environment/endpoint filters. Otherwise, unrelated stored rows can change the answer.

   ```
   npx @apitally/cli sql "SELECT method, path, status_code, COUNT(*) as n FROM request_logs WHERE app_id = <app-id> AND timestamp >= '<since>' AND status_code >= 400 GROUP BY ALL ORDER BY n DESC"
   ```

   Read the [DuckDB schema reference](references/duckdb_tables.md) for available tables, columns and relationships.

6. **Iterate if needed** — refine filters, fetch additional fields (headers, bodies, exceptions), or widen the time range as needed.

## Investigation Patterns

### Error investigation

Fetch request counts grouped by endpoint and status code to find the most frequent errors:

```
npx @apitally/cli metrics <app-id> --since "<since>" \
  --metrics '["requests"]' \
  --group-by '["method","path","status_code"]' \
  --filters '[{"field":"status_code","op":"gte","value":400}]' --db
```

```sql
SELECT method, path, status_code, sum(requests) as requests_sum
FROM metrics
WHERE app_id = <app-id>
  AND period_start >= '<since>'
GROUP BY method, path, status_code
ORDER BY requests_sum DESC
```

Then fetch request logs for a specific error to investigate further:

```
npx @apitally/cli request-logs <app-id> --since "<since>" \
  --fields '["timestamp","request_uuid","url","status_code","response_body_json","exception_type","exception_message"]' \
  --filters '[{"field":"method","op":"eq","value":"<method>"},{"field":"path","op":"eq","value":"<path>"},{"field":"status_code","op":"eq","value":<status_code>}]' \
  --limit 5
```

Use `request-details` to fetch full details (headers, body, exception, application logs, spans) for a specific request:

```
npx @apitally/cli request-details <app-id> <request-uuid>
```

### Discover slow spans and expand a trace

The examples below use app `1` and a sample trace ID; substitute IDs from your results. Discover spans taking at least 100 ms. SQL time bounds use epoch nanoseconds because the bundled DuckDB does not support `TIMESTAMPTZ - INTERVAL` (24 hours = 86400000000000 ns):

```bash
npx @apitally/cli traces 1 --since 24h \
  --filters '[{"field":"duration_ns","op":"gte","value":100000000}]' --db
npx @apitally/cli sql "SELECT trace_id, span_id, name, duration_ns / 1000000.0 AS duration_ms FROM spans WHERE app_id = 1 AND start_time_ns >= epoch_ns(current_timestamp) - 86400000000000 AND duration_ns >= 100000000 ORDER BY duration_ns DESC LIMIT 20"
```

Fetch all available spans for a discovered trace ID. Keep only the positive trace-ID filter, remove sampling and unnecessary time bounds or lower limits, and select the fields needed for the investigation. `--fields` replaces defaults, so this example lists all fields:

```bash
npx @apitally/cli traces 1 \
  --filters '[{"field":"trace_id","op":"in","value":["0123456789abcdef0123456789abcdef"]}]' \
  --fields 'trace_id,span_id,parent_span_id,env,name,kind,status,start_time_ns,end_time_ns,duration_ns,attributes,events,scope_name,scope_version' --db
npx @apitally/cli sql "SELECT span_id, parent_span_id, name, kind, status, duration_ns / 1000000.0 AS duration_ms FROM spans WHERE app_id = 1 AND trace_id = '0123456789abcdef0123456789abcdef' ORDER BY start_time_ns, span_id"
```

Filters select spans, not whole traces. Samples, limits, and time bounds can omit siblings; do not use partial sets as complete-trace statistics. Even an unrestricted ID lookup is subject to ingestion, retention, and the 1,000,000-span cap. See [JSON examples](references/duckdb_json_functions.md#span-attributes-and-events) for raw attribute strings and event timestamps.

### Correlate request logs with spans

Select `trace_id` explicitly, find relevant IDs, then expand them using the trace-ID fetch above:

```bash
npx @apitally/cli request-logs 1 --since 24h \
  --fields 'trace_id,status_code,response_time_ms' --db
npx @apitally/cli sql "SELECT request_uuid, trace_id, status_code, response_time_ms FROM request_logs WHERE app_id = 1 AND epoch_ns(timestamp) >= epoch_ns(current_timestamp) - 86400000000000 AND trace_id IS NOT NULL ORDER BY response_time_ms DESC LIMIT 20"
```

Join on **both app and trace IDs**, not request UUID:

```sql
SELECT r.request_uuid, s.span_id, s.name, s.duration_ns / 1000000.0 AS duration_ms
FROM request_logs r
JOIN spans s ON s.app_id = r.app_id AND s.trace_id = r.trace_id
WHERE r.app_id = 1
  AND r.trace_id = '0123456789abcdef0123456789abcdef'
ORDER BY r.request_uuid, s.start_time_ns, s.span_id;
```

This can return many rows per request, and multiple requests can share a trace. Use `EXISTS` or deduplication for request counts, as shown in the [schema reference](references/duckdb_tables.md#relationships).

### Trace a consumer's activity

```sql
SELECT r.timestamp, r.method, r.url, r.status_code, r.response_time_ms
FROM request_logs r
JOIN consumers c ON r.app_id = c.app_id AND r.consumer_id = c.consumer_id
WHERE r.app_id = <app-id>
  AND r.timestamp >= '<since>'
  AND r.timestamp < '<until>'
  AND c.identifier = 'user@example.com'
ORDER BY r.timestamp ASC
```

### Query headers

Headers are stored as `STRUCT(name VARCHAR, value VARCHAR)[]`. Use DuckDB list comprehensions:

```sql
-- Extract a specific header value
SELECT timestamp, method, path,
       [s.value FOR s IN request_headers IF lower(s.name) = 'content-type'][1] as content_type
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND request_headers IS NOT NULL
LIMIT 20

-- Filter by header existence
SELECT timestamp, method, path
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND len([s FOR s IN request_headers IF lower(s.name) = 'authorization']) > 0
```

### Query JSON body fields

Body fields (`request_body_json`, `response_body_json`) are of type `JSON`. Use DuckDB JSON operators and functions.

**Note:** Request/response bodies larger than 50 KB are not captured by the SDKs and will be `NULL`.

```sql
SELECT timestamp, method, path,
       response_body_json->>'$.error' as error_message
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND response_body_json IS NOT NULL
  AND (response_body_json->>'$.error') IS NOT NULL
```

See [references/duckdb_json_functions.md](references/duckdb_json_functions.md) for more JSON functions and examples.

### Date truncation and grouping

Datetime columns are `TIMESTAMPTZ`. Direct casts like `timestamp::DATE` error out (no ICU extension). Use `AT TIME ZONE 'UTC'` first:

```sql
SELECT date_trunc('day', timestamp AT TIME ZONE 'UTC') AS day, count(*) AS n
FROM request_logs WHERE app_id = <app-id> AND timestamp >= '<since>'
GROUP BY day ORDER BY day
```

## Legacy Database Recovery

If a command reports an incompatible legacy span schema, preserve the old file by choosing a new `--db` path, or obtain user permission to run `reset-db` against the same path and refetch. Reset clears **all tables**, not just spans. Data beyond API retention may no longer be available. See [reset-db](references/commands.md#reset-db) for exact commands.

## Exit Codes

| Code | Meaning                         |
| ---- | ------------------------------- |
| 0    | Success                         |
| 1    | Unknown error                   |
| 2    | Usage error (invalid arguments) |
| 3    | Authentication error            |
| 4    | Input error (invalid values)    |
| 5    | API / network error             |

## References

- [Command reference](references/commands.md) -- full flags, fields, filters, and operators
- [DuckDB table schemas](references/duckdb_tables.md) -- column types, relationships, and special types
- [DuckDB JSON functions](references/duckdb_json_functions.md) -- extraction operators, JSONPath, unnesting arrays
