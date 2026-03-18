# Personal Asset Tracker V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust HTTP server + CLI + egui GUI for personal asset tracking with category trees, flexible typed tags, external entities, per-asset timelines, and event-driven state updates.

**Architecture:** Use a modular Rust workspace with `domain` for validation and event application rules, `server` for HTTP + SQLite persistence, and separate `cli`/`gui` clients calling the server. Persist typed tag values in a single JSON column with strict domain validation, use versioned event types, and keep all state mutations event-driven.

**Tech Stack:** Rust (workspace crates), axum, sqlx + SQLite, serde/serde_json, clap, eframe/egui, tokio, cargo test.

---

## Scope and sequencing

- This plan targets one integrated subsystem (asset tracker v1) and is intentionally sequenced to deliver vertical slices: schema + domain rules, then API, then clients.
- TDD-first workflow is required for each task: write failing test, run, implement minimal code, rerun tests, commit.

## Planned file structure

- Create workspace and crates:
  - `Cargo.toml`
  - `crates/domain/Cargo.toml`
  - `crates/domain/src/lib.rs`
  - `crates/server/Cargo.toml`
  - `crates/server/src/main.rs`
  - `crates/server/src/app.rs`
  - `crates/server/src/db/mod.rs`
  - `crates/server/src/http/mod.rs`
  - `crates/cli/Cargo.toml`
  - `crates/cli/src/main.rs`
  - `crates/gui/Cargo.toml`
  - `crates/gui/src/main.rs`
- Domain modules:
  - `crates/domain/src/types.rs`
  - `crates/domain/src/tag_values.rs`
  - `crates/domain/src/events.rs`
  - `crates/domain/src/errors.rs`
  - `crates/domain/src/external_entities.rs`
- Server DB + migrations:
  - `crates/server/migrations/0001_initial.sql`
  - `crates/server/migrations/0002_event_versioning.sql`
  - `crates/server/src/db/repo_assets.rs`
  - `crates/server/src/db/repo_tag_generator.rs`
  - `crates/server/src/db/repo_events.rs`
  - `crates/server/src/db/repo_tags.rs`
  - `crates/server/src/db/repo_external_entities.rs`
- Server HTTP handlers:
  - `crates/server/src/http/assets.rs`
  - `crates/server/src/http/events.rs`
  - `crates/server/src/http/categories.rs`
  - `crates/server/src/http/tag_definitions.rs`
  - `crates/server/src/http/category_tag_hints.rs`
  - `crates/server/src/http/event_types.rs`
  - `crates/server/src/http/external_entities.rs`
  - `crates/server/src/http/search.rs`
- Tests:
  - `crates/domain/tests/tag_value_validation.rs`
  - `crates/domain/tests/event_application.rs`
  - `crates/server/tests/http_assets.rs`
  - `crates/server/tests/http_categories.rs`
  - `crates/server/tests/http_events.rs`
  - `crates/server/tests/http_search.rs`
  - `crates/cli/tests/cli_flows.rs`
  - `crates/gui/tests/gui_state.rs`

### Task 1: Bootstrap Rust workspace and crate skeletons

**Files:**
- Create: `Cargo.toml`
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/server/Cargo.toml`
- Create: `crates/server/src/main.rs`
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`
- Create: `crates/gui/Cargo.toml`
- Create: `crates/gui/src/main.rs`
- Test: `cargo check`

- [ ] **Step 1: Write failing compile expectation test (workspace not yet valid)**

Run: `cargo check`
Expected: FAIL because workspace/crates do not exist yet.

- [ ] **Step 2: Create minimal workspace manifests and crate entrypoints**

Add minimal `main`/`lib` files that compile with placeholder output.

- [ ] **Step 3: Run compile check**

Run: `cargo check`
Expected: PASS for all workspace members.

- [ ] **Step 4: Commit bootstrap**

```bash
git add Cargo.toml crates/domain crates/server crates/cli crates/gui
git commit -m "chore: bootstrap rust workspace for asset tracker"
```

### Task 2: Implement core domain types and tag value validation

**Files:**
- Create: `crates/domain/src/types.rs`
- Create: `crates/domain/src/tag_values.rs`
- Create: `crates/domain/src/errors.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: `crates/domain/tests/tag_value_validation.rs`

- [ ] **Step 1: Write failing domain tests for field type/value validation**

Cover at least:
- `enum` value accepts known `option_key` and rejects unknown.
- `external_entity(type_id)` requires integer ID and type match.
- `ipv4` accepts canonical dotted quad and rejects invalid/leading-zero forms.
- `money` accepts canonical decimal string and rejects JSON number.
- JSON `null` is rejected for stored tag values.

- [ ] **Step 2: Run domain tests to verify failures**

Run: `cargo test -p domain --test tag_value_validation`
Expected: FAIL with missing type/validator errors.

- [ ] **Step 3: Implement minimal domain types + validator logic**

Implement `FieldType` (Serde JSON enum), `TagValue`, and validation entrypoints.

- [ ] **Step 4: Re-run domain tests**

Run: `cargo test -p domain --test tag_value_validation`
Expected: PASS.

- [ ] **Step 5: Commit domain type system**

```bash
git add crates/domain
git commit -m "feat: add domain field types and tag value validation"
```

### Task 3: Implement domain event model and application engine

**Files:**
- Create: `crates/domain/src/events.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: `crates/domain/tests/event_application.rs`

- [ ] **Step 1: Write failing tests for event application behavior**

Cover at least:
- `set`, `clear`, `increment` (integer/decimal only).
- event type version pinning (`event_type_id` + `event_type_version`).
- all-or-nothing failure on invalid mutation.
- idempotency hash comparison semantics (same payload/same result; mismatch fails).

- [ ] **Step 2: Run event tests to verify failures**

Run: `cargo test -p domain --test event_application`
Expected: FAIL with missing event engine behavior.

- [ ] **Step 3: Implement event engine minimally to satisfy tests**

Implement mutation spec execution, validation, and deterministic apply order.

- [ ] **Step 4: Re-run event tests**

Run: `cargo test -p domain --test event_application`
Expected: PASS.

- [ ] **Step 5: Commit domain event engine**

```bash
git add crates/domain
git commit -m "feat: implement domain event application engine"
```

### Task 4: Create SQLite schema + migrations

**Files:**
- Create: `crates/server/migrations/0001_initial.sql`
- Create: `crates/server/migrations/0002_event_versioning.sql`
- Create: `crates/server/src/db/mod.rs`
- Test: `crates/server/tests/http_assets.rs`

- [ ] **Step 1: Write failing integration test that boots DB and checks required tables/constraints**

Validate presence of tables and critical constraints:
- unique `asset_tag`
- immutable event IDs
- `(asset_id, idempotency_key)` uniqueness
- FK for `external_entity` and enum option references

- [ ] **Step 2: Run server integration test to confirm failure**

Run: `cargo test -p server --test http_assets db_schema_applies`
Expected: FAIL due to missing migrations/schema.

- [ ] **Step 3: Implement initial schema and migration runner**

Include all spec tables including `tag_enum_options`, `external_entity_types`, `external_entities`, event tables, and projection table with `value_json`.

- [ ] **Step 4: Write failing startup compatibility test**

Run: `cargo test -p server --test http_assets startup_fails_on_incompatible_schema_version`
Expected: FAIL because incompatible-schema detection is not yet implemented.

- [ ] **Step 5: Implement schema compatibility check at startup**

Implement explicit startup error when schema version is incompatible.

- [ ] **Step 6: Re-run schema tests**

Run: `cargo test -p server --test http_assets db_schema_applies startup_fails_on_incompatible_schema_version`
Expected: PASS.

- [ ] **Step 7: Commit schema layer**

```bash
git add crates/server/migrations crates/server/src/db crates/server/tests/http_assets.rs
git commit -m "feat: add sqlite schema and migration runner"
```

### Task 5: Implement asset tag generator rules (global counter)

**Files:**
- Create: `crates/server/src/db/repo_tag_generator.rs`
- Modify: `crates/server/src/db/repo_assets.rs`
- Test: `crates/server/tests/http_assets.rs`

- [ ] **Step 1: Write failing test for auto-generated asset tag**

Run: `cargo test -p server --test http_assets auto_generates_asset_tag`
Expected: FAIL because generator logic is missing.

- [ ] **Step 2: Implement minimal global-counter generation**

Implement: single monotonic counter, configurable prefix/digit formatting, transactional write.

- [ ] **Step 3: Re-run generation test**

Run: `cargo test -p server --test http_assets auto_generates_asset_tag`
Expected: PASS.

- [ ] **Step 4: Write failing test for manual override not advancing counter**

Run: `cargo test -p server --test http_assets manual_override_does_not_advance_counter`
Expected: FAIL because sequence behavior is not enforced.

- [ ] **Step 5: Implement minimal override behavior**

Implement: manual tag accepted if unique, counter unchanged.

- [ ] **Step 6: Re-run override test**

Run: `cargo test -p server --test http_assets manual_override_does_not_advance_counter`
Expected: PASS.

- [ ] **Step 7: Write failing test for collision retry in same transaction**

Run: `cargo test -p server --test http_assets generator_retries_on_collision`
Expected: FAIL because retry path is not implemented.

- [ ] **Step 8: Implement collision retry loop**

Implement: on conflict, increment counter and retry within transaction until free tag exists.

- [ ] **Step 9: Re-run collision retry test**

Run: `cargo test -p server --test http_assets generator_retries_on_collision`
Expected: PASS.

- [ ] **Step 10: Commit tag generator rules**

```bash
git add crates/server/src/db/repo_tag_generator.rs crates/server/src/db/repo_assets.rs crates/server/tests/http_assets.rs
git commit -m "feat: implement global asset tag generator rules"
```

### Task 6: Implement server HTTP endpoints for core CRUD and lifecycle rules

**Files:**
- Create: `crates/server/src/app.rs`
- Create: `crates/server/src/http/mod.rs`
- Create: `crates/server/src/http/assets.rs`
- Create: `crates/server/src/http/categories.rs`
- Create: `crates/server/src/http/tag_definitions.rs`
- Create: `crates/server/src/http/category_tag_hints.rs`
- Create: `crates/server/src/http/external_entities.rs`
- Modify: `crates/server/src/main.rs`
- Test: `crates/server/tests/http_assets.rs`
- Test: `crates/server/tests/http_categories.rs`

- [ ] **Step 1: Write failing HTTP tests for asset CRUD + conflict rules**

Cover at least:
- create/read/update metadata for assets
- soft delete returns `410` for later mutations
- blocking category deletion with children/assets (`409`)
- blocking tag-definition type mutation/deletion when referenced (`409`)
- direct asset tag-value update through asset update route returns `409`
- soft-deleted assets excluded from default list/search
- `include_deleted=true` includes soft-deleted assets
- server accepts `Authorization` header but performs no auth decisions
- blocking deletion of referenced external entity types (`409` + reason code)
- blocking hard-delete of referenced external entities (`409` + reason code)
- enum option deletion blocked when referenced (`409` + reason code)
- enum option retirement via `is_active=false` remains allowed

- [ ] **Step 2: Run HTTP asset tests to verify failure**

Run: `cargo test -p server --test http_assets`
Expected: FAIL with route/handler not implemented.

- [ ] **Step 3: Implement handlers and repository functions minimally**

Keep GET read-only; allow read-only `POST /assets/search` per spec.

- [ ] **Step 4: Re-run HTTP asset tests**

Run: `cargo test -p server --test http_assets`
Expected: PASS.

- [ ] **Step 5: Write failing tests for category tag hint CRUD/inheritance**

Run: `cargo test -p server --test http_categories category_tag_hint_crud_and_inheritance`
Expected: FAIL because hint endpoints/queries are missing.

- [ ] **Step 6: Implement category tag hint handlers + inheritance read model**

Implement create/list/delete hint endpoints and inherited lookup behavior.

- [ ] **Step 7: Re-run category tests**

Run: `cargo test -p server --test http_categories category_tag_hint_crud_and_inheritance`
Expected: PASS.

- [ ] **Step 8: Commit core HTTP layer**

```bash
git add crates/server/src crates/server/tests/http_assets.rs crates/server/tests/http_categories.rs
git commit -m "feat: add core HTTP CRUD endpoints with lifecycle rules"
```

### Task 7: Implement event type versioning and event timeline endpoints

**Files:**
- Create: `crates/server/src/http/event_types.rs`
- Create: `crates/server/src/http/events.rs`
- Create: `crates/server/src/db/repo_events.rs`
- Test: `crates/server/tests/http_events.rs`

- [ ] **Step 1: Write failing tests for event-type versions and event posting**

Cover at least:
- create event type v1 and create new versions
- cannot delete referenced version
- `POST /assets/{asset_tag}/events` requires `Idempotency-Key`
- missing `Idempotency-Key` returns `400`
- same key+payload replay returns original result
- same key+different payload returns `409`
- canonical-equivalent JSON payload hashes as same request
- same body but different method/path/asset does not collide idempotency hash scope
- server-generated timestamp ignores client-provided timestamp
- timeline default order is `timestamp DESC, event_id DESC`
- timeline cursor pagination is stable across pages with event-id tiebreaking

- [ ] **Step 2: Run event HTTP tests to verify failure**

Run: `cargo test -p server --test http_events`
Expected: FAIL due to missing endpoints/logic.

- [ ] **Step 3: Implement event handlers + transaction boundaries**

Ensure one transaction updates event log and projection atomically.

- [ ] **Step 4: Re-run event HTTP tests**

Run: `cargo test -p server --test http_events`
Expected: PASS.

- [ ] **Step 5: Commit event API implementation**

```bash
git add crates/server/src/http/event_types.rs crates/server/src/http/events.rs crates/server/src/db/repo_events.rs crates/server/tests/http_events.rs
git commit -m "feat: add versioned event types and timeline endpoints"
```

### Task 8: Implement search endpoint contract

**Files:**
- Create: `crates/server/src/http/search.rs`
- Modify: `crates/server/src/http/mod.rs`
- Test: `crates/server/tests/http_search.rs`

- [ ] **Step 1: Write failing search tests**

Cover:
- `POST /assets/search` filter operators (`eq`, `contains`, `between`, `is_null`)
- operator coverage for `lt`, `lte`, `gt`, `gte`, `is_not_null`, and boolean `eq`
- grouped `OR` blocks with default `AND` behavior
- external entity ID equality filtering
- enum option key equality filtering
- category subtree filtering
- text search filtering
- default stable sort by asset tag when no sort is provided
- explicit multi-key sort precedence
- optional unbounded `limit` behavior in v1
- response contains optional `total_estimate`

- [ ] **Step 2: Run search tests to verify failure**

Run: `cargo test -p server --test http_search`
Expected: FAIL with missing search route/logic.

- [ ] **Step 3: Implement minimal search request/response contract**

Implement `filters`, `sort`, optional `limit`, optional `cursor`, `next_cursor`.

- [ ] **Step 4: Re-run search tests**

Run: `cargo test -p server --test http_search`
Expected: PASS.

- [ ] **Step 5: Commit search API**

```bash
git add crates/server/src/http/search.rs crates/server/src/http/mod.rs crates/server/tests/http_search.rs
git commit -m "feat: add asset search endpoint with typed filters"
```

### Task 9: Implement CLI core workflows for automation

**Files:**
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/cli_flows.rs`

- [ ] **Step 1: Write failing CLI integration tests**

Cover scripted flow:
- create category/tag definitions
- create enum options + external entities
- create asset
- apply event
- fetch timeline
- run search

- [ ] **Step 2: Run CLI tests to verify failure**

Run: `cargo test -p cli --test cli_flows`
Expected: FAIL with missing commands.

- [ ] **Step 3: Implement minimal CLI commands and JSON/table output**

Ensure deterministic output shape for automation.

- [ ] **Step 4: Re-run CLI tests**

Run: `cargo test -p cli --test cli_flows`
Expected: PASS.

- [ ] **Step 5: Commit CLI workflow support**

```bash
git add crates/cli/src/main.rs crates/cli/tests/cli_flows.rs
git commit -m "feat: add CLI commands for core asset workflows"
```

### Task 10: Implement GUI v1 basic flows (no forced pagination)

**Files:**
- Modify: `crates/gui/src/main.rs`
- Create: `crates/gui/tests/gui_state.rs`

- [ ] **Step 1: Write failing GUI state tests**

Cover state/view-model behavior for:
- load categories/assets
- open asset detail
- apply event from event type
- render timeline
- enum/external entity selectors

- [ ] **Step 2: Run GUI state tests to verify failure**

Run: `cargo test -p gui --test gui_state`
Expected: FAIL with missing state/controller logic.

- [ ] **Step 3: Implement minimal GUI screens and HTTP client glue**

Use full-list fetch for v1 (no forced pagination UI).

- [ ] **Step 4: Re-run GUI state tests**

Run: `cargo test -p gui --test gui_state`
Expected: PASS.

- [ ] **Step 5: Run GUI compile checks**

Run: `cargo check -p gui`
Expected: PASS.

- [ ] **Step 6: Commit GUI baseline**

```bash
git add crates/gui/src/main.rs crates/gui/tests/gui_state.rs
git commit -m "feat: add egui baseline for asset browsing and event application"
```

### Task 11: End-to-end verification and docs sync

**Files:**
- Modify: `docs/superpowers/specs/2026-03-18-personal-asset-tracker-design.md` (only if implementation-driven clarifications are needed)

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 2: Run formatting and lint checks**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Verify binary startup paths**

Run: `cargo run -p server -- --help && cargo run -p cli -- --help && cargo run -p gui -- --help`
Expected: PASS with help output for each binary.

- [ ] **Step 4: Commit verification updates**

```bash
git add -A
git commit -m "chore: verify workspace tests and v1 readiness"
```

## Implementation notes

- Keep all mutation logic in domain/service layers, not HTTP handlers.
- Keep SQL queries repository-local and avoid leaking DB row shapes to the GUI.
- Prefer additive migrations; do not edit already-applied migration files.
- Keep API error responses structured with machine-readable codes for CLI automation.
