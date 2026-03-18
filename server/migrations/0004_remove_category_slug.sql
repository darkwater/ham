CREATE TABLE categories_v2 (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    parent_category_id INTEGER REFERENCES categories(id)
);

INSERT INTO categories_v2 (id, name, created_at, parent_category_id)
SELECT id, name, created_at, parent_category_id
FROM categories;

DROP TABLE categories;

ALTER TABLE categories_v2 RENAME TO categories;
