-- Existing queued messages retain NULL; new messages include their MLS group ID.
ALTER TABLE mls_messages ADD COLUMN group_id BLOB;

CREATE INDEX mls_messages_group_id_idx ON mls_messages(group_id);
