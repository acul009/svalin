-- Add migration script here
CREATE TABLE agents (fingerprint BLOB PRIMARY KEY, data BLOB NOT NULL);
