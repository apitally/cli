# DuckDB Table Schemas

Tables are created automatically when using the `--db` flag with `apps`, `consumers`, `endpoints`, `metrics`, `request-logs`, `traces`, or `request-details` commands. DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).

## apps

```sql
CREATE TABLE apps (
    app_id          INTEGER NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    framework       TEXT NOT NULL,            -- e.g. FastAPI, Express
    client_id       TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL
);
```

## app_envs

```sql
CREATE TABLE app_envs (
    app_id          INTEGER NOT NULL,
    app_env_id      INTEGER NOT NULL,
    name            TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    last_sync_at    TIMESTAMPTZ,
    UNIQUE (app_id, app_env_id)
);
```

## consumers

```sql
CREATE TABLE consumers (
    app_id          INTEGER NOT NULL,
    consumer_id     INTEGER NOT NULL,
    identifier      TEXT NOT NULL,             -- e.g. email, username, API key name
    name            TEXT NOT NULL,             -- auto-generated from identifier if not set
    "group"         TEXT,                      -- optional consumer group name
    created_at      TIMESTAMPTZ NOT NULL,
    last_request_at TIMESTAMPTZ,
    UNIQUE (app_id, consumer_id)
);
```

The `identifier` is the consumer string set in the application (e.g. email, username, API key name). The `"group"` column name is quoted because it is a reserved word in SQL.

## endpoints

```sql
CREATE TABLE endpoints (
    app_id          INTEGER NOT NULL,
    endpoint_id     INTEGER NOT NULL,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    UNIQUE (app_id, endpoint_id)
);
```

## metrics

```sql
CREATE TABLE metrics (
    app_id              INTEGER NOT NULL,
    period_start        TIMESTAMPTZ NOT NULL,
    period_end          TIMESTAMPTZ NOT NULL,
    env                 VARCHAR,
    consumer_id         BIGINT,
    method              VARCHAR,
    path                VARCHAR,
    status_code         INTEGER,
    requests            BIGINT,
    requests_per_minute DOUBLE,
    bytes_received      BIGINT,
    bytes_sent          BIGINT,
    client_errors       BIGINT,
    server_errors       BIGINT,
    error_rate          DOUBLE,
    response_time_p50   INTEGER,             -- milliseconds
    response_time_p75   INTEGER,             -- milliseconds
    response_time_p90   INTEGER,             -- milliseconds
    response_time_p95   INTEGER,             -- milliseconds
    response_time_p99   INTEGER              -- milliseconds
);
```

Columns are only populated if included in `--metrics` or `--group-by` during fetch. No unique constraint; deduplication is handled by deleting existing rows for the time range being inserted.

## request_logs

```sql
CREATE TABLE request_logs (
    app_id                  INTEGER NOT NULL,
    timestamp               TIMESTAMPTZ NOT NULL,
    request_uuid            VARCHAR NOT NULL,
    env                     VARCHAR,            -- environment name, e.g. "prod"
    method                  VARCHAR NOT NULL,
    path                    VARCHAR,            -- parameterized route template, e.g. /users/{user_id}
    url                     VARCHAR NOT NULL,   -- full URL with actual path values, e.g. https://api.example.com/users/123
    consumer_id             INTEGER,            -- references consumers.consumer_id
    request_headers         STRUCT(name VARCHAR, value VARCHAR)[],
    request_size_bytes      BIGINT,
    request_body_json       JSON,               -- max 50 KB, null if too large
    status_code             INTEGER,
    response_time_ms        INTEGER,
    response_headers        STRUCT(name VARCHAR, value VARCHAR)[],
    response_size_bytes     BIGINT,
    response_body_json      JSON,               -- max 50 KB, null if too large
    client_ip               VARCHAR,
    client_country_iso_code VARCHAR,
    exception_type          VARCHAR,
    exception_message       VARCHAR,
    exception_stacktrace    VARCHAR,
    sentry_event_id         VARCHAR,
    trace_id                VARCHAR,            -- OpenTelemetry trace ID (hex)
    UNIQUE (app_id, request_uuid)
);
```

Columns are only populated if the corresponding field was included in the `--fields` flag during fetch.

## application_logs

```sql
CREATE TABLE application_logs (
    app_id       INTEGER NOT NULL,
    request_uuid VARCHAR NOT NULL,
    timestamp    TIMESTAMPTZ NOT NULL,
    message      VARCHAR NOT NULL,
    level        VARCHAR,
    logger       VARCHAR,
    file         VARCHAR,
    line         INTEGER
);
```

Populated by the `request-details` command when using `--db`.

## spans

```sql
CREATE TABLE spans (
    app_id         INTEGER NOT NULL,
    trace_id       VARCHAR NOT NULL,
    span_id        VARCHAR NOT NULL,
    parent_span_id VARCHAR,
    env            VARCHAR,
    name           VARCHAR,
    kind           VARCHAR,
    status         VARCHAR,
    start_time_ns  BIGINT NOT NULL,
    end_time_ns    BIGINT,
    duration_ns    BIGINT,
    attributes     JSON,
    events         JSON,
    scope_name     VARCHAR,
    scope_version  VARCHAR,
    UNIQUE (app_id, trace_id, span_id)
);
```

Both `traces --db` and `request-details --db` populate this shared table. Trace IDs are 32-character lowercase hex; span IDs are 16-character lowercase hex. Each command replaces the **complete row** for a matching `(app_id, trace_id, span_id)`, setting omitted optional columns to `NULL`. Unreturned spans remain, including when the response is empty.

Request-details takes `trace_id` from its enclosing response. Its span objects omit `env`, `events`, `scope_name`, and `scope_version`, so it clears those columns on replacement. The enclosing request's environment is not used as a per-span environment. All optional columns are nullable to support field selection, even if their selected API values are never null.

`start_time_ns` and `end_time_ns` are Unix epoch nanoseconds; `duration_ns` is a duration in nanoseconds. Use integer bounds for exact time comparisons and `duration_ns / 1000000.0` for milliseconds. Scope persistent queries by `app_id` and relevant trace IDs or start-time bounds.

`attributes` is a JSON object with native JSON values. `events` is a JSON array of objects with `timestamp`, `name`, and `attributes`. Event timestamps are ISO 8601 UTC strings with nanosecond precision. Selected empty collections are `{}` and `[]`; omitted fields are SQL `NULL`. See [attribute and event SQL examples](duckdb_json_functions.md#span-attributes-and-events).

Refetch older local span rows with JSON-encoded attribute strings to use these queries.

### Legacy span schema

Databases whose `spans` table has `request_uuid` require an explicit reset and refetch. The CLI reports an input error without changing records. With user permission, run `reset-db --db <same-path>` to clear **all tables**, then refetch. Alternatively, fetch into a new database file to retain the old one for inspection. Data beyond API retention may not be available to refetch. See [reset-db](commands.md#reset-db).

## Relationships

- `request_logs.consumer_id` references `consumers.consumer_id` (join on both `app_id` and `consumer_id`)
- `metrics.consumer_id` references `consumers.consumer_id` (join on both `app_id` and `consumer_id`, only when metrics are grouped by consumer_id)
- `endpoints.app_id` references `apps.app_id`
- `metrics.app_id` references `apps.app_id`
- `request_logs.app_id` references `apps.app_id`
- `app_envs.app_id` references `apps.app_id`
- `request_logs.env` matches `app_envs.name` (string, not a foreign key to `app_env_id`)
- `metrics.env` matches `app_envs.name` (string, only when metrics are grouped by env)
- `application_logs.request_uuid` references `request_logs.request_uuid` (join on both `app_id` and `request_uuid`)
- `spans.trace_id` matches `request_logs.trace_id` (join on both `app_id` and `trace_id`)
- `spans.app_id` references `apps.app_id`
- `spans.env` matches `app_envs.name`, scoped by `app_id`; populated only if the latest span fetch returned it
- `spans.parent_span_id` matches another span's `span_id`, scoped by both `app_id` and `trace_id`

Requests and traces are not one-to-one: multiple requests may share a trace, a trace has many spans, and spans may exist without request logs. A direct join can multiply request or span counts. For request counts, use `EXISTS` or deduplicate requests before counting.

For example, count requests associated with a stored error span without multiplying by the number of spans:

```sql
SELECT count(*) AS request_count
FROM request_logs r
WHERE r.app_id = 1
  AND r.trace_id = '0123456789abcdef0123456789abcdef'
  AND EXISTS (
      SELECT 1 FROM spans s
      WHERE s.app_id = r.app_id AND s.trace_id = r.trace_id
        AND s.status = 'ERROR'
  );
```

Find parent spans within the same trace:

```sql
SELECT s.span_id, s.name, p.span_id AS parent_span_id, p.name AS parent_name
FROM spans s
LEFT JOIN spans p
  ON p.app_id = s.app_id AND p.trace_id = s.trace_id
  AND p.span_id = s.parent_span_id
WHERE s.app_id = 1
  AND s.trace_id = '0123456789abcdef0123456789abcdef'
ORDER BY s.start_time_ns, s.span_id;
```
