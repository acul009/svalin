CREATE TABLE sessions (
    spki_hash BLOB PRIMARY KEY,
    issuer BLOB NOT NULL,
    certificate BLOB NOT NULL
);
