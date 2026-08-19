CREATE TABLE mls_messages (
    id BLOB PRIMARY KEY NOT NULL,
    data BLOB NOT NULL,
    received_at INTEGER NOT NULL
);

CREATE TABLE mls_message_receivers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id BLOB NOT NULL,
    spki_hash BLOB NOT NULL,
    FOREIGN KEY (message_id) REFERENCES mls_messages(id)
);

CREATE INDEX mls_message_receivers_message_idx ON mls_message_receivers(message_id);
CREATE INDEX mls_message_receivers_spki_idx ON mls_message_receivers(spki_hash);
