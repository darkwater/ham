ALTER TABLE event_types ADD COLUMN current_version INTEGER NOT NULL DEFAULT 1;

CREATE TABLE event_type_mutations_v2 (
    id INTEGER PRIMARY KEY,
    event_type_id TEXT NOT NULL,
    event_type_version INTEGER NOT NULL DEFAULT 1,
    mutation_index INTEGER NOT NULL,
    operation TEXT NOT NULL,
    tag_definition_id INTEGER NOT NULL,
    input_key TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (event_type_id) REFERENCES event_types(event_type_id),
    FOREIGN KEY (tag_definition_id) REFERENCES tag_definitions(id),
    UNIQUE (event_type_id, event_type_version, mutation_index)
);

INSERT INTO event_type_mutations_v2 (
    id,
    event_type_id,
    event_type_version,
    mutation_index,
    operation,
    tag_definition_id,
    input_key,
    created_at
)
SELECT
    id,
    event_type_id,
    1,
    mutation_index,
    operation,
    tag_definition_id,
    input_key,
    created_at
FROM event_type_mutations;

DROP TABLE event_type_mutations;

ALTER TABLE event_type_mutations_v2 RENAME TO event_type_mutations;

ALTER TABLE asset_events ADD COLUMN event_type_version INTEGER NOT NULL DEFAULT 1;
