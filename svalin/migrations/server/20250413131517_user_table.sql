CREATE TABLE users (
    spki_hash BLOB NOT NULL PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    data BLOB NOT NULL
);
