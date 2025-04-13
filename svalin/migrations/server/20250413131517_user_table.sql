CREATE TABLE users (
    fingerprint BLOB PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    data BLOB NOT NULL
);
