
CREATE TABLE system_reports (
    spki_hash BLOB NOT NULL PRIMARY KEY,
    generated_at INTEGER NOT NULL,
    report BLOB NOT NULL
);
