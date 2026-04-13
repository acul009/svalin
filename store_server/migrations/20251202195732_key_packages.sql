CREATE TABLE key_packages (
    id TEXT PRIMARY KEY,
    spki_hash BLOB NOT NULL,
    data BLOB NOT NULL
);
