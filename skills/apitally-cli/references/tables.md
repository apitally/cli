# DuckDB Table Schemas

Tables are created automatically when using the `--db` flag with `apps`, `consumers`, or `request-logs` commands. DuckDB uses a [PostgreSQL-compatible SQL dialect](https://duckdb.org/docs/stable/sql/dialect/overview).

## apps

```sql
CREATE TABLE apps (
    app_id          INTEGER NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    framework       TEXT NOT NULL,
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
    identifier      TEXT NOT NULL,
    name            TEXT NOT NULL,
    "group"         TEXT,
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
    app_env                 VARCHAR,
    method                  VARCHAR NOT NULL,
    path                    VARCHAR,
    url                     VARCHAR NOT NULL,
    consumer_id             INTEGER,
    request_headers         STRUCT("1" VARCHAR, "2" VARCHAR)[],
    request_size            BIGINT,
    request_body_json       JSON,
    status_code             INTEGER,
    response_time_ms        INTEGER,
    response_headers        STRUCT("1" VARCHAR, "2" VARCHAR)[],
    response_size           BIGINT,
    response_body_json      JSON,
    client_ip               VARCHAR,
    client_country_iso_code VARCHAR,
    exception_type          VARCHAR,
    exception_message       VARCHAR,
    exception_stacktrace    VARCHAR,
    sentry_event_id         VARCHAR,
    trace_id                VARCHAR,
    UNIQUE (app_id, request_uuid)
);
```

Columns are only populated if the corresponding field was included in the `--fields` flag during fetch.

## Relationships

- `request_logs.consumer_id` references `consumers.consumer_id` (join on both `app_id` and `consumer_id`)
- `request_logs.app_id` references `apps.app_id`
- `app_envs.app_id` references `apps.app_id`
- `request_logs.app_env` matches `app_envs.name` (string, not a foreign key to `app_env_id`)

## Special Types

### Headers (`STRUCT("1" VARCHAR, "2" VARCHAR)[]`)

Headers are arrays of structs where `"1"` is the header name and `"2"` is the header value. Use DuckDB list comprehensions:

```sql
-- Extract a specific header value
[s."2" FOR s IN request_headers IF lower(s."1") = 'content-type'][1]

-- Check if a header exists
len([s FOR s IN request_headers IF lower(s."1") = 'authorization']) > 0
```

### JSON body fields (`JSON`)

`request_body_json` and `response_body_json` are JSON strings. Use DuckDB JSON operators:

```sql
-- Extract a string field
response_body_json::JSON->>'error'

-- Extract a nested field
request_body_json::JSON->'user'->>'email'

-- Use in WHERE
WHERE response_body_json::JSON->>'status' = 'failed'
```
