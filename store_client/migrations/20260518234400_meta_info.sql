

CREATE TABLE meta_info (
    spki_hash BLOB NOT NULL PRIMARY KEY,
    updated_at INTEGER NOT NULL,
    data BLOB NOT NULL
);
