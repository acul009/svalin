ALTER TABLE sessions ADD COLUMN expires_at INTEGER;
CREATE INDEX sessions_expires_at_idx ON sessions(expires_at);
