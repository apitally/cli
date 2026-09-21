# Trace Arrow fixtures

Captured on 2026-09-21 from the real cloud `get_traces_stream` endpoint at
cloud commit `19e9fcc290a8fb3640d6389ab90dd546c7f4a844` using ClickHouse
25.8.16.34 (`clickhouse/clickhouse-server:25.8.16`). These are unmodified
HTTP response bytes, not IPC constructed with an Arrow library.

The isolated ClickHouse table uses `cloud/clickhouse/table_spans.sql` unchanged.
Synthetic rows use app 1 and retention_days 3650. An httpx ASGI client calls
`POST /v1/apps/1/traces` on the cloud FastAPI application. Only authentication,
the PostgreSQL session dependency, and rate limiting are replaced. The session
recognizes synthetic app 1/team 1; queries and response streaming use the real
cloud ClickHouse engine. No production configuration, credentials, or data are
used. Arrow compression is ClickHouse's default `lz4_frame`.

| File | Selection | Rows |
| --- | --- | --- |
| `traces-all.arrow` | All 14 fields in `FieldName` order | 3 |
| `traces-default.arrow` | Omit `fields` | 3 |
| `traces-required.arrow` | `fields: []` | 3 |
| `traces-empty.arrow` | All fields plus `name eq missing` | 0 |

`traces-all.ndjson` is the matching unmodified all-fields NDJSON response.
`request-details-traces.json` is the linked request-details response with
`include_consumer_id=true`, used to test writes from both commands.

Each Arrow request specifies `format: arrow` and a positive trace-ID `in` filter for
`0123456789abcdef0123456789abcdef` and
`fedcba9876543210fedcba9876543210`, with no time bounds. Required fields appear
first: trace_id, span_id, start_time_ns. Empty output contains a schema and no
record batches.

## Observed schema

- Non-nullable UTF-8 strings: trace_id, span_id, name, kind, status.
- Nullable UTF-8 strings: parent_span_id, env, scope_name, scope_version.
- Non-nullable signed int64: start_time_ns, end_time_ns, duration_ns.
- Non-nullable attributes: `map<string, string>`. The child `entries` struct
  and its `key` are non-nullable; `value` is nullable.
- Non-nullable events: list with a nullable child named `item`, containing a
  struct with `timestamp`, `name`, and `attributes` fields. All three fields
  are non-nullable. Timestamp is **timestamp[ns, tz=UTC]**. Name is a UTF-8
  string. Event attributes have the same map schema as span attributes.
- No schema-level or field-level custom metadata. The UTC annotation belongs
  to the timestamp data type.

## Synthetic values

The first trace contains span 1 (`GET /books`, SERVER, UNSET) and its child
span 2 (`db.query`, CLIENT, ERROR). The second contains span 3 (`background`,
INTERNAL, OK). Start times are respectively 1767225600123456001,
1767225600123457001, and 1767225600123458001. Durations are 250000000,
200000000, and 1001 ns. The first two spans have environment `prod` and scope
`test.instrumentation` / `1.2.3`. Span 3 has null parent/environment/scope,
empty attributes, and empty events.

Span 1 attributes are `http.route: "\"/books\""`, `retry.count: "2"`, and
`cached: "true"`. It has an `exception` event at
`2026-01-01T00:00:00.123456001Z` with `exception.type: "\"TimeoutError\""`,
`code: "503"`, and `retryable: "true"`, then a `log` event at
`2026-01-01T00:00:00.123456010Z` with an empty attributes map.

Span 2 attributes are `db.system: "\"postgresql\""`, `retry.count: "2"`, and
`cached: "false"`. Its `exception` event at
`2026-01-01T00:00:00.123458000Z` has `exception.type: "\"TimeoutError\""`.
Raw JSON-encoded attribute values stay strings. Selected empty collections
are valid empty maps/lists, not nulls.

## Refreshing fixtures

Use an isolated ClickHouse instance and the cloud API test setup (see
`cloud/tests/api/test_traces.py`). Seed the three spans described above using
integer nanosecond timestamps, plus a request log in app 1 with UUID
`11111111-2222-4333-8444-555555555555` linked to the first trace. Use the
source table definitions, synthetic authentication, and the actual endpoint.
Never regenerate these files with an Arrow writer: they exercise the real
ClickHouse wire format, including compression and nested field metadata.

Send `POST /v1/apps/1/traces` with this body, adding each selection from the
file table above, and save the raw HTTP response bytes:

```json
{"format":"arrow","filters":[{"field":"trace_id","op":"in","value":["0123456789abcdef0123456789abcdef","fedcba9876543210fedcba9876543210"]}]}
```

Use `format: ndjson` for the all-fields NDJSON counterpart. Fetch the linked
request from `GET /v1/apps/1/request-logs/11111111-2222-4333-8444-555555555555?include_consumer_id=true`.
Inspect the actual Arrow schema when refreshing; the Rust import must retain
the timestamp precision and raw attribute strings shown above. Running the
CLI tests requires neither ClickHouse nor Python.
