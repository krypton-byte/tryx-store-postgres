-- Tryx Store: PostgreSQL schema (ported from SQLite)
-- All tables use device_id for multi-device isolation.

CREATE TABLE IF NOT EXISTS device (
    id              INTEGER PRIMARY KEY,
    lid             TEXT NOT NULL DEFAULT '',
    pn              TEXT NOT NULL DEFAULT '',
    registration_id INTEGER NOT NULL DEFAULT 0,
    noise_key       BYTEA NOT NULL DEFAULT '',
    identity_key    BYTEA NOT NULL DEFAULT '',
    signed_pre_key  BYTEA NOT NULL DEFAULT '',
    signed_pre_key_id INTEGER NOT NULL DEFAULT 0,
    signed_pre_key_signature BYTEA NOT NULL DEFAULT '',
    adv_secret_key  BYTEA NOT NULL DEFAULT '',
    account         BYTEA,
    push_name       TEXT NOT NULL DEFAULT '',
    app_version_primary INTEGER NOT NULL DEFAULT 0,
    app_version_secondary INTEGER NOT NULL DEFAULT 0,
    app_version_tertiary BIGINT NOT NULL DEFAULT 0,
    app_version_last_fetched_ms BIGINT NOT NULL DEFAULT 0,
    edge_routing_info BYTEA,
    props_hash      TEXT,
    next_pre_key_id INTEGER NOT NULL DEFAULT 0,
    nct_salt        BYTEA,
    server_has_prekeys BOOLEAN NOT NULL DEFAULT FALSE,
    server_cert_chain BYTEA,
    login_counter   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS identities (
    address    TEXT NOT NULL,
    key        BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    address    TEXT NOT NULL,
    record     BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE IF NOT EXISTS prekeys (
    id         INTEGER NOT NULL,
    key        BYTEA NOT NULL,
    uploaded   BOOLEAN NOT NULL DEFAULT FALSE,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (id, device_id)
);

CREATE TABLE IF NOT EXISTS signed_prekeys (
    id         INTEGER NOT NULL,
    record     BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (id, device_id)
);

CREATE TABLE IF NOT EXISTS sender_keys (
    address    TEXT NOT NULL,
    record     BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE IF NOT EXISTS app_state_keys (
    key_id     BYTEA NOT NULL,
    key_data   BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (key_id, device_id)
);

CREATE TABLE IF NOT EXISTS app_state_versions (
    name       TEXT NOT NULL,
    state_data BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (name, device_id)
);

CREATE TABLE IF NOT EXISTS app_state_mutation_macs (
    name       TEXT NOT NULL,
    version    BIGINT NOT NULL,
    index_mac  BYTEA NOT NULL,
    value_mac  BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    PRIMARY KEY (name, index_mac, device_id)
);

CREATE TABLE IF NOT EXISTS sender_key_devices (
    group_jid  TEXT NOT NULL,
    device_jid TEXT NOT NULL,
    has_key    INTEGER NOT NULL DEFAULT 0,
    device_id  INTEGER NOT NULL,
    updated_at BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (group_jid, device_jid, device_id)
);

CREATE TABLE IF NOT EXISTS lid_pn_mapping (
    lid             TEXT NOT NULL,
    phone_number    TEXT NOT NULL,
    created_at      BIGINT NOT NULL DEFAULT 0,
    learning_source TEXT NOT NULL DEFAULT '',
    updated_at      BIGINT NOT NULL DEFAULT 0,
    device_id       INTEGER NOT NULL,
    PRIMARY KEY (lid, device_id)
);

CREATE TABLE IF NOT EXISTS base_keys (
    address    TEXT NOT NULL,
    message_id TEXT NOT NULL,
    base_key   BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (address, message_id, device_id)
);

CREATE TABLE IF NOT EXISTS device_registry (
    user_id      TEXT NOT NULL,
    devices_json TEXT NOT NULL,
    timestamp    INTEGER NOT NULL DEFAULT 0,
    phash        TEXT,
    device_id    INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL DEFAULT 0,
    raw_id       INTEGER,
    PRIMARY KEY (user_id, device_id)
);

CREATE TABLE IF NOT EXISTS group_metadata (
    group_jid  TEXT NOT NULL,
    info       BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    updated_at BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (group_jid, device_id)
);

CREATE TABLE IF NOT EXISTS tc_tokens (
    jid              TEXT NOT NULL,
    token            BYTEA NOT NULL,
    token_timestamp  BIGINT NOT NULL DEFAULT 0,
    sender_timestamp BIGINT,
    device_id        INTEGER NOT NULL,
    updated_at       BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (jid, device_id)
);

CREATE TABLE IF NOT EXISTS sent_messages (
    chat_jid   TEXT NOT NULL,
    message_id TEXT NOT NULL,
    payload    BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    created_at BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (chat_jid, message_id, device_id)
);

CREATE TABLE IF NOT EXISTS msg_secrets (
    chat       TEXT NOT NULL,
    sender     TEXT NOT NULL,
    msg_id     TEXT NOT NULL,
    secret     BYTEA NOT NULL,
    device_id  INTEGER NOT NULL,
    created_at BIGINT NOT NULL DEFAULT 0,
    expires_at BIGINT NOT NULL DEFAULT 0,
    message_ts BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (chat, sender, msg_id, device_id)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_lid_pn_phone ON lid_pn_mapping (phone_number, device_id);
CREATE INDEX IF NOT EXISTS idx_sender_key_devices_jid ON sender_key_devices (device_jid, device_id);
CREATE INDEX IF NOT EXISTS idx_tc_tokens_ts ON tc_tokens (token_timestamp, device_id);
CREATE INDEX IF NOT EXISTS idx_sent_messages_ts ON sent_messages (created_at, device_id);
CREATE INDEX IF NOT EXISTS idx_msg_secrets_expires ON msg_secrets (expires_at, device_id) WHERE expires_at > 0;
