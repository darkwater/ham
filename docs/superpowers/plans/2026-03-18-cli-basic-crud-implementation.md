# CLI Basic CRUD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add basic CLI CRUD commands for categories and assets using clap-modeled operations first, then command handlers that call existing HTTP endpoints.

**Architecture:** Extend the current `clap` command tree with resource-first subcommands (`category`, `asset`) and dedicated typed args per operation. Route parsed commands through focused handlers that build HTTP requests via shared request helpers, returning a command-oriented JSON envelope and concise human output. Keep `flow scripted-core` behavior unchanged.

**Tech Stack:** Rust, clap derive, serde/serde_json, ureq, axum test stubs, cargo test

---

### Task 1: Model the full clap command tree (including parent-id)

**Files:**
- Modify: `cli/src/main.rs`
- Test: `cli/src/main.rs` (unit tests module)

- [ ] **Step 1: Write failing parser test for category create with parent**

Add unit test `parse_category_create_with_parent_id` that parses:

`cli category create --name Child --parent-id 10`

and asserts parent ID is present as `Some(10)`.

- [ ] **Step 2: Run that single test to verify failure**

Run: `cargo test -p cli parse_category_create_with_parent_id -- --nocapture`
Expected: FAIL because category CRUD clap args are not defined yet.

- [ ] **Step 3: Write failing parser test for malformed parent-id**

Add unit test `parse_category_create_rejects_non_numeric_parent_id` for:

`cli category create --name Child --parent-id abc`

Assert clap parse error kind is invalid value.

- [ ] **Step 4: Run that single test to verify failure**

Run: `cargo test -p cli parse_category_create_rejects_non_numeric_parent_id -- --nocapture`
Expected: FAIL until clap type wiring is added.

- [ ] **Step 5: Implement minimal clap enums/args to satisfy both tests**

In `cli/src/main.rs`, add:
- `CliCommand::{Flow, Category, Asset}`
- `CategoryCommand::{Create, List, Delete}`
- `AssetCommand::{Create, Get, List, Update, Delete}`
- typed per-command arg structs with `i64` IDs

- [ ] **Step 6: Add parser tests for the rest of the command surface**

Add unit tests:
- `parse_category_list`
- `parse_category_delete`
- `parse_asset_create`
- `parse_asset_get_with_include_deleted`
- `parse_asset_list_with_include_deleted`
- `parse_asset_update_display_name`
- `parse_asset_update_clear_display_name`
- `parse_asset_delete`

- [ ] **Step 7: Run parser tests to verify pass**

Run:
- `cargo test -p cli parse_category_create_with_parent_id -- --nocapture`
- `cargo test -p cli parse_category_create_rejects_non_numeric_parent_id -- --nocapture`
- `cargo test -p cli parse_category_list -- --nocapture`
- `cargo test -p cli parse_category_delete -- --nocapture`
- `cargo test -p cli parse_asset_create -- --nocapture`
- `cargo test -p cli parse_asset_get_with_include_deleted -- --nocapture`
- `cargo test -p cli parse_asset_list_with_include_deleted -- --nocapture`
- `cargo test -p cli parse_asset_update_display_name -- --nocapture`
- `cargo test -p cli parse_asset_update_clear_display_name -- --nocapture`
- `cargo test -p cli parse_asset_delete -- --nocapture`
Expected: PASS for parser tests.

- [ ] **Step 8: Commit parser modeling**

```bash
git add cli/src/main.rs
git commit -m "feat: add clap model for category and asset commands"
```

### Task 2: Add command-oriented output and error envelope scaffolding

**Files:**
- Modify: `cli/src/main.rs`
- Test: `cli/tests/cli_flows.rs`

- [ ] **Step 1: Write failing integration test for HTTP error envelope**

Add test `category_create_http_error_uses_command_error_envelope` expecting JSON:

```json
{
  "ok": false,
  "command": "category create",
  "error": {
    "code": "HTTP_ERROR",
    "step": "category_create",
    "status_code": 400,
    "message": "..."
  }
}
```

- [ ] **Step 2: Run that single test to verify failure**

Run: `cargo test -p cli --test cli_flows category_create_http_error_uses_command_error_envelope -- --nocapture`
Expected: FAIL because current CLI is flow-envelope-centric.

- [ ] **Step 3: Implement minimal non-flow error envelope path**

In `cli/src/main.rs`, add a command-mode error rendering path with:
- `ok=false`
- `command` string (for example `category create`)
- `error.code/error.step/error.status_code/error.message`

Also add minimal executable non-flow dispatch for `category create` so the integration test can reach HTTP error handling.

Keep flow-mode errors untouched.

- [ ] **Step 4: Re-run the single error test to verify pass**

Run: `cargo test -p cli --test cli_flows category_create_http_error_uses_command_error_envelope -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit envelope scaffolding**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "refactor: add command-mode error envelope for cli crud"
```

### Task 3: Implement category create command (contract-anchored)

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/cli_flows.rs`
- Reference: `server/src/http/categories.rs`

- [ ] **Step 1: Write failing test for category create request mapping**

Add test `category_create_posts_expected_payload` asserting:
- method `POST`
- path `/categories`
- body `{ "name": "Network" }` for root category creation
- body `{ "name": "Child", "parent_category_id": 10 }` when parent is provided.

This test targets exact payload shape, which remains incomplete after Task 2 scaffolding.

- [ ] **Step 2: Run the test to verify failure**

Run: `cargo test -p cli --test cli_flows category_create_posts_expected_payload -- --nocapture`
Expected: FAIL because exact payload mapping is not complete yet.

- [ ] **Step 3: Implement minimal `category create` handler**

In `cli/src/main.rs`, add `run_category_create` aligned with `server/src/http/categories.rs` request fields.

- [ ] **Step 4: Re-run the test to verify pass**

Run: `cargo test -p cli --test cli_flows category_create_posts_expected_payload -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Write failing validation test for blank name**

Add `category_create_blank_name_returns_validation_error` (trimmed blank name -> `VALIDATION_ERROR` with `step=category_create`).

- [ ] **Step 6: Run validation test to verify failure**

Run: `cargo test -p cli --test cli_flows category_create_blank_name_returns_validation_error -- --nocapture`
Expected: FAIL.

- [ ] **Step 7: Implement minimal blank-name validation and pass test**

Run: `cargo test -p cli --test cli_flows category_create_blank_name_returns_validation_error -- --nocapture`
Expected: PASS.

- [ ] **Step 8: Commit category create**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "feat: add cli category create"
```

### Task 4: Implement category list and delete commands

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/cli_flows.rs`
- Reference: `server/src/http/categories.rs`

- [ ] **Step 1: Write failing real-server smoke test for category create+list path**

Add `category_create_and_list_succeeds_against_real_server_app` using `server::app::build_app`, then execute:
- `cli category create --name Network`
- `cli category list`

This should fail before `category list` is implemented.

- [ ] **Step 2: Run smoke test to verify failure**

Run: `cargo test -p cli --test cli_flows category_create_and_list_succeeds_against_real_server_app -- --nocapture`
Expected: FAIL.

- [ ] **Step 3: Write failing test for category list request mapping**

Add `category_list_gets_categories` asserting `GET /categories`.

- [ ] **Step 4: Run list test to verify failure**

Run: `cargo test -p cli --test cli_flows category_list_gets_categories -- --nocapture`
Expected: FAIL.

- [ ] **Step 5: Implement minimal `category list` handler and pass both list + smoke tests**

Run:
- `cargo test -p cli --test cli_flows category_list_gets_categories -- --nocapture`
- `cargo test -p cli --test cli_flows category_create_and_list_succeeds_against_real_server_app -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Write failing test for category delete mapping**

Add `category_delete_calls_delete_endpoint` asserting `DELETE /categories/{id}`.

- [ ] **Step 7: Run delete test to verify failure**

Run: `cargo test -p cli --test cli_flows category_delete_calls_delete_endpoint -- --nocapture`
Expected: FAIL.

- [ ] **Step 8: Implement dedicated `delete` HTTP helper and `category delete` handler**

In `cli/src/main.rs`, add a shared `delete` request helper and use it in `run_category_delete`.

- [ ] **Step 9: Run delete test to verify pass**

Run: `cargo test -p cli --test cli_flows category_delete_calls_delete_endpoint -- --nocapture`
Expected: PASS.

- [ ] **Step 10: Commit category list/delete**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "feat: add cli category list and delete"
```

### Task 5: Implement asset create and get commands

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/cli_flows.rs`
- Reference: `server/src/http/assets.rs`

- [ ] **Step 1: Write failing test for asset create mapping**

Add `asset_create_posts_expected_payload` asserting:
- `POST /assets`
- payload has `category_id` and optional `asset_tag`
- no `display_name` in create payload (contract alignment).

- [ ] **Step 2: Run create test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_create_posts_expected_payload -- --nocapture`
Expected: FAIL.

- [ ] **Step 3: Implement `asset create` and pass test**

Run: `cargo test -p cli --test cli_flows asset_create_posts_expected_payload -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Write failing test for asset get mapping with include-deleted**

Add `asset_get_appends_include_deleted_query_when_set` asserting path:
- `/assets/{id}` by default
- `/assets/{id}?include_deleted=true` with flag.

- [ ] **Step 5: Run get test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_get_appends_include_deleted_query_when_set -- --nocapture`
Expected: FAIL.

- [ ] **Step 6: Implement `asset get` and pass test**

Run: `cargo test -p cli --test cli_flows asset_get_appends_include_deleted_query_when_set -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit asset create/get**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "feat: add cli asset create and get"
```

### Task 6: Implement asset list, update, and delete commands

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/cli_flows.rs`
- Reference: `server/src/http/assets.rs`

- [ ] **Step 1: Write failing test for asset list mapping**

Add `asset_list_supports_include_deleted_query` asserting `GET /assets` and optional `?include_deleted=true`.

- [ ] **Step 2: Run list test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_list_supports_include_deleted_query -- --nocapture`
Expected: FAIL.

- [ ] **Step 3: Implement `asset list` and pass test**

Run: `cargo test -p cli --test cli_flows asset_list_supports_include_deleted_query -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Write failing test `asset_update_sets_display_name`**

- [ ] **Step 5: Run that test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_update_sets_display_name -- --nocapture`
Expected: FAIL.

- [ ] **Step 6: Implement minimal set-display-name update path and pass**

Run: `cargo test -p cli --test cli_flows asset_update_sets_display_name -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Write failing test `asset_update_clears_display_name`**

- [ ] **Step 8: Run that test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_update_clears_display_name -- --nocapture`
Expected: FAIL.

- [ ] **Step 9: Implement minimal clear-display-name update path and pass**

Run: `cargo test -p cli --test cli_flows asset_update_clears_display_name -- --nocapture`
Expected: PASS.

- [ ] **Step 10: Write failing test `asset_update_rejects_conflicting_flags`**

Assert JSON error includes `error.code = "VALIDATION_ERROR"` and `error.step = "asset_update"`.

- [ ] **Step 11: Run that test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_update_rejects_conflicting_flags -- --nocapture`
Expected: FAIL.

- [ ] **Step 12: Implement conflicting-flag validation and pass**

Run: `cargo test -p cli --test cli_flows asset_update_rejects_conflicting_flags -- --nocapture`
Expected: PASS.

- [ ] **Step 13: Write failing test `asset_update_rejects_missing_update_fields`**

Assert JSON error includes `error.code = "VALIDATION_ERROR"` and `error.step = "asset_update"`.

- [ ] **Step 14: Run that test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_update_rejects_missing_update_fields -- --nocapture`
Expected: FAIL.

- [ ] **Step 15: Implement missing-fields validation and pass**

Run: `cargo test -p cli --test cli_flows asset_update_rejects_missing_update_fields -- --nocapture`
Expected: PASS.

- [ ] **Step 16: Add dedicated `patch` helper and ensure asset update uses it**

In `cli/src/main.rs`, add shared `patch` helper and route all asset update requests through it.

- [ ] **Step 17: Write failing test for asset delete mapping**

Add `asset_delete_calls_delete_endpoint` asserting `DELETE /assets/{id}`.

- [ ] **Step 18: Run delete test to verify failure**

Run: `cargo test -p cli --test cli_flows asset_delete_calls_delete_endpoint -- --nocapture`
Expected: FAIL.

- [ ] **Step 19: Implement `asset delete` and pass test**

Run: `cargo test -p cli --test cli_flows asset_delete_calls_delete_endpoint -- --nocapture`
Expected: PASS.

- [ ] **Step 20: Commit asset list/update/delete**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "feat: add cli asset list update and delete"
```

### Task 7: Add output behavior tests for 200/201 and 204 cases

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/cli_flows.rs`

- [ ] **Step 1: Write failing JSON output tests for success responses**

Add tests:
- `json_output_returns_raw_body_for_201_create`
- `json_output_returns_raw_body_for_200_get_or_list`
- `json_output_uses_no_content_envelope_for_204`

- [ ] **Step 2: Run these tests to verify failure**

Run:
- `cargo test -p cli --test cli_flows json_output_returns_raw_body_for_201_create -- --nocapture`
- `cargo test -p cli --test cli_flows json_output_returns_raw_body_for_200_get_or_list -- --nocapture`
- `cargo test -p cli --test cli_flows json_output_uses_no_content_envelope_for_204 -- --nocapture`
Expected: FAIL.

- [ ] **Step 3: Implement minimal JSON success rendering and pass tests**

Run:
- `cargo test -p cli --test cli_flows json_output_returns_raw_body_for_201_create -- --nocapture`
- `cargo test -p cli --test cli_flows json_output_returns_raw_body_for_200_get_or_list -- --nocapture`
- `cargo test -p cli --test cli_flows json_output_uses_no_content_envelope_for_204 -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Write failing human output tests for success responses**

Add tests:
- `human_output_create_and_get_show_concise_summary`
- `human_output_list_shows_one_line_per_item`
- `human_output_no_content_prints_ok_status_204`

- [ ] **Step 5: Run these tests to verify failure**

Run:
- `cargo test -p cli --test cli_flows human_output_create_and_get_show_concise_summary -- --nocapture`
- `cargo test -p cli --test cli_flows human_output_list_shows_one_line_per_item -- --nocapture`
- `cargo test -p cli --test cli_flows human_output_no_content_prints_ok_status_204 -- --nocapture`
Expected: FAIL.

- [ ] **Step 6: Implement minimal human rendering and pass tests**

Run:
- `cargo test -p cli --test cli_flows human_output_create_and_get_show_concise_summary -- --nocapture`
- `cargo test -p cli --test cli_flows human_output_list_shows_one_line_per_item -- --nocapture`
- `cargo test -p cli --test cli_flows human_output_no_content_prints_ok_status_204 -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit output behavior finalization**

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "test: finalize cli crud success output behavior"
```

### Task 8: Verify canonical error step names and final workspace health

**Files:**
- Modify: `cli/src/main.rs` (if needed)
- Modify: `cli/tests/cli_flows.rs`

- [ ] **Step 1: Write failing error-step-name tests for every CRUD handler**

Add tests that force HTTP failure and assert exact `error.step` values:
- `category_create_error_step_name_is_category_create`
- `category_list_error_step_name_is_category_list`
- `category_delete_error_step_name_is_category_delete`
- `asset_create_error_step_name_is_asset_create`
- `asset_get_error_step_name_is_asset_get`
- `asset_list_error_step_name_is_asset_list`
- `asset_update_error_step_name_is_asset_update`
- `asset_delete_error_step_name_is_asset_delete`

- [ ] **Step 2: Run step-name tests to verify failure**

Run each test explicitly:
- `cargo test -p cli --test cli_flows category_create_error_step_name_is_category_create -- --nocapture`
- `cargo test -p cli --test cli_flows category_list_error_step_name_is_category_list -- --nocapture`
- `cargo test -p cli --test cli_flows category_delete_error_step_name_is_category_delete -- --nocapture`
- `cargo test -p cli --test cli_flows asset_create_error_step_name_is_asset_create -- --nocapture`
- `cargo test -p cli --test cli_flows asset_get_error_step_name_is_asset_get -- --nocapture`
- `cargo test -p cli --test cli_flows asset_list_error_step_name_is_asset_list -- --nocapture`
- `cargo test -p cli --test cli_flows asset_update_error_step_name_is_asset_update -- --nocapture`
- `cargo test -p cli --test cli_flows asset_delete_error_step_name_is_asset_delete -- --nocapture`
Expected: FAIL until all handlers use canonical step names.

- [ ] **Step 3: Implement any missing step-name wiring and re-run tests**

Run the same explicit list; expected PASS.

- [ ] **Step 4: Write failing human-mode error output tests**

Add tests:
- `human_error_output_http_failure_is_one_line`
- `human_error_output_validation_failure_is_one_line`

Each test should run a CRUD command with `--output human` and assert stderr matches a concise one-line pattern containing code, step, status, and message.

- [ ] **Step 5: Run human error tests to verify failure**

Run:
- `cargo test -p cli --test cli_flows human_error_output_http_failure_is_one_line -- --nocapture`
- `cargo test -p cli --test cli_flows human_error_output_validation_failure_is_one_line -- --nocapture`
Expected: FAIL.

- [ ] **Step 6: Implement minimal human error rendering adjustments and pass tests**

Run the same two tests; expected PASS.

- [ ] **Step 7: Run full CLI tests**

Run: `cargo test -p cli --test cli_flows`
Expected: PASS.

- [ ] **Step 8: Run final workspace verification and commit**

Run:
- `cargo fmt --all`
- `cargo test --workspace`

Then commit:

```bash
git add cli/src/main.rs cli/tests/cli_flows.rs
git commit -m "test: add real-server smoke coverage for cli crud"
```
