-- This is the primary event table
CREATE TABLE IF NOT EXISTS ClientEvents (
    id                  INTEGER     PRIMARY KEY     AUTOINCREMENT,
    event_value         TEXT        NOT NULL,
    details             TEXT        NOT NULL,
    snowflake           BIGINT      NOT NULL,
    level               VARCHAR(20) NOT NULL,
    caused_by_user_id   TEXT,
    instance_id         TEXT
);
