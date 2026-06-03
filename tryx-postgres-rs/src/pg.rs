//! PostgreSQL storage engine.

use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

const SCHEMA: &str = include_str!("schema.sql");

// Opcodes for the generic `tryx_store_call` dispatch.
pub mod opcode {
    pub const GET_SENDER_KEY_DEVICES: u32 = 30;
    pub const SET_SENDER_KEY_STATUS: u32 = 31;
    pub const CLEAR_SENDER_KEY_DEVICES: u32 = 32;
    pub const CLEAR_ALL_SENDER_KEY_DEVICES: u32 = 33;
    pub const DELETE_SENDER_KEY_DEVICE_ROWS: u32 = 34;
    pub const GET_LID_MAPPING: u32 = 35;
    pub const GET_PN_MAPPING: u32 = 36;
    pub const PUT_LID_MAPPING: u32 = 37;
    pub const GET_ALL_LID_MAPPINGS: u32 = 38;
    pub const SAVE_BASE_KEY: u32 = 39;
    pub const HAS_SAME_BASE_KEY: u32 = 40;
    pub const DELETE_BASE_KEY: u32 = 41;
    pub const UPDATE_DEVICE_LIST: u32 = 42;
    pub const GET_DEVICES: u32 = 43;
    pub const DELETE_DEVICES: u32 = 44;
    pub const GET_GROUP_METADATA: u32 = 45;
    pub const PUT_GROUP_METADATA: u32 = 46;
    pub const GET_TC_TOKEN: u32 = 47;
    pub const PUT_TC_TOKEN: u32 = 48;
    pub const DELETE_TC_TOKEN: u32 = 49;
    pub const GET_ALL_TC_TOKEN_JIDS: u32 = 50;
    pub const DELETE_EXPIRED_TC_TOKENS: u32 = 51;
    pub const STORE_SENT_MESSAGE: u32 = 52;
    pub const TAKE_SENT_MESSAGE: u32 = 53;
    pub const DELETE_EXPIRED_SENT_MESSAGES: u32 = 54;
    pub const PUT_MSG_SECRETS: u32 = 60;
    pub const GET_MSG_SECRET: u32 = 61;
    pub const GET_MSG_SECRET_WITH_TS: u32 = 62;
    pub const DELETE_EXPIRED_MSG_SECRETS: u32 = 63;
    pub const PUT_MUTATION_MACS: u32 = 24;
    pub const GET_MUTATION_MAC: u32 = 25;
    pub const DELETE_MUTATION_MACS: u32 = 26;
    pub const HAS_SESSION: u32 = 7;
    pub const LOAD_ALL_SIGNED_PREKEYS: u32 = 14;
}

type PgResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct PgStore {
    pool: Pool,
    device_id: i32,
}

impl PgStore {
    pub async fn connect(dsn: &str, pool_min: usize, pool_max: usize) -> PgResult<Self> {
        let mut cfg = Config::new();
        // Parse DSN key=value pairs
        for part in dsn.split_whitespace() {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "host" => { cfg.host = Some(v.to_string()); }
                    "port" => { cfg.port = v.parse().ok(); }
                    "dbname" => { cfg.dbname = Some(v.to_string()); }
                    "user" => { cfg.user = Some(v.to_string()); }
                    "password" => { cfg.password = Some(v.to_string()); }
                    _ => {}
                }
            }
        }
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: pool_max,
            ..Default::default()
        });
        let _ = pool_min; // deadpool doesn't have min_size; kept for API compat.

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        // Run migrations
        let client = pool.get().await?;
        client.batch_execute(SCHEMA).await?;

        Ok(Self { pool, device_id: 1 })
    }

    // ── Identity ───────────────────────────────────────────────────
    pub async fn put_identity(&self, addr: &str, key: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO identities (address, key, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (address, device_id) DO UPDATE SET key = $2",
            &[&addr, &key, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn load_identity(&self, addr: &str) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT key FROM identities WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn delete_identity(&self, addr: &str) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "DELETE FROM identities WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(())
    }

    // ── Sessions ───────────────────────────────────────────────────
    pub async fn get_session(&self, addr: &str) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT record FROM sessions WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn put_session(&self, addr: &str, data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO sessions (address, record, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (address, device_id) DO UPDATE SET record = $2",
            &[&addr, &data, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn delete_session(&self, addr: &str) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "DELETE FROM sessions WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(())
    }

    // ── PreKeys ────────────────────────────────────────────────────
    pub async fn store_prekey(&self, id: u32, data: &[u8], uploaded: bool) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO prekeys (id, key, uploaded, device_id) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id, device_id) DO UPDATE SET key = $2, uploaded = $3",
            &[&(id as i32), &data, &uploaded, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn load_prekey(&self, id: u32) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT key FROM prekeys WHERE id = $1 AND device_id = $2",
            &[&(id as i32), &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn remove_prekey(&self, id: u32) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "DELETE FROM prekeys WHERE id = $1 AND device_id = $2",
            &[&(id as i32), &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn get_max_prekey_id(&self) -> PgResult<u32> {
        let c = self.pool.get().await?;
        let row = c.query_one(
            "SELECT COALESCE(MAX(id), 0) FROM prekeys WHERE device_id = $1",
            &[&self.device_id],
        ).await?;
        let v: i32 = row.get(0);
        Ok(v as u32)
    }

    // ── Signed PreKeys ─────────────────────────────────────────────
    pub async fn store_signed_prekey(&self, id: u32, data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO signed_prekeys (id, record, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (id, device_id) DO UPDATE SET record = $2",
            &[&(id as i32), &data, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn load_signed_prekey(&self, id: u32) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT record FROM signed_prekeys WHERE id = $1 AND device_id = $2",
            &[&(id as i32), &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn remove_signed_prekey(&self, id: u32) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "DELETE FROM signed_prekeys WHERE id = $1 AND device_id = $2",
            &[&(id as i32), &self.device_id],
        ).await?;
        Ok(())
    }

    // ── Sender Keys ────────────────────────────────────────────────
    pub async fn put_sender_key(&self, addr: &str, data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO sender_keys (address, record, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (address, device_id) DO UPDATE SET record = $2",
            &[&addr, &data, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn get_sender_key(&self, addr: &str) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT record FROM sender_keys WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn delete_sender_key(&self, addr: &str) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "DELETE FROM sender_keys WHERE address = $1 AND device_id = $2",
            &[&addr, &self.device_id],
        ).await?;
        Ok(())
    }

    // ── AppSyncStore ───────────────────────────────────────────────
    pub async fn get_sync_key(&self, kid: &[u8]) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT key_data FROM app_state_keys WHERE key_id = $1 AND device_id = $2",
            &[&kid, &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn set_sync_key(&self, kid: &[u8], data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO app_state_keys (key_id, key_data, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (key_id, device_id) DO UPDATE SET key_data = $2",
            &[&kid, &data, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn get_version(&self, name: &str) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT state_data FROM app_state_versions WHERE name = $1 AND device_id = $2",
            &[&name, &self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn set_version(&self, name: &str, data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO app_state_versions (name, state_data, device_id) VALUES ($1, $2, $3) \
             ON CONFLICT (name, device_id) DO UPDATE SET state_data = $2",
            &[&name, &data, &self.device_id],
        ).await?;
        Ok(())
    }

    pub async fn get_latest_sync_key_id(&self) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT key_id FROM app_state_keys WHERE device_id = $1 ORDER BY key_id DESC LIMIT 1",
            &[&self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    // ── DeviceStore ────────────────────────────────────────────────
    pub async fn save_device(&self, data: &[u8]) -> PgResult<()> {
        let c = self.pool.get().await?;
        // Device blob is stored as-is; karat handles serialization.
        c.execute(
            "INSERT INTO device (id, noise_key) VALUES ($1, $2) \
             ON CONFLICT (id) DO UPDATE SET noise_key = $2",
            &[&self.device_id, &data],
        ).await?;
        Ok(())
    }

    pub async fn load_device(&self) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT noise_key FROM device WHERE id = $1",
            &[&self.device_id],
        ).await?;
        Ok(row.map(|r| r.get(0)))
    }

    pub async fn device_exists(&self) -> PgResult<bool> {
        let c = self.pool.get().await?;
        let row = c.query_one(
            "SELECT COUNT(*) FROM device WHERE id = $1",
            &[&self.device_id],
        ).await?;
        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    pub async fn create_device(&self) -> PgResult<i32> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO device (id) VALUES ($1) ON CONFLICT DO NOTHING",
            &[&self.device_id],
        ).await?;
        Ok(self.device_id)
    }

    // ── Generic dispatch for remaining ProtocolStore/MsgSecretStore ops ──
    pub async fn dispatch(&self, op: u32, input: &[u8]) -> PgResult<Option<Vec<u8>>> {
        let c = self.pool.get().await?;
        let args: serde_json::Value = if input.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(input)?
        };

        match op {
            opcode::PUT_MUTATION_MACS => {
                let name = args["name"].as_str().unwrap_or("");
                let version = args["version"].as_i64().unwrap_or(0);
                let macs = args["macs"].as_array();
                if let Some(macs) = macs {
                    for m in macs {
                        let idx: Vec<u8> = serde_json::from_value(m["index_mac"].clone())?;
                        let val: Vec<u8> = serde_json::from_value(m["value_mac"].clone())?;
                        c.execute(
                            "INSERT INTO app_state_mutation_macs (name, version, index_mac, value_mac, device_id) \
                             VALUES ($1, $2, $3, $4, $5) \
                             ON CONFLICT (name, index_mac, device_id) DO UPDATE SET version = $2, value_mac = $4",
                            &[&name, &version, &idx.as_slice(), &val.as_slice(), &self.device_id],
                        ).await?;
                    }
                }
                Ok(None)
            }
            opcode::GET_MUTATION_MAC => {
                let name = args["name"].as_str().unwrap_or("");
                let idx: Vec<u8> = serde_json::from_value(args["index_mac"].clone())?;
                let row = c.query_opt(
                    "SELECT value_mac FROM app_state_mutation_macs WHERE name = $1 AND index_mac = $2 AND device_id = $3",
                    &[&name, &idx.as_slice(), &self.device_id],
                ).await?;
                Ok(row.map(|r| r.get::<_, Vec<u8>>(0)))
            }
            opcode::DELETE_MUTATION_MACS => {
                let name = args["name"].as_str().unwrap_or("");
                let macs: Vec<Vec<u8>> = serde_json::from_value(args["index_macs"].clone())?;
                for mac in &macs {
                    c.execute(
                        "DELETE FROM app_state_mutation_macs WHERE name = $1 AND index_mac = $2 AND device_id = $3",
                        &[&name, &mac.as_slice(), &self.device_id],
                    ).await?;
                }
                Ok(None)
            }
            opcode::GET_SENDER_KEY_DEVICES => {
                let gjid = args["group_jid"].as_str().unwrap_or("");
                let rows = c.query(
                    "SELECT device_jid, has_key FROM sender_key_devices WHERE group_jid = $1 AND device_id = $2",
                    &[&gjid, &self.device_id],
                ).await?;
                let result: Vec<(String, bool)> = rows.iter()
                    .map(|r| (r.get::<_, String>(0), r.get::<_, i32>(1) != 0))
                    .collect();
                Ok(Some(serde_json::to_vec(&result)?))
            }
            opcode::SET_SENDER_KEY_STATUS => {
                let gjid = args["group_jid"].as_str().unwrap_or("");
                let entries: Vec<(String, bool)> = serde_json::from_value(args["entries"].clone())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                for (djid, has) in &entries {
                    c.execute(
                        "INSERT INTO sender_key_devices (group_jid, device_jid, has_key, device_id, updated_at) \
                         VALUES ($1, $2, $3, $4, $5) \
                         ON CONFLICT (group_jid, device_jid, device_id) DO UPDATE SET has_key = $3, updated_at = $5",
                        &[&gjid, &djid.as_str(), &(*has as i32), &self.device_id, &now],
                    ).await?;
                }
                Ok(None)
            }
            opcode::CLEAR_SENDER_KEY_DEVICES => {
                let gjid = args["group_jid"].as_str().unwrap_or("");
                c.execute(
                    "DELETE FROM sender_key_devices WHERE group_jid = $1 AND device_id = $2",
                    &[&gjid, &self.device_id],
                ).await?;
                Ok(None)
            }
            opcode::CLEAR_ALL_SENDER_KEY_DEVICES => {
                c.execute(
                    "DELETE FROM sender_key_devices WHERE device_id = $1",
                    &[&self.device_id],
                ).await?;
                Ok(None)
            }
            opcode::GET_LID_MAPPING => {
                let lid = args["lid"].as_str().unwrap_or("");
                let row = c.query_opt(
                    "SELECT lid, phone_number, created_at, learning_source, updated_at FROM lid_pn_mapping \
                     WHERE lid = $1 AND device_id = $2",
                    &[&lid, &self.device_id],
                ).await?;
                match row {
                    Some(r) => {
                        let v = serde_json::json!({
                            "lid": r.get::<_, String>(0),
                            "phone_number": r.get::<_, String>(1),
                            "created_at": r.get::<_, i64>(2),
                            "learning_source": r.get::<_, String>(3),
                            "updated_at": r.get::<_, i64>(4),
                        });
                        Ok(Some(serde_json::to_vec(&v)?))
                    }
                    None => Ok(None),
                }
            }
            opcode::PUT_LID_MAPPING => {
                let lid = args["lid"].as_str().unwrap_or("");
                let pn = args["phone_number"].as_str().unwrap_or("");
                let ca = args["created_at"].as_i64().unwrap_or(0);
                let ls = args["learning_source"].as_str().unwrap_or("");
                let ua = args["updated_at"].as_i64().unwrap_or(0);
                c.execute(
                    "INSERT INTO lid_pn_mapping (lid, phone_number, created_at, learning_source, updated_at, device_id) \
                     VALUES ($1, $2, $3, $4, $5, $6) \
                     ON CONFLICT (lid, device_id) DO UPDATE SET phone_number = $2, learning_source = $4, updated_at = $5",
                    &[&lid, &pn, &ca, &ls, &ua, &self.device_id],
                ).await?;
                Ok(None)
            }
            opcode::STORE_SENT_MESSAGE => {
                let chat = args["chat_jid"].as_str().unwrap_or("");
                let mid = args["message_id"].as_str().unwrap_or("");
                let payload: Vec<u8> = serde_json::from_value(args["payload"].clone())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                c.execute(
                    "INSERT INTO sent_messages (chat_jid, message_id, payload, device_id, created_at) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT (chat_jid, message_id, device_id) DO UPDATE SET payload = $3, created_at = $5",
                    &[&chat, &mid, &payload.as_slice(), &self.device_id, &now],
                ).await?;
                Ok(None)
            }
            opcode::TAKE_SENT_MESSAGE => {
                let chat = args["chat_jid"].as_str().unwrap_or("");
                let mid = args["message_id"].as_str().unwrap_or("");
                let row = c.query_opt(
                    "DELETE FROM sent_messages WHERE chat_jid = $1 AND message_id = $2 AND device_id = $3 RETURNING payload",
                    &[&chat, &mid, &self.device_id],
                ).await?;
                Ok(row.map(|r| r.get::<_, Vec<u8>>(0)))
            }
            opcode::PUT_MSG_SECRETS => {
                let entries: Vec<serde_json::Value> = serde_json::from_value(args["entries"].clone())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                for e in &entries {
                    let chat = e["chat"].as_str().unwrap_or("");
                    let sender = e["sender"].as_str().unwrap_or("");
                    let msg_id = e["msg_id"].as_str().unwrap_or("");
                    let secret: Vec<u8> = serde_json::from_value(e["secret"].clone())?;
                    let expires = e["expires_at"].as_i64().unwrap_or(0);
                    let mts = e["message_ts"].as_i64().unwrap_or(0);
                    c.execute(
                        "INSERT INTO msg_secrets (chat, sender, msg_id, secret, device_id, created_at, expires_at, message_ts) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                         ON CONFLICT (chat, sender, msg_id, device_id) DO UPDATE SET \
                         secret = $4, created_at = $6, \
                         expires_at = CASE WHEN msg_secrets.expires_at = 0 OR $7 = 0 THEN 0 \
                                      ELSE GREATEST(msg_secrets.expires_at, $7) END, \
                         message_ts = GREATEST(msg_secrets.message_ts, $8)",
                        &[&chat, &sender, &msg_id, &secret.as_slice(), &self.device_id, &now, &expires, &mts],
                    ).await?;
                }
                let count = entries.len() as i64;
                Ok(Some(count.to_le_bytes().to_vec()))
            }
            opcode::GET_MSG_SECRET => {
                let chat = args["chat"].as_str().unwrap_or("");
                let sender = args["sender"].as_str().unwrap_or("");
                let msg_id = args["msg_id"].as_str().unwrap_or("");
                let row = c.query_opt(
                    "SELECT secret FROM msg_secrets WHERE chat = $1 AND sender = $2 AND msg_id = $3 AND device_id = $4",
                    &[&chat, &sender, &msg_id, &self.device_id],
                ).await?;
                Ok(row.map(|r| r.get::<_, Vec<u8>>(0)))
            }
            opcode::GET_MSG_SECRET_WITH_TS => {
                let chat = args["chat"].as_str().unwrap_or("");
                let sender = args["sender"].as_str().unwrap_or("");
                let msg_id = args["msg_id"].as_str().unwrap_or("");
                let row = c.query_opt(
                    "SELECT secret, message_ts FROM msg_secrets WHERE chat = $1 AND sender = $2 AND msg_id = $3 AND device_id = $4",
                    &[&chat, &sender, &msg_id, &self.device_id],
                ).await?;
                match row {
                    Some(r) => {
                        let secret: Vec<u8> = r.get(0);
                        let ts: i64 = r.get(1);
                        let v = serde_json::json!({"secret": secret, "message_ts": ts});
                        Ok(Some(serde_json::to_vec(&v)?))
                    }
                    None => Ok(None),
                }
            }
            opcode::DELETE_EXPIRED_MSG_SECRETS => {
                let cutoff = args["cutoff"].as_i64().unwrap_or(0);
                let row = c.query_one(
                    "WITH d AS (DELETE FROM msg_secrets WHERE expires_at > 0 AND expires_at <= $1 AND device_id = $2 RETURNING 1) SELECT COUNT(*) FROM d",
                    &[&cutoff, &self.device_id],
                ).await?;
                let count: i64 = row.get(0);
                Ok(Some((count as u32).to_le_bytes().to_vec()))
            }
            _ => Ok(None),
        }
    }
}
