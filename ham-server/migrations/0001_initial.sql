CREATE TABLE categories (
    id                 INTEGER PRIMARY KEY,
    display_name       TEXT NOT NULL,
    created_at         TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    parent_category_id INTEGER REFERENCES categories(id)
);
-- we currently assume that id 1 is the root category
INSERT INTO categories (id, display_name) VALUES (1, 'Root');

CREATE TABLE assets (
    id           INTEGER PRIMARY KEY,
    category_id  INTEGER NOT NULL REFERENCES categories(id),
    display_name TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at   TEXT
);

CREATE TABLE enum_types (
    id           INTEGER PRIMARY KEY,
    type_key     TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE field_definitions (
    id           INTEGER PRIMARY KEY,
    -- name         TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    value_type   TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE category_field_hints (
    category_id INTEGER NOT NULL REFERENCES categories(id),
    field_id    INTEGER NOT NULL REFERENCES field_definitions(id),

    PRIMARY KEY (category_id, field_id)
);

CREATE TABLE enum_values (
    id           INTEGER PRIMARY KEY,
    enum_type_id INTEGER NOT NULL REFERENCES enum_types(id),
    -- name         TEXT NOT NULL,
    display_name TEXT NOT NULL,
    order_index  INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

    -- UNIQUE (enum_type_id, name)
);

CREATE TABLE event_types (
    id              INTEGER PRIMARY KEY,
    -- name            TEXT PRIMARY KEY,
    display_name    TEXT NOT NULL,
    description     TEXT,
    current_version INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE event_type_mutations (
    id                 INTEGER PRIMARY KEY,
    event_type_id      TEXT NOT NULL REFERENCES event_types(id),
    event_type_version INTEGER NOT NULL DEFAULT 1,
    mutation_index     INTEGER NOT NULL,
    operation          TEXT NOT NULL,
    field_id           INTEGER NOT NULL REFERENCES field_definitions(id),
    input_key          TEXT,
    created_at         TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE (event_type_id, event_type_version, mutation_index)
);

CREATE TABLE asset_current_field_values (
    asset_id   INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    field_id   INTEGER NOT NULL REFERENCES field_definitions(id),
    value      JSON NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (asset_id, field_id)
);

CREATE TABLE asset_events (
    id                 INTEGER PRIMARY KEY,
    asset_id           INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    idempotency_key    TEXT NOT NULL,
    event_type_id      TEXT NOT NULL REFERENCES event_types(kd),
    event_type_version INTEGER NOT NULL DEFAULT 1,
    payload_json       TEXT NOT NULL,
    created_at         TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE (asset_id, idempotency_key)
);

CREATE TABLE settings (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO settings (name, value) VALUES ('next_asset_id', '1');
INSERT INTO settings (name, value) VALUES ('asset_prefix', 'A');
