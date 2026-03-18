ALTER TABLE categories ADD COLUMN parent_category_id INTEGER REFERENCES categories(id);

ALTER TABLE assets ADD COLUMN deleted_at TEXT;

ALTER TABLE tag_enum_options ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1;
