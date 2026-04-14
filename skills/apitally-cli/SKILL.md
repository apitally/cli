---
name: apitally-cli
description: >
  Retrieve and investigate API metrics and request log data from Apitally. Fetches
  aggregated metrics, request logs, consumers, and app metadata via the Apitally CLI,
  stores data in a local DuckDB database, and runs SQL queries to investigate issues
  or answer questions. Use when the user mentions Apitally, the Apitally CLI, API
  metrics, API request logs, or API consumers.
---

# Apitally CLI

The Apitally CLI retrieves API metrics and request log data from [Apitally](https://apitally.io) and optionally stores it in a local DuckDB database for investigation with SQL. Two main data sources:

- **Metrics** — pre-aggregated data (request counts, error rates, response time percentiles, throughput). Retention: **30 days** at 1-minute intervals, **13 months** at 30-minute intervals.
- **Request logs** — individual API requests with method, URL, status code, response time, consumer, headers, payloads, exceptions, traces, and more. Retention: **15 days**.

Run commands with `npx` (no install needed):

```
npx @apitally/cli <command> [--api-key <key>]
```

A team-scoped API key is required to use the CLI. The `auth` command writes the provided API key to `~/.apitally/auth.json`. It's then used by all subsequent commands unless overridden by the `--api-key` flag.

All commands output NDJSON to stdout by default. With `--db`, data is written to a DuckDB database instead (`~/.apitally/data.duckdb` by default), enabling SQL queries via the `sql` command.

## Key Concepts

- **App and environment** — An app is a monitored API application, identified by a numeric `app_id`. Each app has one or more environments (e.g. "prod", "dev"). The `env` field on request logs is a string matching the environment name.
- **Consumer** — An API client or user tracked by Apitally. `consumer_id` is a numeric internal ID (surrogate key, used in request log filters and JOINs). `identifier` is a string set by the application (e.g. email, username) to uniquely identify the consumer. `name` is a display name (auto-generated from `identifier` if not explicitly set). `group` is an optional group name.
- **Path vs URL** — `path` is the parameterized route template (e.g. `/users/{user_id}`), good for grouping by endpoint. `url` is the full request URL with actual values and query parameters (e.g. `https://api.example.com/users/123?limit=10`).
- **Application logs** — Server-side log entries emitted by application code during request handling. Only available via `request-details` as the `logs` field.
- **Spans** — OpenTelemetry trace spans representing units of work during request handling (e.g. database queries, external API calls, instrumented function calls). Only available via `request-details`. Form a tree via `parent_span_id`.

## Command Quick Reference

All commands are run via `npx @apitally/cli <command>`. For full details, see [references/commands.md](references/commands.md).

- `auth [--api-key <key>]` -- configure API key
- `whoami` -- check auth, show team
- `apps [--db [<path>]]` -- list apps (get app IDs)
- `consumers <app-id> [--requests-since <dt>] [--db [<path>]]` -- list consumers for an app (get consumer IDs)
- `endpoints <app-id> [--method <methods>] [--path <pattern>] [--db [<path>]]` -- list endpoints for an app
- `metrics <app-id> --since <dt> [--until <dt>] --metrics <json> [--interval <interval>] [--group-by <json>] [--filters <json>] [--timezone <tz>] [--db [<path>]]` -- fetch aggregated metrics
- `request-logs <app-id> --since <dt> [--until <dt>] [--fields <json>] [--filters <json>] [--limit <n>] [--db [<path>]]` -- fetch request logs (max 1,000,000 rows at once)
- `request-details <app-id> <request-uuid> [--db [<path>]]` -- fetch full details for a single request (including headers, payloads, exception info, application logs, and spans)
- `sql "<query>" [--db <path>]` -- run SQL against local DuckDB
- `reset-db [--db <path>]` -- drop and recreate all tables in local DuckDB

## Investigation Workflow

1. **Check authentication** — run `npx @apitally/cli whoami`. If it fails, ask the user to run `npx @apitally/cli auth` to set their API key. Explain that API keys can be created in the Apitally dashboard under Settings > API keys (https://app.apitally.io/settings/api-keys).

2. **Identify the app** — run `npx @apitally/cli apps` to list apps and get their IDs. If there is more than one app, and the correct app can't be inferred from the user's messages, ask the user which app they mean. Use the app ID consistently for all commands and SQL `WHERE` conditions throughout the investigation.

3. **Determine the time range** — check if the user specified a time range (e.g. "last 24 hours", "since Monday", a specific date). If not, default to the last 7 days. Use this time range consistently for `--requests-since` / `--since` / `--until` flags and SQL `WHERE` conditions throughout the investigation.

4. **Fetch supporting data if needed** — skip unless you need endpoint discovery or consumer identification.
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

5. **Fetch data** — choose based on the question. Always read the [command reference](references/commands.md) for available options.
   - **Metrics** — for questions that can be answered with aggregated metrics: traffic volume, error rates, response time trends, throughput, endpoint comparisons. Use `--group-by` and `--interval` to break down by environment, endpoint, consumer, status code, or time period.

     ```
     npx @apitally/cli metrics <app-id> --since "<since>" \
       --metrics '["requests","error_rate","response_time_p50","response_time_p95"]' \
       --group-by '["method","path"]' --interval day --db
     ```

   - **Request logs** — for questions that require individual request data: specific errors, exceptions, headers, payloads, traces, etc. Narrow down fields and use filters to avoid fetching unnecessarily large volumes of data. Refetching replaces existing records in DuckDB (no duplicates).

     ```
     npx @apitally/cli request-logs <app-id> --since "<since>" \
       --fields '<json-array-of-field-names>' \
       --filters '<json-array-of-filter-objects>' \
       --db
     ```

     Filter by endpoint: `--filters '[{"field":"method","op":"eq","value":"GET"},{"field":"path","op":"eq","value":"/v1/users/{user_id}"}]'`
     Filter by consumer: `--filters '[{"field":"consumer_id","op":"in","value":[1,2,3]}]'`

   - **Both** — for broad investigations, start with metrics for an overview, then fetch request logs to drill into specifics.

6. **Query DuckDB** using the `sql` command — **CRITICAL: The DuckDB database is persistent and retains data from previous fetches, including other sessions. You MUST filter your SQL queries to match the scope of your current investigation.** Always include `WHERE` conditions on `app_id`, `period_start`/`timestamp`, and any other relevant fields. Without these filters, results will include unrelated data and will be **wrong**.

   ```
   npx @apitally/cli sql "SELECT method, path, status_code, COUNT(*) as n FROM request_logs WHERE app_id = <app-id> AND timestamp >= '<since>' AND status_code >= 400 GROUP BY ALL ORDER BY n DESC"
   ```

   Read the [DuckDB schema reference](references/duckdb_tables.md) for available tables, columns and relationships.

7. **Iterate if needed** — refine filters, fetch additional fields (headers, bodies, exceptions), or widen the time range as needed.

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
