# Personal Asset Tracker Design

Date: 2026-03-18
Status: Draft approved in chat

## 1. Goals and Constraints

- Build a personal asset tracking system in Rust with a server-client model.
- Use plain HTTP (no HTTPS termination in app).
- Keep server auth out of scope for v1; clients may send `Authorization` headers for reverse-proxy setups.
- Support both CLI and GUI clients.
- Preserve strict HTTP semantics: `GET` is always read-only; mutating operations must use non-`GET`. Read-only `POST` is allowed for complex query endpoints (for example `POST /assets/search`).
- Prioritize flexibility over Snipe-IT style rigid entities.

## 2. Recommended Architecture

Use a modular monolith in a Rust workspace:

- `crates/domain`: core models, validation, event application logic.
- `crates/server`: HTTP API + SQLite persistence + migration runner.
- `crates/cli`: batch/script-friendly client for automation and headless testing.
- `crates/gui`: egui client (based on `eframe_template`).

Keep API DTOs inside the shared domain crate for v1 (for example `domain::dto`) rather than splitting a separate `api_types` crate initially.

## 3. Core Data Model

### 3.1 Category and Tag Model

- `Category`: tree structure with `id`, `name`, `parent_id`.
- Asset belongs to exactly one category.
- `TagDefinition`: user-defined field definitions with a static `field_type` enum (Serde JSON-encoded enum values).
  - `field_type` is immutable once the tag definition is referenced by any event type version or stored asset tag value.
  - Changing type requires creating a new tag definition and migrating data via explicit events/tools.
- `CategoryTagHint`: links tags to categories.
  - Hints inherit down category tree.
  - Hints are suggestions only, not enforcement requirements.
- `ExternalEntityType`: user-defined reference type catalog (for example `person`, `location`, `vendor`).
- `ExternalEntity`: concrete record in a type catalog (for example `Office Shelf`, `Alice`).
- There is no special `location` domain concept in code; location is modeled as `ExternalEntityType` + `ExternalEntity`.

### 3.2 Field Type System

`field_type` is a static Rust enum serialized as JSON in storage and wire contracts. It is not user-extensible at runtime in v1.

Examples:

- `"text"`
- `"integer"`
- `"decimal"`
- `"boolean"`
- `"date"`
- `"datetime"`
- `"money"`
- `"url"`
- `"mac_address"`
- `"ipv4"`
- `"enum"`
- `{ "external_entity": 4 }` (references `external_entity_types.id = 4`)

This drives both validation and UI rendering.

Tag value storage uses a single JSON column (`asset_tag_values.value_json`) for all field types. Validation is strict and type-aware in the domain layer.

Canonical value representations in `value_json`:

- `text`: JSON string
- `integer`: JSON integer
- `decimal`: JSON string decimal representation
- `boolean`: JSON boolean
- `date`: RFC3339 full-date string (`YYYY-MM-DD`)
- `datetime`: UTC RFC3339 datetime string
- `money`: JSON string decimal representation (no currency component in v1; presentation currency is UI setting)
- `url`: absolute URL string
- `mac_address`: normalized lowercase MAC string (`xx:xx:xx:xx:xx:xx`)
- `ipv4`: canonical dotted-quad IPv4 string only (exactly four octets, each `0-255`, no leading zeros except `0`), IPv6 out of scope for v1
- `enum`: JSON string containing `tag_enum_options.option_key` for that tag definition
- `external_entity(<type_id>)`: JSON integer containing `external_entities.id` whose row must match referenced `external_entity_types.id`

Null representation rule:

- `asset_tag_values.value_json` must never store JSON `null`.
- Missing tag value is represented only by absence (`NULL` / missing row per table design), and query operators (`eq`, `is_null`, `is_not_null`) target that single representation.

Per-tag enum definition model:

- Enum options are scoped to a single tag definition in v1 (no global reusable enum registry).
- `tag_enum_options` table stores:
  - `id`
  - `tag_definition_id`
  - `option_key` (stable stored value)
  - `label` (UI display value)
  - `sort_order`
  - `is_active`
- Uniqueness: `UNIQUE(tag_definition_id, option_key)`.
- Enum value validation requires `option_key` to exist and be active for the target tag definition.

### 3.3 Asset Identity

- Internal stable UUID primary key.
- Immutable human-facing `asset_tag` (like Snipe-IT style).
- Tag generator is global sequence with user-configurable prefix and digit length.
  - Server auto-generates by default.
  - Client may optionally propose explicit override; server validates uniqueness.
- Sequence is a single global monotonic counter.
  - Auto-generation increments atomically from the global counter.
  - Manual override does not advance sequence in v1.
  - Changing prefix/digit length only affects newly generated tags; existing tags are unchanged.
  - If an auto-generated candidate collides with an existing tag (for example from manual override), generation increments and retries within the same transaction until a free tag is found.

## 4. Event Model (Dynamic but Guarded)

### 4.1 Event Definitions

- `EventType` is user-defined and stored in SQLite (global scope, not category-scoped in v1).
- Each event type has zero or more ordered mutation specs.
- Each mutation spec contains:
  - `target_tag_id`
  - `op` (`set` | `clear` | `increment`)
  - `value_source` (`literal` | `input`)
  - `literal_value` when `value_source = literal`
  - `input_key`, `input_type`, `required`, and optional `default` when `value_source = input`
- `input_type` reuses the same `field_type` enum used by tag definitions, including JSON representation and canonical value rules.
- Event type definitions are append-only and versioned.
  - Editing an event type creates a new version.
  - Existing versions are immutable.
  - Referenced versions cannot be deleted.

### 4.2 Mutation Capabilities (v1)

Simple operations only:

- `set`
- `clear`
- `increment` (arithmetic fields only: `integer` and `decimal`)

`increment` rules in v1:

- Allowed only for `integer` and `decimal` fields.
- No mixed-type coercion.
  - `integer` requires integer operand.
  - `decimal` requires canonical decimal-string operand.

These mutations target asset tag values.

### 4.3 Event Instances and Timeline

- `AssetEvent`: immutable event record with event type reference, timestamp, optional note, and `inputs` key/value payload.
- Every asset has a timeline of attached events.
- Event application is atomic all-or-nothing.
  - If any mutation is invalid, the entire event fails.
  - `inputs` are validated strictly against the selected `EventType` mutation specs before any writes occur.
- `AssetEvent` stores `event_type_id` and `event_type_version` so replay/projection always uses the exact schema used at write time.
- Event timestamps are server-generated only (UTC RFC3339 with microsecond precision).
- `POST /assets/{asset_tag}/events` requires `Idempotency-Key` header unique per asset.
  - Server stores canonical request hash per `(asset_id, idempotency_key)`.
  - Canonical request hash is computed from RFC 8785 canonical JSON for the request body plus `(method, path, asset_id)`.
  - Same key + same payload returns original response without creating a new event.
  - Same key + different payload returns `409 Conflict` with `idempotency_key_payload_mismatch`.
  - Missing `Idempotency-Key` returns `400 Bad Request`.

### 4.4 Current State Projection

- Current asset state is represented by projected/current tag values.
- Server updates projection when events are applied.
- Snapshot edits that bypass events are not allowed in v1.
- Event application and projection update happen in one SQLite transaction with rollback on error.

## 5. HTTP API Shape

REST resources + dedicated event endpoints:

- CRUD endpoints for categories, tag definitions, and category tag hints.
- CRUD endpoints for external entity types and external entities.
- Event type endpoints are versioned lifecycle endpoints (not unrestricted CRUD):
  - `POST /event-types` creates initial version.
  - `POST /event-types/{id}/versions` creates new immutable version.
  - `GET /event-types/{id}` and `GET /event-types/{id}/versions/{version}` fetch definitions.
  - Deleting referenced versions is forbidden; v1 does not require delete support.
- Asset endpoints support create/read/delete and metadata updates only (for example display name, category assignment, notes).
- Asset tag values are event-driven only; direct tag value updates through asset update routes are rejected (`409 Conflict`).
- `POST /assets/{asset_tag}/events` to apply one event instance.
- `GET /assets/{asset_tag}/events` for timeline retrieval (ordered + paginated).
- Query/filter support across category subtree, tags, external-entity references, availability-like fields, and text search.
- V1 search contract uses `POST /assets/search` with JSON body:
  - `filters`: list of predicates combined with `AND` by default; optional grouped `OR` blocks.
  - `sort`: ordered list of keys with direction (`asc`/`desc`), defaulting to stable asset tag order.
  - `limit`: optional integer with no hard max in v1.
  - `cursor`: opaque token from prior response.
- V1 search response envelope:
  - `items`: asset records for current page.
  - `next_cursor`: opaque token or `null`.
  - `total_estimate`: optional approximate count.
- GUI v1 may request unpaginated asset lists (no cursor, no limit) for initial implementation simplicity.
- Filter operators (v1):
  - Text-like fields: `eq`, `contains`
  - Numeric/date-like fields: `eq`, `lt`, `lte`, `gt`, `gte`, `between`
  - Boolean-like fields: `eq`
- Missing tag values are `NULL` in query semantics.
  - `eq` does not match `NULL`.
  - `is_null` and `is_not_null` are explicit operators.
  - `between` is inclusive at both bounds.
- Type-specific filter notes in v1:
  - `money`: numeric comparisons (`eq`, `lt`, `lte`, `gt`, `gte`, `between`).
  - `enum`: `eq` on `option_key`.
  - `external_entity(<type_id>)`: `eq` on entity id.
  - `url` and `mac_address`: treated as text-like.
  - `ipv4`: `eq` on canonical value; optional `contains` as substring match on canonical text.
- Event timeline default order: `timestamp DESC, event_id DESC`.
- Pagination uses cursor semantics: `limit` + opaque `cursor` token in request/response.
- Cursor stability is defined by `(timestamp DESC, event_id DESC)` using server-generated timestamps.

Asset delete behavior:

- Delete is soft-delete only in v1 (`deleted_at` timestamp).
- Asset events and projected tag values are retained.
- Deleted assets are excluded from default list/search responses.
- Deleted assets can be included with `include_deleted=true`.
- Soft-deleted assets are immutable in v1:
  - Event apply and metadata updates return `410 Gone`.
  - Restore endpoint is out of scope for v1.

External entity lifecycle behavior:

- `external_entity_types` referenced by any `tag_definitions.field_type` cannot be deleted.
- `external_entities` referenced by any `asset_tag_values` cannot be hard-deleted in v1.
- Renaming external entity types/entities is allowed and does not change IDs.
- Referential violations return `409 Conflict`.

Category and tag-definition lifecycle behavior:

- Category deletion is blocked if category has child categories or assigned assets.
- Tag definition deletion is blocked if referenced by any event type version, category tag hint, or stored asset tag value.
- Enum option deletion is blocked if referenced by any stored asset tag value or event literal/default; use `is_active = false` to retire options.
- Blocked lifecycle operations return `409 Conflict` with machine-readable reason code.

Auth behavior:

- Server ignores `Authorization` for v1.
- Server never performs auth decisions and never maps credentials to identity/permissions.
- Reverse proxy can enforce credentials externally if desired.

## 6. Persistence and Migrations

- SQLite is the only v1 datastore.
- Versioned migration system from day one.
- Startup checks schema version and fails clearly on incompatibility.
- Event application and asset tag generation each run in a single transaction.
- Unique constraints are required for `asset_tag` and immutable `asset_events.id`.
- Idempotency uniqueness is required for `(asset_id, idempotency_key)`.

Expected table families:

- Categories tree
- Assets
- Tag definitions
- Tag enum options
- Category tag hints
- External entity types
- External entities
- Event types
- Event type mutations
- Asset current tag values (projection)
- Asset events (immutable log)
- Tag generator settings/counters

## 7. Client Requirements

### 7.1 CLI

- Full coverage of core workflows for scripting/automation.
- Deterministic output modes (human table + JSON).
- Suitable for batch transactions and AI-driven testing.

### 7.2 GUI (egui)

- Category tree browsing.
- Asset detail view with full timeline.
- Event application forms generated from selected event type schema.

## 8. Testing Strategy

- Domain unit tests: field type validation, mutation rules, atomic event behavior.
- Server integration tests: HTTP contract, SQLite transactions, migration checks.
- CLI tests: golden/fixture-based command output for end-to-end flows.

## 9. V1 Scope and Non-Goals

### In Scope

- Server + CLI + GUI over HTTP.
- Category tree with inherited tag hints.
- Dynamic DB-defined event types with simple mutations.
- Immutable event timeline and atomic application.
- Asset tag generation and immutability.

### Out of Scope

- Server-side auth/permissions/roles.
- Multi-user concerns.
- Category-level required-field enforcement.
- User scripting language for events.
- Multi-category membership for assets.
- Import/export workflows.

## 10. Delivery Slices

1. Schema + migration framework + domain primitives.
2. Category/tag/asset CRUD + asset tag generation.
3. Event type definition + event apply engine + timeline API.
4. CLI complete flow coverage.
5. GUI core workflows.
6. Polish: search ergonomics and diagnostics.
