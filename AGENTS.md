# AGENTS.md

## Product Overview

[Apitally](https://apitally.io) is an API monitoring and analytics tool. This is a CLI tool for AI agents. It retrieves data from the Apitally API and outputs it in NDJSON format, or optionally stores it in a local DuckDB database and allows running arbitrary SQL queries against it.

## Repository Structure

```
src/
  main.rs             Entry point, CLI argument parsing (clap), command dispatch
  auth.rs             Authentication config (load/save/resolve) + auth command
  apps.rs             Apps command (fetch, DB write)
  consumers.rs        Consumers command (paginated fetch, DB write)
  request_logs.rs     Request logs command (Arrow IPC or NDJSON streaming)
  sql.rs              SQL command (query DuckDB, output NDJSON)
  utils.rs            Shared helpers (open DuckDB connection, check HTTP response)
Cargo.toml            Dependencies and package metadata
```

## Tech Stack

- **Language**: Rust (single static binary, no runtime dependencies)
- **CLI parsing**: clap (derive API)
- **HTTP client**: ureq (blocking; response body implements `Read` for streaming)
- **Serialization**: serde + serde_json
- **Error handling**: anyhow
- **Embedded database**: duckdb (statically linked, bundled Arrow via `duckdb::arrow::*`)

## CLI Subcommands

| Subcommand     | API Endpoint                                 | Format             |
| -------------- | -------------------------------------------- | ------------------ |
| `auth`         | —                                            | —                  |
| `apps`         | `GET /v1/apps`                               | JSON               |
| `consumers`    | `GET /v1/apps/{app_id}/consumers`            | JSON               |
| `request-logs` | `POST /v1/apps/{app_id}/request-logs/stream` | Arrow IPC / NDJSON |
| `sql`          | Local DuckDB                                 | NDJSON             |

## Authentication with API

API key is resolved with the following precedence:

1. `--api-key` CLI flag
2. `APITALLY_API_KEY` environment variable
3. `~/.apitally/auth.json` (written by `apitally auth` command)

If no API key is found, the CLI exits with an error prompting the user to run `apitally auth` or provide `--api-key`.

The API base URL follows the same pattern: `--api-base-url` flag > `APITALLY_API_BASE_URL` env var > `auth.json` > default (`https://api.apitally.io`).

## Common Commands

```bash
cargo build              # Build (debug)
cargo build --release    # Build (release, optimized)
cargo run -- <args>      # Run with arguments
cargo test               # Run tests
cargo clippy             # Lint
cargo fmt                # Format code
```

## Conventions

- Prefer simple, concise code; avoid unnecessary abstractions
- Use `anyhow::Result` for error handling in application code
- Use `duckdb::arrow::*` re-exports for all Arrow type imports (ensures type compatibility with duckdb). The standalone `arrow` and `arrow-ipc` crates in `Cargo.toml` exist solely to enable features (`ipc`, `json`, `chrono-tz` on `arrow`; `lz4` on `arrow-ipc`) that duckdb doesn't activate on its bundled Arrow. Cargo unifies both into a single Arrow instance. When updating `duckdb`, check that its Arrow version still matches and bump the `arrow`/`arrow-ipc` versions in `Cargo.toml` if needed (`cargo tree -p duckdb --depth 1`).
- Follow existing code patterns and conventions
- Verify work compiles and passes `cargo clippy` before considering a task complete
- Keep user-facing output concise
- Use inline comments sparingly — only to explain non-obvious intent
