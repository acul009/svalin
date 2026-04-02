CREATE TABLE key_packages (
    id TEXT PRIMARY KEY,
    owner_spki_hash BLOB NOT NULL,
    user_spki_hash BLOB NOT NULL,
    data BLOB NOT NULL
);
