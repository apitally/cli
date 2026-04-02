# AGENTS.md

## Product Overview

[Apitally](https://apitally.io) is an API monitoring and analytics tool. This is a CLI tool for AI agents and humans. It retrieves data from the Apitally API and outputs it in NDJSON format, or optionally stores it in a local DuckDB database and allows running arbitrary SQL queries against it.

## Repository Structure

```
src/
  main.rs               Entry point, CLI argument parsing (clap), command dispatch
  auth.rs               Authentication config (load/save/resolve) + auth command
  whoami.rs             Whoami command (auth check, team info)
  apps.rs               Apps command (fetch, DB write)
  consumers.rs          Consumers command (paginated fetch, DB write)
  endpoints.rs          Endpoints command (fetch, DB write)
  request_logs.rs       Request logs command (Arrow IPC or NDJSON streaming)
  request_details.rs    Request details command (single request fetch, DB write)
  sql.rs                SQL command (query DuckDB, output NDJSON)
  utils.rs              Shared helpers (open DuckDB connection, check HTTP response)
npm/
  cli.js                Thin wrapper that spawns the native binary
  install.js            postinstall script that downloads the correct binary
skills/
  apitally-cli/         Agent skill: guides AI agents through using the CLI to investigate API issues
    SKILL.md            Workflow, command reference, investigation patterns, SQL examples
    references/         Detailed command and table schema references
```

## Tech Stack

- **Language**: Rust (single static binary, no runtime dependencies)
- **CLI parsing**: clap (derive API)
- **HTTP client**: ureq (blocking; response body implements `Read` for streaming)
- **Serialization**: serde + serde_json
- **Error handling**: anyhow
- **Embedded database**: duckdb (statically linked, bundled Arrow via `duckdb::arrow::*`)
- **Distribution**: npm package (`@apitally/cli`); postinstall downloads the native binary

## CLI Subcommands

| Subcommand        | Data source                                         |
| ----------------- | --------------------------------------------------- |
| `auth`            | —                                                   |
| `whoami`          | `GET /v1/team`                                      |
| `apps`            | `GET /v1/apps`                                      |
| `consumers`       | `GET /v1/apps/{app_id}/consumers`                   |
| `endpoints`       | `GET /v1/apps/{app_id}/endpoints`                   |
| `request-logs`    | `POST /v1/apps/{app_id}/request-logs`               |
| `request-details` | `GET /v1/apps/{app_id}/request-logs/{request_uuid}` |
| `sql`             | Local DuckDB                                        |

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

## Distribution

The CLI is published to npm as `@apitally/cli`. The npm package contains no Rust code — its `postinstall` script (`npm/install.js`) downloads the pre-built binary for the user's platform from the GitHub release. `npm/cli.js` is a thin Node wrapper that spawns the native binary.

### Release workflow (`.github/workflows/release.yaml`)

Triggered by publishing a GitHub release. Two jobs:

1. **build** — cross-compiles for 6 targets (linux x64/arm64, macOS x64/arm64, Windows x64/arm64), packages as `.tar.gz`/`.zip`, uploads as artifacts.
2. **publish** — attaches artifacts to the GitHub release, then publishes to npm (version derived from the git tag; pre-releases use the `next` dist-tag).

## Architecture Patterns

### Module `run()` functions

Each command module exposes a `pub fn run(...)` that does all the work: resolve auth, call the API, write output. The last parameter is `impl Write` (stdout in production, `Vec<u8>` in tests), except `auth::run` which takes `&mut impl Read` (stdin) instead. Data goes to the writer (stdout), human messages and progress go to stderr.

### Error handling and exit codes

`CliError` in `utils.rs` has three variants with dedicated constructors and exit codes:

| Constructor   | Exit code | When to use                       |
| ------------- | --------- | --------------------------------- |
| `auth_err()`  | 3         | Missing/invalid API key, 401/403  |
| `input_err()` | 4         | Bad user input, 400/404/422       |
| `api_err()`   | 5         | Server errors, transport failures |

Exit code 2 comes from clap (usage errors). `check_response` in `utils.rs` centralizes HTTP status-to-error mapping.

### HTTP conventions

API requests use `api_get`/`api_post` helpers in `utils.rs`. API key is sent as the `Api-Key` header.

## Tests

Inline `#[cfg(test)] mod tests` in each source file. Run with `cargo test`.

- **Few, coarse-grained tests**: each command module typically has just two tests — `test_run_ndjson` and `test_run_with_db` — that call the module's `run()` function end-to-end with a mock HTTP server. Not many small unit tests.
- **HTTP mocking**: `mockito` — tests register endpoints with canned responses and assert they were called.
- **Output capture**: `run()` functions accept `impl Write`; tests pass a `Vec<u8>` buffer and assert on the NDJSON output. DB tests query the written rows instead.
- **Shared helpers** in `utils::test_utils`: `temp_db()` (temp dir + DB path), `parse_ndjson()` (deserialize NDJSON from buffer).
- **CI** (`.github/workflows/tests.yaml`): `cargo fmt --check`, `cargo clippy`, `cargo llvm-cov`.

## Conventions

- Prefer simple, concise code; avoid unnecessary abstractions
- Use `anyhow::Result` for error handling in application code
- Use `duckdb::arrow::*` re-exports for all Arrow type imports (ensures type compatibility with duckdb). The standalone `arrow` and `arrow-ipc` crates in `Cargo.toml` exist solely to enable features (`ipc`, `json`, `chrono-tz` on `arrow`; `lz4` on `arrow-ipc`) that duckdb doesn't activate on its bundled Arrow. Cargo unifies both into a single Arrow instance. When updating `duckdb`, check that its Arrow version still matches and bump the `arrow`/`arrow-ipc` versions in `Cargo.toml` if needed (`cargo tree -p duckdb --depth 1`).
- Follow existing code patterns and conventions
- Verify work compiles and passes `cargo clippy` before considering a task complete
- Keep user-facing output concise
- Use inline comments sparingly — only to explain non-obvious intent
- When changing CLI commands, flags, output format, or DuckDB schemas, update the agent skill in `skills/apitally-cli/` (including its `references/` docs) to reflect those changes
- Keep `README.md` and `AGENTS.md` up to date with the latest changes
