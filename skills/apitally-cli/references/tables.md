# DuckDB Table Schemas

Tables are created automatically when using the `--db` flag with `apps`, `consumers`, `request-logs`, or `request-details` commands. DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).

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
    request_body_json       JSON,
    status_code             INTEGER,
    response_time_ms        INTEGER,
    response_headers        STRUCT(name VARCHAR, value VARCHAR)[],
    response_size_bytes     BIGINT,
    response_body_json      JSON,
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
    request_uuid   VARCHAR NOT NULL,
    span_id        VARCHAR NOT NULL,          -- OpenTelemetry span ID (hex)
    parent_span_id VARCHAR,
    name           VARCHAR NOT NULL,
    kind           VARCHAR NOT NULL,          -- e.g. SERVER, CLIENT, INTERNAL
    start_time_ns  BIGINT NOT NULL,           -- Unix epoch nanoseconds
    end_time_ns    BIGINT NOT NULL,           -- Unix epoch nanoseconds
    duration_ns    BIGINT NOT NULL,
    status         VARCHAR NOT NULL,          -- e.g. OK, ERROR, UNSET
    attributes     JSON
);
```

Populated by the `request-details` command when using `--db`.

## Relationships

- `request_logs.consumer_id` references `consumers.consumer_id` (join on both `app_id` and `consumer_id`)
- `request_logs.app_id` references `apps.app_id`
- `app_envs.app_id` references `apps.app_id`
- `request_logs.env` matches `app_envs.name` (string, not a foreign key to `app_env_id`)
- `application_logs.request_uuid` references `request_logs.request_uuid` (join on both `app_id` and `request_uuid`)
- `spans.request_uuid` references `request_logs.request_uuid` (join on both `app_id` and `request_uuid`)

## Special Types

### Headers (`STRUCT(name VARCHAR, value VARCHAR)[]`)

Headers are arrays of structs with `name` and `value` fields. Use DuckDB list comprehensions:

```sql
-- Extract a specific header value
[s.value FOR s IN request_headers IF lower(s.name) = 'content-type'][1]

-- Check if a header exists
len([s FOR s IN request_headers IF lower(s.name) = 'authorization']) > 0
```

### JSON body fields (`JSON`)

`request_body_json` and `response_body_json` are `JSON` columns. Use DuckDB JSON operators. Note: `->>` has low operator precedence, so always wrap in parentheses when used in WHERE clauses (after AND/OR).

```sql
-- Extract a string field
response_body_json->>'error'

-- Extract a nested field
response_body_json->'user'->>'email'

-- Extract from an array (0-indexed)
response_body_json->'items'->>0

-- Use in WHERE (parentheses required)
WHERE (response_body_json->>'status') = 'failed'
```
