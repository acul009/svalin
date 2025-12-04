-- Add migration script here
CREATE TABLE agents (
    spki_hash BLOB PRIMARY KEY,
    data BLOB NOT NULL
);
