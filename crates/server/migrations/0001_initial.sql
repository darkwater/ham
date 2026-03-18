CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE assets (
    id INTEGER PRIMARY KEY,
    category_id INTEGER NOT NULL,
    asset_tag TEXT NOT NULL,
    display_name TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (category_id) REFERENCES categories(id),
    UNIQUE (asset_tag)
);

CREATE TABLE external_entity_types (
    id INTEGER PRIMARY KEY,
    type_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tag_definitions (
    id INTEGER PRIMARY KEY,
    tag_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    value_type TEXT NOT NULL,
    external_entity_type_id INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (external_entity_type_id) REFERENCES external_entity_types(id)
);

CREATE TABLE tag_enum_options (
    id INTEGER PRIMARY KEY,
    tag_definition_id INTEGER NOT NULL,
    option_key TEXT NOT NULL,
    display_name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tag_definition_id) REFERENCES tag_definitions(id),
    UNIQUE (tag_definition_id, option_key)
);

CREATE TABLE category_tag_hints (
    category_id INTEGER NOT NULL,
    tag_definition_id INTEGER NOT NULL,
    is_required INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (category_id, tag_definition_id),
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (tag_definition_id) REFERENCES tag_definitions(id)
);

CREATE TABLE external_entities (
    id INTEGER PRIMARY KEY,
    external_entity_type_id INTEGER NOT NULL,
    external_key TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (external_entity_type_id) REFERENCES external_entity_types(id),
    UNIQUE (external_entity_type_id, external_key)
);

CREATE TABLE event_types (
    event_type_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE event_type_mutations (
    id INTEGER PRIMARY KEY,
    event_type_id TEXT NOT NULL,
    mutation_index INTEGER NOT NULL,
    operation TEXT NOT NULL,
    tag_definition_id INTEGER NOT NULL,
    input_key TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (event_type_id) REFERENCES event_types(event_type_id),
    FOREIGN KEY (tag_definition_id) REFERENCES tag_definitions(id),
    UNIQUE (event_type_id, mutation_index)
);

CREATE TABLE asset_current_tag_values (
    asset_id INTEGER NOT NULL,
    tag_definition_id INTEGER NOT NULL,
    value_json TEXT NOT NULL,
    enum_option_id INTEGER,
    external_entity_id INTEGER,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (asset_id, tag_definition_id),
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_definition_id) REFERENCES tag_definitions(id),
    FOREIGN KEY (enum_option_id) REFERENCES tag_enum_options(id),
    FOREIGN KEY (external_entity_id) REFERENCES external_entities(id)
);

CREATE TABLE asset_events (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL,
    idempotency_key TEXT NOT NULL,
    event_type_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (event_type_id) REFERENCES event_types(event_type_id),
    UNIQUE (asset_id, idempotency_key)
);

CREATE TABLE tag_generator_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    prefix TEXT NOT NULL DEFAULT '',
    number_width INTEGER NOT NULL DEFAULT 4,
    separator TEXT NOT NULL DEFAULT '-',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO tag_generator_settings (id) VALUES (1);

CREATE TABLE tag_generator_counters (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_value INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO tag_generator_counters (id) VALUES (1);
