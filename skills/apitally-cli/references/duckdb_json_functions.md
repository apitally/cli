# DuckDB JSON Functions

Functions for querying request/response bodies and span `attributes`/`events` columns (DuckDB `JSON` type).

## Extraction Operators

| Function                          | Operator | Returns   | Description                                                                      |
| :-------------------------------- | :------- | :-------- | :------------------------------------------------------------------------------- |
| `json_exists(json, path)`         |          | `BOOLEAN` | Returns `true` if the path exists in the JSON, `false` otherwise                 |
| `json_extract(json, path)`        | `->`     | `JSON`    | Extract JSON at the given path. If path is a `LIST`, returns a `LIST` of JSON    |
| `json_extract_string(json, path)` | `->>`    | `VARCHAR` | Extract text at the given path. If path is a `LIST`, returns a `LIST` of VARCHAR |
| `json_value(json, path)`          |          | `JSON`    | Extract JSON at the given path. Returns `NULL` if the value is not a scalar      |

Use `->>` when you want a string result (most common). Use `->` when you need to chain further extraction or preserve the JSON type.

## JSONPath Syntax

Paths start with `$` and use dot notation for object keys, brackets for array indices:

| Pattern          | Meaning                             |
| :--------------- | :---------------------------------- |
| `$.field`        | Top-level field                     |
| `$.nested.field` | Nested object field                 |
| `$.items[0]`     | First element of an array (0-based) |
| `$.items[#-1]`   | Last element of an array            |
| `$.items[*]`     | All elements of an array            |

## Operator Precedence

`->` and `->>` have low precedence. **Always wrap in parentheses** when combined with `AND`, `OR`, or comparison operators:

```sql
-- Correct
WHERE app_id = 123 AND (response_body_json->>'$.status') = 'failed'

-- Wrong — parsed incorrectly due to AND before ->>
WHERE app_id = 123 AND response_body_json->>'$.status' = 'failed'
```

## Examples

```sql
-- Extract a top-level string field
SELECT response_body_json->>'$.error' AS error_message
FROM request_logs
WHERE app_id = 123 AND timestamp >= '2025-01-01'

-- Extract a nested field
SELECT response_body_json->'$.user'->>'$.email' AS user_email
FROM request_logs
WHERE app_id = 123 AND timestamp >= '2025-01-01'

-- Extract from an array by index (0-based)
SELECT response_body_json->'$.items'->>0 AS first_item
FROM request_logs
WHERE app_id = 123 AND timestamp >= '2025-01-01'

-- Filter on a JSON field in WHERE (parentheses required)
SELECT method, path, status_code
FROM request_logs
WHERE app_id = 123
  AND timestamp >= '2025-01-01'
  AND (response_body_json->>'$.error_code') = 'RATE_LIMITED'

-- Extract multiple fields from response body
SELECT timestamp, method, path,
       response_body_json->>'$.error' AS error,
       response_body_json->>'$.message' AS message
FROM request_logs
WHERE app_id = 123
  AND timestamp >= '2025-01-01'
  AND status_code >= 400
```

## Span Attributes and Events

Both `traces` and `request-details` preserve raw attribute maps. For example:

```json
{"db.system":"\"postgresql\"","retry.count":"2","cached":"true"}
```

Decode two layers: extract the map's string value, cast that string to JSON, then extract its scalar at `$`. Quote keys containing literal dots as a single JSONPath key (`$."db.system"`), not nested fields. These examples assume app `1` and the shown trace ID were fetched with `attributes` and `events` selected. Substitute your investigation's IDs.

```sql
SELECT span_id,
       json_extract_string(attributes, '$."db.system"') AS encoded_db_system,
       json_extract_string(
           json_extract_string(attributes, '$."db.system"')::JSON, '$'
       ) AS db_system,
       json_extract_string(
           json_extract_string(attributes, '$."retry.count"')::JSON, '$'
       )::BIGINT AS retry_count,
       json_extract_string(
           json_extract_string(attributes, '$.cached')::JSON, '$'
       )::BOOLEAN AS cached
FROM spans
WHERE app_id = 1
  AND trace_id = '0123456789abcdef0123456789abcdef';
```

`encoded_db_system` contains `"postgresql"` including the quotes; `db_system` is `postgresql`. `retry_count` is the integer `2` and `cached` is boolean `true`. An omitted attributes column is SQL `NULL`; a selected empty map is `{}`.

Expand the events array with `json_each` and decode event attribute values the same way:

```sql
SELECT s.span_id,
       event.value->>'$.timestamp' AS event_timestamp_utc,
       event.value->>'$.name' AS event_name,
       json_extract_string(
           json_extract_string(event.value, '$.attributes."exception.type"')::JSON, '$'
       ) AS exception_type
FROM spans s, json_each(s.events) AS event
WHERE s.app_id = 1
  AND s.trace_id = '0123456789abcdef0123456789abcdef'
  AND (event.value->>'$.name') = 'exception';
```

Event timestamp strings represent UTC and retain nanosecond digits. Database JSON may use a space instead of `T` and omit `Z`, unlike API NDJSON; do not infer local time from that formatting. Keep the string to preserve all digits; casting to `TIMESTAMPTZ` can lose sub-microsecond precision. Use integer `start_time_ns`/`end_time_ns` for exact span-time comparisons.

## Scalar Functions

| Function                                    | Description                                                                                                                                                                                                        |
| :------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `json_array_length(json[, path])`           | Number of elements in a JSON array. Returns `0` if not an array.                                                                                                                                                   |
| `json_contains(json_haystack, json_needle)` | Returns `true` if `json_needle` is contained in `json_haystack`. Both parameters are of JSON type, but `json_needle` can also be a numeric value or a string, however the string must be wrapped in double quotes. |
| `json_keys(json[, path])`                   | Keys of a JSON object as `LIST` of `VARCHAR`. Useful for exploring unknown shapes.                                                                                                                                 |
| `json_type(json[, path])`                   | Type of the JSON value: `ARRAY`, `OBJECT`, `VARCHAR`, `BIGINT`, `UBIGINT`, `DOUBLE`, `BOOLEAN`, `NULL`.                                                                                                            |

```sql
-- Filter responses that return arrays with more than 100 items
SELECT timestamp, method, path,
       json_array_length(response_body_json, '$.items') AS item_count
FROM request_logs
WHERE app_id = 123
  AND timestamp >= '2025-01-01'
  AND json_array_length(response_body_json, '$.items') > 100

-- Discover the top-level keys in response bodies
SELECT json_keys(response_body_json) AS keys, COUNT(*) AS n
FROM request_logs
WHERE app_id = 123 AND timestamp >= '2025-01-01'
  AND response_body_json IS NOT NULL
GROUP BY keys ORDER BY n DESC
LIMIT 10
```

## Unnesting JSON Arrays with `json_each`

`json_each(json, path)` is a table function that expands a JSON array into one row per element. Use it to analyze individual items inside arrays.

Each row has columns: `key` (array index), `value` (element as JSON), `type` (element type).

```sql
-- Unnest items from a response body like {"items": [{"id": 1, ...}, {"id": 2, ...}]}
SELECT r.timestamp, r.method, r.path,
       item.value->>'$.id' AS item_id,
       item.value->>'$.status' AS item_status
FROM request_logs r, json_each(r.response_body_json, '$.items') AS item
WHERE r.app_id = 123
  AND r.timestamp >= '2025-01-01'
  AND r.response_body_json IS NOT NULL

-- Count how many array items match a condition per request
SELECT r.request_uuid, r.method, r.path,
       COUNT(*) AS total_items,
       SUM(CASE WHEN (item.value->>'$.status') = 'failed' THEN 1 ELSE 0 END) AS failed_items
FROM request_logs r, json_each(r.response_body_json, '$.items') AS item
WHERE r.app_id = 123
  AND r.timestamp >= '2025-01-01'
  AND r.response_body_json IS NOT NULL
GROUP BY r.request_uuid, r.method, r.path
HAVING failed_items > 0
ORDER BY failed_items DESC
```
