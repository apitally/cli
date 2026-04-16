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

- Retrieve API metrics and request logs, including headers, payloads, traces, and more
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

To use the CLI, you need an API key. You can create one in the [Apitally dashboard](https://app.apitally.io/settings/api-keys) under _Settings → API keys_.

Then run the `auth` command to configure your API key interactively:

```bash
npx @apitally/cli auth
```

Or provide the key directly:

```bash
npx @apitally/cli auth --api-key "your-api-key"
```

The API key is saved to `~/.apitally/auth.json`.

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
| `sql`             | Run SQL queries against a local DuckDB database |
| `reset-db`        | Drop and recreate all tables in local DuckDB    |

All commands output NDJSON to stdout by default. Use the `--db` flag to write data to a local DuckDB database instead, which can then be queried with the `sql` command. The database defaults to `~/.apitally/data.duckdb` if no other path is specified.

Run `npx @apitally/cli --help` for detailed usage information.

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
