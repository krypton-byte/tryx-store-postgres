//! Standalone integration tests for tryx-postgres-rs.
//!
//! These tests validate the .so works correctly WITHOUT Tryx.
//! They test the PostgreSQL operations directly through the Rust API.
//!
//! Requirements:
//!   - PostgreSQL running locally
//!   - Database "tryx_test" (or set PG_DSN env var)
//!
//! Run:
//!   PG_DSN="host=localhost dbname=tryx_test user=postgres" cargo test

use std::ffi::CString;
use std::ptr;

// We test the PgStore directly (not via FFI) for unit-level confidence.
#[cfg(test)]
mod pg_tests {
    use super::*;

    fn test_dsn() -> String {
        std::env::var("PG_DSN").unwrap_or_else(|_| {
            "host=localhost port=5432 dbname=tryx_test user=postgres password= sslmode=disable"
                .to_string()
        })
    }

    #[tokio::test]
    async fn test_connect() {
        let dsn = test_dsn();
        let result = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3).await;
        assert!(result.is_ok(), "Failed to connect: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_identity_crud() {
        let dsn = test_dsn();
        let store = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3)
            .await
            .expect("connect");

        let addr = "test-identity@s.whatsapp.net";
        let key = vec![0xAA; 32];

        // Put
        store.put_identity(addr, &key).await.expect("put");

        // Load
        let loaded = store.load_identity(addr).await.expect("load");
        assert_eq!(loaded, Some(key.clone()));

        // Delete
        store.delete_identity(addr).await.expect("delete");

        // Verify deleted
        let loaded = store.load_identity(addr).await.expect("load after delete");
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_session_crud() {
        let dsn = test_dsn();
        let store = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3)
            .await
            .expect("connect");

        let addr = "test-session@s.whatsapp.net";
        let data = b"session-record-data".to_vec();

        store.put_session(addr, &data).await.expect("put");

        let loaded = store.get_session(addr).await.expect("get");
        assert_eq!(loaded, Some(data));

        store.delete_session(addr).await.expect("delete");

        let loaded = store.get_session(addr).await.expect("get after delete");
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_prekey_crud() {
        let dsn = test_dsn();
        let store = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3)
            .await
            .expect("connect");

        let pk_data = b"prekey-record".to_vec();

        store
            .store_prekey(100, &pk_data, false)
            .await
            .expect("store");

        let loaded = store.load_prekey(100).await.expect("load");
        assert_eq!(loaded, Some(pk_data));

        let max_id = store.get_max_prekey_id().await.expect("max_id");
        assert!(max_id >= 100);

        store.remove_prekey(100).await.expect("remove");
    }

    #[tokio::test]
    async fn test_device_lifecycle() {
        let dsn = test_dsn();
        let store = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3)
            .await
            .expect("connect");

        let id = store.create_device().await.expect("create");
        assert_eq!(id, 1); // default device_id is 1

        let exists = store.device_exists().await.expect("exists");
        assert!(exists);

        let device_data = b"device-blob-data".to_vec();
        store.save_device(&device_data).await.expect("save");

        let loaded = store.load_device().await.expect("load");
        assert_eq!(loaded, Some(device_data));
    }

    #[tokio::test]
    async fn test_dispatch_sender_key_devices() {
        let dsn = test_dsn();
        let store = tryx_postgres::pg::PgStore::connect(&dsn, 1, 3)
            .await
            .expect("connect");

        // Set sender key status
        let input = serde_json::json!({
            "group_jid": "test-group@g.us",
            "entries": [["device1@s.whatsapp.net", true], ["device2@s.whatsapp.net", false]]
        });
        let input_bytes = serde_json::to_vec(&input).unwrap();
        store
            .dispatch(31, &input_bytes)
            .await
            .expect("set_sender_key_status");

        // Get sender key devices
        let input = serde_json::json!({"group_jid": "test-group@g.us"});
        let input_bytes = serde_json::to_vec(&input).unwrap();
        let result = store
            .dispatch(30, &input_bytes)
            .await
            .expect("get_sender_key_devices");
        assert!(result.is_some());

        // Clear
        store
            .dispatch(32, &input_bytes)
            .await
            .expect("clear_sender_key_devices");
    }
}
