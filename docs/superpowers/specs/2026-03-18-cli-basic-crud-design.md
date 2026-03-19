# CLI Basic CRUD Design

Date: 2026-03-18
Status: Draft approved in chat

## 1. Scope

Add basic CRUD-focused CLI commands in `cli` using `clap` structs/enums first, then handler implementation.

In scope:

- Categories: create, list, delete.
- Assets: create, get, list, update metadata, delete.
- Keep existing scripted flow command intact.

Out of scope:

- Tag value direct mutation commands.
- Category rename/move commands.
- Bulk operations.
- CRUD for tag definitions, external entities, or event types.

## 2. Command Surface

Global flags remain shared across all commands:

- `--base-url <URL>` (default `http://127.0.0.1:3000`)
- `--output <json|human>` (default `json`)

Top-level subcommands:

- `flow scripted-core` (existing behavior)
- `category <subcommand>`
- `asset <subcommand>`

Category subcommands:

- `category create --name <NAME> [--parent-id <ID>]`
- `category list`
- `category delete --id <ID>`

Asset subcommands:

- `asset create --category-id <ID> [--asset-tag <TAG>]`
- `asset get --id <ID> [--include-deleted]`
- `asset list [--include-deleted]`
- `asset update --id <ID> [--display-name <NAME>] [--clear-display-name]`
- `asset delete --id <ID>`

## 3. Clap-First Modeling

CLI argument shape is encoded with typed `clap` enums/structs before request logic.

Design:

- Extend `CliCommand` with `Category` and `Asset` variants.
- Add nested enums:
  - `CategoryCommand::{Create, List, Delete}`
  - `AssetCommand::{Create, Get, List, Update, Delete}`
- Add per-operation arg structs, for example:
  - `CategoryCreateArgs`, `CategoryDeleteArgs`
  - `AssetCreateArgs`, `AssetGetArgs`, `AssetListArgs`, `AssetUpdateArgs`, `AssetDeleteArgs`

This keeps parsing, required/optional fields, and operation intent explicit in type definitions.

## 4. HTTP Mapping

Each command maps directly to existing server endpoints.

- `category create` -> `POST /categories` with `{ name, parent_category_id? }`
- `category list` -> `GET /categories`
- `category delete` -> `DELETE /categories/{id}`
- `asset create` -> `POST /assets` with `{ category_id, asset_tag? }`
- `asset get` -> `GET /assets/{id}` or `GET /assets/{id}?include_deleted=true`
- `asset list` -> `GET /assets` or `GET /assets?include_deleted=true`
- `asset update` -> `PATCH /assets/{id}` with `{ display_name?, clear_display_name? }`
- `asset delete` -> `DELETE /assets/{id}`

The CLI is a thin API client: server-side rules remain authoritative.

## 5. Validation and Error Handling

Client-side validation (minimal and obvious):

- `asset update` requires exactly one intent:
  - set display name with `--display-name`, or
  - clear it with `--clear-display-name`.
- `asset update` rejects using both `--display-name` and `--clear-display-name` together.
- `category create` name must not be blank after trimming.

Error output remains stable:

- JSON mode uses a command-oriented error envelope with machine-readable `code`, `step`, and optional `status_code`.
- Human mode prints concise one-line error summary to stderr.

Error JSON schema for non-flow commands:

- `{ "ok": false, "command": "<top-level path>", "error": { "code": "<CODE>", "step": "<step_name>", "status_code": <number|null>, "message": "<text>" } }`

Canonical `step` naming convention:

- Use snake_case handler names matching command intent, for example `category_create`, `category_list`, `category_delete`, `asset_create`, `asset_get`, `asset_list`, `asset_update`, `asset_delete`.

Suggested additional client error code:

- `VALIDATION_ERROR` for preflight checks that fail before HTTP call.

`<ID>` values are typed numeric clap args (`i64`), so non-numeric input is rejected by clap before HTTP execution.

## 6. Output Behavior

JSON mode:

- For `200`/`201`, print full response JSON body.
- For `204 No Content`, print `{ "ok": true, "status_code": 204, "response": null }`.

Human mode:

- `create` and `get` print concise field summary.
- `asset update` currently returns `204`, so it prints `OK status=204`.
- `delete` and any `204` response print `OK status=204`.
- List commands print one line per item with stable key fields.

No table-rendering dependency is introduced in this slice.

## 7. Internal Execution Structure

Execution pipeline:

1. Parse once into `CliArgs`.
2. Dispatch to `run_command(cli)`.
3. Route to specific command handler.
4. Build endpoint path and optional JSON body.
5. Execute HTTP with shared request helpers.
6. Render with shared output layer.

HTTP helper layer:

- Keep existing `get`/`post` helpers.
- Add `patch` and `delete` helpers.

## 8. Testing Plan

Add and update tests in `cli`:

- Parser tests for new command tree and argument requirements.
- Stub-server integration tests that assert request method/path/body for each CRUD operation.
- Error-shape tests for HTTP and validation failures, including stable `step` names per command handler.
- Keep existing `flow scripted-core` tests unchanged to guard regressions.

At least one integration smoke path should run against real `server::app` for new CRUD commands.

## 9. Backward Compatibility

- Preserve `flow scripted-core` behavior and output format.
- Keep global flags unchanged.
- New CRUD commands are additive.
