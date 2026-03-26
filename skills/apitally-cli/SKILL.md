---
name: apitally-cli
description: >
  Retrieve and investigate API request log data from Apitally. Fetches request logs,
  consumers, and app metadata via the Apitally CLI, stores data in a local
  DuckDB database, and runs SQL queries to investigate issues. Use when the user
  wants to investigate API issues, trace a consumer's activity, or inspect
  request/response details. Also use when the user mentions Apitally, the Apitally CLI,
  or API request logs, or when asked to set up or authenticate with the Apitally CLI.
---

# Apitally CLI

The Apitally CLI retrieves API request log data from [Apitally](https://apitally.io) and optionally stores it in a local DuckDB database for investigation with SQL. Each record is an individual API request with method, URL, status code, response time, consumer, headers, payloads, exceptions, and more. Request log retention is **15 days**.

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
- `request-logs <app-id> --since <dt> [--until <dt>] [--fields <json>] [--filters <json>] [--limit <n>] [--db [<path>]]` -- fetch request logs (max 1,000,000 rows at once)
- `request-details <app-id> <request-uuid> [--db [<path>]]` -- fetch full details for a single request (including headers, payloads, exception info, application logs, and spans)
- `sql "<query>" [--db <path>]` -- run SQL against local DuckDB

## Workflow

1. **Check authentication** — run `npx @apitally/cli whoami`. If it fails, ask the user to provide their Apitally API key or run `npx @apitally/cli auth` themselves. Explain that API keys can be created in the Apitally dashboard under Settings > API keys (https://app.apitally.io/settings/api-keys). If the user provides a key, run `npx @apitally/cli auth --api-key <key>` to store it.

2. **Identify the app** — run `npx @apitally/cli apps` to list apps and get their IDs. If there is more than one app, and the correct app can't be inferred from the user's messages, ask the user which app they mean.

3. **Determine if consumers are involved** — decide which scenario applies:
   - **(a) Specific consumer(s)**: the user is asking about specific consumers (e.g. by email, name, or group). Fetch consumers first, then query to find the matching `consumer_id`, then use it as a filter when fetching request logs.
   - **(b) Consumer context needed**: the investigation involves consumers but not specific ones known upfront (e.g. "which consumers cause the most errors"). Fetch consumers into DuckDB for later JOINs with request logs.
   - **(c) No consumer involvement**: skip fetching consumers.

4. **Fetch consumers** into DuckDB (only if scenario (a) or (b) applies):

   ```
   npx @apitally/cli consumers <app-id> --db
   ```

   For scenario (a), query to find the consumer IDs:

   ```
   npx @apitally/cli sql "SELECT consumer_id, identifier, name, \"group\" FROM consumers WHERE identifier ILIKE '%@example.com'"
   ```

5. **Fetch request logs** into DuckDB with time range, fields, and filters tailored to the investigation:

   ```
   npx @apitally/cli request-logs <app-id> --since "2026-03-23T00:00:00Z" \
     --fields '["timestamp","method","path","url","status_code","consumer_id"]' \
     --filters '[{"field":"status_code","op":"gte","value":400}]' \
     --db
   ```

   For scenario (a), add a consumer filter: `{"field":"consumer_id","op":"in","value":[1,2,3]}`

   Narrow down fields and use filters as much as possible to avoid fetching unnecessarily large volumes of data. Refetching data later (e.g. with more fields) replaces existing records in DuckDB and does not create duplicates.

6. **Query with SQL** — **CRITICAL: The DuckDB database is persistent and retains data from previous fetches, including other sessions. You MUST filter your SQL queries to match the scope of your current investigation.** Always include `WHERE` conditions on `app_id`, `timestamp`, and any other relevant fields. Without these filters, results will include unrelated data and will be **wrong**.

   ```
   npx @apitally/cli sql "SELECT method, path, status_code, COUNT(*) as n FROM request_logs WHERE app_id = <app-id> AND timestamp >= '2026-03-23T00:00:00Z' AND status_code >= 400 GROUP BY ALL ORDER BY n DESC"
   ```

7. **Iterate** — refine filters, fetch additional fields (headers, bodies, exceptions), or widen the time range as needed.

## Investigation Patterns

For DuckDB table schemas, see [references/tables.md](references/tables.md).

### Inspect a specific request

Use `request-details` to fetch full details (headers, body, exception, application logs, spans) for a single request:

```
npx @apitally/cli request-details <app-id> <request-uuid>
```

### Trace a consumer's activity

```sql
SELECT r.timestamp, r.method, r.path, r.status_code, r.response_time_ms,
       c.identifier, c.name as consumer_name
FROM request_logs r
JOIN consumers c ON r.app_id = c.app_id AND r.consumer_id = c.consumer_id
WHERE r.app_id = <app-id>
  AND r.timestamp >= '<since>'
  AND c.identifier = 'user@example.com'
ORDER BY r.timestamp DESC
```

### Top consumers by error count

```sql
SELECT c.identifier, c.name,
       COUNT(*) as total_requests,
       SUM(CASE WHEN r.status_code >= 400 THEN 1 ELSE 0 END) as errors
FROM request_logs r
JOIN consumers c ON r.app_id = c.app_id AND r.consumer_id = c.consumer_id
WHERE r.app_id = <app-id>
  AND r.timestamp >= '<since>'
GROUP BY c.identifier, c.name
ORDER BY errors DESC
LIMIT 20
```

### Exception investigation

Fetch with exception fields first:

```
npx @apitally/cli request-logs <app-id> --since "2026-03-23T00:00:00Z" \
  --fields '["timestamp","request_uuid","method","path","status_code","exception_type","exception_message","exception_stacktrace"]' \
  --filters '[{"field":"status_code","op":"eq","value":500}]' \
  --db
```

Then group by exception type:

```sql
SELECT exception_type, exception_message, COUNT(*) as count,
       MIN(timestamp) as first_seen, MAX(timestamp) as last_seen
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND exception_type IS NOT NULL
GROUP BY exception_type, exception_message
ORDER BY count DESC
```

### Query headers

Headers are stored as arrays of `STRUCT(name VARCHAR, value VARCHAR)`. Use DuckDB list comprehensions:

```sql
SELECT timestamp, method, path,
       [s.value FOR s IN request_headers IF lower(s.name) = 'content-type'][1] as content_type
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND request_headers IS NOT NULL
LIMIT 20
```

### Query JSON body fields

Body fields (`request_body_json`, `response_body_json`) are JSON. Use DuckDB JSON operators (parentheses required around `->>` in WHERE clauses due to operator precedence):

```sql
SELECT timestamp, method, path,
       response_body_json->>'error' as error_message
FROM request_logs
WHERE app_id = <app-id>
  AND timestamp >= '<since>'
  AND response_body_json IS NOT NULL
  AND (response_body_json->>'error') IS NOT NULL
```

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
- [DuckDB table schemas](references/tables.md) -- column types, relationships, and special types
