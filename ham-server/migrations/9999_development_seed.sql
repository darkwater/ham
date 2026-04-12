INSERT INTO categories (id, parent_category_id, display_name) VALUES
    (2, 1, 'Tech'),
    (3, 2, 'Computer'),
    (4, 2, 'Keyboard'),
    (5, 2, 'Mouse'),
    (6, 2, 'Monitor'),
    (7, 2, 'Audio'),
    (8, 1, 'Appliance');

INSERT INTO assets (id, category_id, display_name) VALUES
    ( 1, 3, 'tetsuya'),
    ( 2, 3, 'sinon'),
    ( 3, 3, 'Steam Deck LCD'),
    ( 4, 3, 'Steam Deck OLED'),
    ( 5, 4, 'Ergodox EZ Shine'),
    ( 6, 4, 'Ergodox EZ Shine'),
    ( 7, 4, 'Keychron K3 Max-H2'),
    ( 8, 5, 'Logitech G PRO'),
    ( 9, 5, 'Attack Shark X5'),
    (10, 6, 'Gigabyte AORUS FO32U2P'),
    (11, 6, 'LG ULTRAGEAR 27GL850-B'),
    (12, 6, 'LG ULTRAGEAR 27GL850-B'),
    (13, 7, 'Antlion ModMic Wireless'),
    (14, 8, 'Fridge'),
    (15, 8, 'AEG L7WB86GW');

INSERT INTO field_definitions (id, display_name, value_type) VALUES
    (1, 'Serial', 'String'),
    (2, 'Purchase Date', 'Date'),
    (3, 'Purchase Price', 'Money'),
    (4, 'Location', 'Enum(1)'),
    (5, 'RAM GB', 'Float'),
    (6, 'Resolution', 'String'),
    (7, 'Refresh Rate', 'Integer');

INSERT INTO enum_types (type_key, display_name) VALUES
    ('location', 'Location');

INSERT INTO enum_values (id, enum_type_id, display_name, order_index) VALUES
    (1, 1, 'Desk', 0),
    (2, 1, 'TV Console', 1),
    (3, 1, 'Office', 2);

INSERT INTO category_field_hints (category_id, field_id) VALUES
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4),
    (3, 5),
    (6, 6),
    (6, 7);

INSERT INTO asset_field_values (asset_id, field_id, value) VALUES
    (1, 1, '"1234567890"'),
    (1, 2, '"2023-01-01"'),
    (1, 3, '399.99'),
    (1, 4, '1'),
    (1, 5, '32'),
    (10, 6, '"3840x2160"'),
    (10, 7, '240');

UPDATE settings SET value = 'FBK' WHERE name = 'asset_tag_prefix';
