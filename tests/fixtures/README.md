# Trace Arrow fixtures

Captured on 2026-09-21 from the actual cloud endpoints using ClickHouse
25.8.16.34 (`clickhouse/clickhouse-server:25.8.16`). These are unmodified
HTTP response bytes, not IPC constructed with an Arrow library.

Cloud source Git blobs:
- `apitally_cloud/api/traces.py`: `56724af2668f3eed9dced860e12844dd878c6e34`
- `apitally_cloud/api/request_logs.py`: `a83373bfd5841b17ee6ac858815981cbdc0640fa`

The isolated ClickHouse tables use the cloud table definitions unchanged.
Synthetic rows use app 1 and retention_days 3650. An httpx ASGI client calls
the cloud FastAPI application. Only authentication, the PostgreSQL session
dependency, and rate limiting are replaced; ClickHouse queries and response
streaming are real. No production configuration, credentials, or data are used.
Arrow compression is ClickHouse's default `lz4_frame`.

| File | Selection | Rows |
| --- | --- | --- |
| `traces-all.arrow` | All 14 fields in `FieldName` order | 3 |
| `traces-default.arrow` | Omit `fields` | 3 |
| `traces-required.arrow` | `fields: []` | 3 |
| `traces-empty.arrow` | All fields plus `name eq missing` | 0 |

`traces-all.ndjson` is the matching all-fields NDJSON response.
`request-details-traces.json` is the linked request-details response with
`include_consumer_id=true`, used to test writes from both commands.

## Observed schema

- Non-nullable UTF-8 strings: trace_id, span_id, name, kind, status, attributes, events.
- Nullable UTF-8 strings: parent_span_id, env, scope_name, scope_version.
- Non-nullable signed int64: start_time_ns, end_time_ns, duration_ns.
- Attributes and events contain complete JSON documents, imported directly into
  DuckDB JSON columns. Attribute values retain their native JSON types.
- Event timestamps are ISO 8601 UTC strings retaining nanoseconds, including
  `2026-01-01T00:00:00.123456001Z`. There are no Arrow timestamp fields.
- No custom schema or field metadata. Empty output has a schema and no batches.

## Synthetic values

The first trace, `0123456789abcdef0123456789abcdef`, contains span 1
(`GET /books`, SERVER, UNSET) and its child span 2 (`db.query`, CLIENT, ERROR).
The second trace, `fedcba9876543210fedcba9876543210`, contains span 3
(`background`, INTERNAL, OK). Start times are 1767225600123456001,
1767225600123457001, and 1767225600123458001; durations are 250000000,
200000000, and 1001 ns. Span 3 has null parent/environment/scope and empty
attributes/events.

Span 1 and its exception event exercise nested objects with literal dotted
keys, mixed-type arrays, explicit nulls, quotes, backslashes, and newlines.
Span 2 has `db.system: "postgresql"`, `retry.count: 2`, and `cached: false`.
Its exception event has `exception.type: "TimeoutError"`. The NDJSON fixture
records every value; the linked request-details response has the same span
attributes.

## Refreshing fixtures

Use an isolated ClickHouse instance and the cloud API test setup (see
`cloud/tests/api/test_traces.py`). Seed the three spans from `traces-all.ndjson`,
JSON-encoding each attribute value for the stored `Map(String, String)` fields.
Preserve integer nanosecond timestamps. Seed a request log in app 1 with UUID
`11111111-2222-4333-8444-555555555555` linked to the first trace.

Send `POST /v1/apps/1/traces` with this body, adding each selection from the
file table above, and save the raw HTTP response bytes:

```json
{"format":"arrow","filters":[{"field":"trace_id","op":"in","value":["0123456789abcdef0123456789abcdef","fedcba9876543210fedcba9876543210"]}]}
```

Use `format: ndjson` for the all-fields counterpart. Fetch the linked request
from `GET /v1/apps/1/request-logs/11111111-2222-4333-8444-555555555555?include_consumer_id=true`.
Verify decoded JSON equality across the responses, including event timestamp
precision. Running the CLI tests requires neither ClickHouse nor Python.
