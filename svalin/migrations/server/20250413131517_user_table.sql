CREATE TABLE users (
    fingerprint BLOB PRIMARY KEY,
    spki_hash BLOB NOT NULL UNIQUE,
    username TEXT NOT NULL UNIQUE,
    data BLOB NOT NULL
);
