# Apitally CLI

[![Tests](https://github.com/apitally/cli/actions/workflows/tests.yaml/badge.svg?event=push)](https://github.com/apitally/cli/actions)
[![Codecov](https://codecov.io/gh/apitally/cli/graph/badge.svg?token=O3VWKH6DH9)](https://codecov.io/gh/apitally/cli)

A command-line tool for [Apitally](https://apitally.io), built for agents.

## Authentication

Run the `auth` command to configure your API key interactively:

```bash
apitally auth
```

Or provide the key directly:

```bash
apitally auth --api-key "your-api-key"
```

The API key is saved to `~/.apitally/auth.json`.

You can also set the API key via the `APITALLY_API_KEY` environment variable or pass the `--api-key` flag to any command.

## Usage

```bash
apitally apps
apitally consumers --app-id 123
apitally request-logs --app-id 123 --since "2026-01-01T00:00:00Z" --limit 100
apitally sql "SELECT * FROM request_logs LIMIT 10" --db /path/to/db.duckdb
```

See `apitally --help` for all options.
