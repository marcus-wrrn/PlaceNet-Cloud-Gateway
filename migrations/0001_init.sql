-- Valid Hamlet login credentials, seeded out of band via the admin CLI.
CREATE TABLE IF NOT EXISTS credentials (
    username   TEXT PRIMARY KEY,
    pw_hash    TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- Stable device record assigned to a credential on first login.
-- The MQTT password is NOT stored here: it is rotated on each login and
-- pushed straight into the broker's dynamic-security state.
CREATE TABLE IF NOT EXISTS devices (
    device_id     TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE REFERENCES credentials(username),
    mqtt_username TEXT NOT NULL UNIQUE,
    created_at    INTEGER NOT NULL
);
