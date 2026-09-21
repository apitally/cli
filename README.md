# Apitally CLI

[![Tests](https://github.com/apitally/cli/actions/workflows/tests.yaml/badge.svg?event=push)](https://github.com/apitally/cli/actions)
[![Codecov](https://codecov.io/gh/apitally/cli/graph/badge.svg?token=O3VWKH6DH9)](https://codecov.io/gh/apitally/cli)
[![Release](https://img.shields.io/github/v/release/apitally/cli?color=informational)](https://github.com/apitally/cli/releases/latest)
[![npm](https://img.shields.io/npm/v/@apitally/cli?logo=npm&color=%23cb0000)](https://www.npmjs.com/package/@apitally/cli)

A command-line interface for Apitally, built for agents and humans.

Apitally is a simple API monitoring and analytics tool that makes it easy to understand API usage, monitor performance, and troubleshoot issues.

Learn more about Apitally on our 🌎 [website](https://apitally.io) or check out
the 📚 [documentation](https://docs.apitally.io).

## Highlights

- Retrieve API metrics and request logs, including headers and payloads
- Discover trace spans by duration, status, attributes, and events
- Drill into individual requests with application logs and associated spans
- Load data into a local [DuckDB](https://github.com/duckdb/duckdb) database and analyze it with arbitrary SQL queries
- Single Rust binary with bundled DuckDB, no runtime dependencies
- Includes an [agent skill](skills/apitally-cli/), so agents know how to use the CLI effectively out of the box

## Installation

### For agents

Install the `apitally-cli` skill using the [skills CLI](https://github.com/vercel-labs/skills):

```bash
npx skills add apitally/cli
```

### For humans

The CLI can be used with `npx`, no installation required:

```shell
npx @apitally/cli <command>
```

If you wish to use the `apitally` binary directly, install it with the standalone installer script:

```shell
# On macOS and Linux
curl -fsSL https://apitally.io/cli/install.sh | sh
```

```shell
# On Windows
powershell -ExecutionPolicy Bypass -c "irm https://apitally.io/cli/install.ps1 | iex"
```

You can also download the binary for your platform from the [latest release](https://github.com/apitally/cli/releases/latest) on GitHub.

## Authentication

Run the `auth` command to authenticate the CLI:

```bash
npx @apitally/cli auth
```

This opens a browser-based auth flow where you log in to the Apitally dashboard and select a team. A newly created API key is then passed back to the CLI and saved to `~/.apitally/auth.json`.

If you already have an API key, you can provide it directly:

```bash
npx @apitally/cli auth --api-key "your-api-key"
```

You can also set the API key via the `APITALLY_API_KEY` environment variable or pass the `--api-key` flag to any command.

## Commands

| Command           | Description                                     |
| ----------------- | ----------------------------------------------- |
| `auth`            | Configure API key                               |
| `whoami`          | Check authentication and show team info         |
| `apps`            | List all apps in your team                      |
| `consumers`       | List consumers for an app                       |
| `endpoints`       | List endpoints for an app                       |
| `metrics`         | Fetch aggregated metrics for an app             |
| `request-logs`    | Fetch request log data for an app               |
| `request-details` | Fetch full details for a specific request       |
| `traces`          | Fetch trace spans for an app                    |
| `sql`             | Run SQL queries against a local DuckDB database |
| `reset-db`        | Drop and recreate all tables in local DuckDB    |

List results and SQL queries output NDJSON to stdout. Use the `--db` flag on data-fetching commands to write to a local DuckDB database instead, then query it with `sql`. The database defaults to `~/.apitally/data.duckdb` if no other path is specified.

`traces --db` and `request-details --db` share the `spans` table, keyed by app, trace, and span IDs. Refetching replaces complete matching rows, clearing omitted fields. Existing databases with the old request-based span schema require an explicit reset and refetch: `reset-db` clears **all tables**. Use a new database file instead to preserve the old data; data beyond API retention may not be available to refetch.

For detailed usage, run `npx @apitally/cli <command> --help`.

For a full command reference, see [skills/apitally-cli/references/commands.md](skills/apitally-cli/references/commands.md).

For DuckDB table schemas, see [skills/apitally-cli/references/duckdb_tables.md](skills/apitally-cli/references/duckdb_tables.md).

## Exit codes

| Code | Meaning                                                 |
| ---- | ------------------------------------------------------- |
| 0    | Success                                                 |
| 1    | General / unknown error                                 |
| 2    | Usage error (invalid arguments, missing required flags) |
| 3    | Authentication error (missing or invalid API key)       |
| 4    | Input error (invalid argument values)                   |
| 5    | API / network error                                     |

## Getting help

If you need help please
[create a new discussion](https://github.com/orgs/apitally/discussions/categories/q-a)
on GitHub or email us at [support@apitally.io](mailto:support@apitally.io). We'll get back to you as soon as possible.
