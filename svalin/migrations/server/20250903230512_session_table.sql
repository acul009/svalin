CREATE TABLE sessions (
    fingerprint BLOB PRIMARY KEY,
    issuer BLOB NOT NULL,
    certificate BLOB NOT NULL
);
