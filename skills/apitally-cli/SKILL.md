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

The Apitally CLI retrieves API request log data from [Apitally](https://apitally.io) and optionally stores it in a local DuckDB database for investigation with SQL. Each record is an individual API request with method, URL, status code, response time, consumer, headers, payloads, exceptions, and more. Request log retention is **15 days**. Older data is not available.

Run commands with npx (no install needed):

```
npx @apitally/cli <command> [--api-key <key>]
```

All commands output NDJSON to stdout by default. With `--db`, data is written to a DuckDB database instead (`~/.apitally/data.duckdb` by default), enabling SQL queries via the `sql` command.

A team-scoped API key is required to use the CLI. The `auth` command writes the provided API key to `~/.apitally/auth.json`. It's then used by all subsequent commands unless overridden by the `--api-key` flag.

## Workflow

1. **Check authentication** -- run `npx @apitally/cli whoami`. If it fails, ask the user to provide their Apitally API key or run `npx @apitally/cli auth` themselves. Explain that API keys can be created in the Apitally dashboard under Settings > API keys (https://app.apitally.io/settings/api-keys). If the user provides a key, run `npx @apitally/cli auth --api-key <key>` to store it.

2. **Identify the app** -- run `npx @apitally/cli apps` to list apps and get their IDs. If there is only one app, use it. Otherwise, if the correct app can't be inferred from the user's previous messages, ask the user which app they mean.

3. **Determine if consumers are involved** -- decide which scenario applies:
   - **(a) Specific consumer(s)**: the user is asking about specific consumers (e.g. by email, name, or group). Fetch consumers first, then query to find the matching `consumer_id`, then use it as a filter when fetching request logs.
   - **(b) Consumer context needed**: the investigation involves consumers but not specific ones known upfront (e.g. "which consumers cause the most errors"). Fetch consumers into DuckDB for later JOINs with request logs.
   - **(c) No consumer involvement**: skip fetching consumers.

4. **Fetch consumers** into DuckDB (only if scenario (a) or (b) applies):

   ```
   npx @apitally/cli consumers <app-id> --db
   ```

   For scenario (a), query to find the consumer ID:

   ```
   npx @apitally/cli sql "SELECT consumer_id, identifier, name, \"group\" FROM consumers WHERE identifier ILIKE 'user@example.com'" --db
   ```

5. **Fetch request logs** into DuckDB with time range, fields and filters tailored to the investigation:

   ```
   npx @apitally/cli request-logs <app-id> --since "2026-03-23T00:00:00Z" \
     --fields '["timestamp","method","path","status_code","response_time_ms","consumer_id","request_headers","request_body_json","response_body_json","exception_type","exception_message","exception_stacktrace"]' \
     --filters '[{"field":"status_code","op":"gte","value":400}]' \
     --db
   ```

   For scenario (a), add a `consumer_id` filter: `[{"field":"consumer_id","op":"in","value":[1,2,3]}]`

6. **Query with SQL**:

   ```
   npx @apitally/cli sql "SELECT method, path, status_code, COUNT(*) as n FROM request_logs WHERE status_code >= 400 GROUP BY ALL ORDER BY n DESC" --db
   ```

7. **Iterate** -- refine filters, fetch additional fields (headers, bodies, exceptions), or widen the time range as needed

Use `--filters` and `--fields` to narrow data at fetch time as much as possible to avoid fetching unnecessary data. Refetching data replaces existing records in DuckDB and does not create duplicates.

## Command Quick Reference

All commands are run via `npx @apitally/cli <command>`. For full details, see [references/commands.md](references/commands.md).

- `auth [--api-key <key>]` -- configure API key
- `whoami` -- check auth, show team
- `apps [--db [<path>]]` -- list apps (get app IDs)
- `consumers <app-id> [--db [<path>]]` -- list consumers for an app
- `request-logs <app-id> --since <dt> [--until <dt>] [--fields <json>] [--filters <json>] [--limit <n>] [--db [<path>]]` -- fetch request logs (max 1,000,000 rows at once)
- `sql "<query>" [--db [<path>]]` -- run SQL against local DuckDB

## Investigation Patterns

For DuckDB table schemas, see [references/tables.md](references/tables.md).

### Find failing requests

```sql
SELECT timestamp, method, path, status_code, response_time_ms, client_ip
FROM request_logs
WHERE status_code >= 400
ORDER BY timestamp DESC
LIMIT 50
```

### Error breakdown by endpoint

```sql
SELECT method, path, status_code, COUNT(*) as count
FROM request_logs
WHERE status_code >= 400
GROUP BY method, path, status_code
ORDER BY count DESC
```

### Find slow requests

```sql
SELECT timestamp, method, path, status_code, response_time_ms
FROM request_logs
ORDER BY response_time_ms DESC
LIMIT 20
```

### Response time by endpoint (p50, p95)

```sql
SELECT method, path,
       COUNT(*) as count,
       ROUND(quantile_cont(response_time_ms, 0.5)) as p50_ms,
       ROUND(quantile_cont(response_time_ms, 0.95)) as p95_ms,
       MAX(response_time_ms) as max_ms
FROM request_logs
GROUP BY method, path
ORDER BY p95_ms DESC
```

### Inspect a specific request

To inspect headers and payloads, first re-fetch with additional fields:

```
npx @apitally/cli request-logs <app-id> --since "2026-03-23T00:00:00Z" \
  --fields '["timestamp","request_uuid","method","url","status_code","response_time_ms","request_headers","request_body_json","response_body_json","exception_type","exception_message","exception_stacktrace"]' \
  --filters '[{"field":"request_uuid","op":"eq","value":"<uuid>"}]' \
  --db
```

Then query:

```sql
SELECT * FROM request_logs WHERE request_uuid = '<uuid>'
```

### Trace a consumer's activity

```sql
SELECT r.timestamp, r.method, r.path, r.status_code, r.response_time_ms,
       c.identifier, c.name as consumer_name
FROM request_logs r
JOIN consumers c ON r.app_id = c.app_id AND r.consumer_id = c.consumer_id
WHERE c.identifier = 'user@example.com'
ORDER BY r.timestamp DESC
```

### Top consumers by error count

```sql
SELECT c.identifier, c.name,
       COUNT(*) as total_requests,
       SUM(CASE WHEN r.status_code >= 400 THEN 1 ELSE 0 END) as errors
FROM request_logs r
JOIN consumers c ON r.app_id = c.app_id AND r.consumer_id = c.consumer_id
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
WHERE exception_type IS NOT NULL
GROUP BY exception_type, exception_message
ORDER BY count DESC
```

### Query headers

Headers are stored as arrays of `STRUCT("1" VARCHAR, "2" VARCHAR)` (name-value tuples). Use DuckDB list comprehensions:

```sql
SELECT timestamp, method, path,
       [s."2" FOR s IN request_headers IF lower(s."1") = 'content-type'][1] as content_type
FROM request_logs
WHERE request_headers IS NOT NULL
LIMIT 20
```

### Query JSON body fields

Body fields (`request_body_json`, `response_body_json`) are JSON. Use DuckDB JSON functions:

```sql
SELECT timestamp, method, path,
       response_body_json::JSON->>'error' as error_message
FROM request_logs
WHERE response_body_json IS NOT NULL
  AND response_body_json::JSON->>'error' IS NOT NULL
```

### Requests by country

```sql
SELECT client_country_iso_code, COUNT(*) as count
FROM request_logs
WHERE client_country_iso_code IS NOT NULL
GROUP BY client_country_iso_code
ORDER BY count DESC
```

## Exit Codes

| Code | Meaning                         |
| ---- | ------------------------------- |
| 0    | Success                         |
| 2    | Usage error (invalid arguments) |
| 3    | Authentication error            |
| 4    | Input error (invalid values)    |
| 5    | API / network error             |

## References

- [Command reference](references/commands.md) -- full flags, fields, filters, and operators
- [DuckDB table schemas](references/tables.md) -- column types, relationships, and special types
