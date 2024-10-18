-- CREATE EXTENSION timescaledb;

CREATE TABLE IF NOT EXISTS values
(
    time TIMESTAMPTZ NOT NULL,
    topic TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL
);

SELECT create_hypertable('values', 'time');