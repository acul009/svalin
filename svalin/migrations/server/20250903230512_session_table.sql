CREATE TABLE sessions (
    fingerprint BLOB PRIMARY KEY,
    issuer TEXT NOT NULL,
    certificate BLOB NOT NULL
);
