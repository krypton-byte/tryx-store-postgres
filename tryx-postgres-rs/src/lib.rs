//! Tryx Store FFI — PostgreSQL implementation.
//!
//! Exports a standardized C ABI surface (`tryx_store_*`) that any storage
//! backend (Postgres, Mongo, etc.) can implement. Karat loads this `.so`
//! at runtime via `libloading`.

pub mod pg;

use std::ffi::{CStr, c_char, c_void};
use std::ptr;
use std::sync::OnceLock;

// ── FFI buffer type ────────────────────────────────────────────────
#[repr(C)]
pub struct TryxBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl TryxBuffer {
    fn null() -> Self { Self { data: ptr::null_mut(), len: 0 } }
    fn from_vec(v: Vec<u8>) -> Self {
        let mut v = v.into_boxed_slice();
        let buf = Self { data: v.as_mut_ptr(), len: v.len() };
        std::mem::forget(v);
        buf
    }
}

// ── Helpers ────────────────────────────────────────────────────────
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

unsafe fn cstr(p: *const c_char) -> &'static str {
    unsafe { CStr::from_ptr(p).to_str().unwrap_or("") }
}

unsafe fn slice(p: *const u8, len: usize) -> &'static [u8] {
    if p.is_null() || len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(p, len) } }
}

macro_rules! handle {
    ($h:expr) => {
        unsafe { &*(($h) as *const pg::PgStore) }
    };
}

// ── Lifecycle ──────────────────────────────────────────────────────
/// Connect to PostgreSQL. `config_json`: `{"dsn":"...","pool_min":2,"pool_max":10}`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_connect(
    config_json: *const c_char,
    out: *mut *mut c_void,
) -> i32 {
    let cfg = unsafe { cstr(config_json) };
    let config: serde_json::Value = match serde_json::from_str(cfg) {
        Ok(v) => v,
        Err(_) => return -1,
    };
    let dsn = config["dsn"].as_str().unwrap_or("");
    let pool_min = config["pool_min"].as_u64().unwrap_or(2) as usize;
    let pool_max = config["pool_max"].as_u64().unwrap_or(10) as usize;

    match runtime().block_on(pg::PgStore::connect(dsn, pool_min, pool_max)) {
        Ok(store) => {
            let boxed = Box::new(store);
            unsafe { *out = Box::into_raw(boxed) as *mut c_void };
            0
        }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_destroy(h: *mut c_void) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h as *mut pg::PgStore)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_free_buffer(buf: TryxBuffer) {
    if !buf.data.is_null() && buf.len > 0 {
        unsafe { drop(Vec::from_raw_parts(buf.data, buf.len, buf.len)) };
    }
}

// ── SignalStore: Identity ──────────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_put_identity(
    h: *mut c_void, addr: *const c_char, key: *const u8, key_len: usize,
) -> i32 {
    let s = handle!(h);
    let addr = unsafe { cstr(addr) };
    let key = unsafe { slice(key, key_len) };
    match runtime().block_on(s.put_identity(addr, key)) { Ok(_) => 0, Err(_) => -1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_load_identity(
    h: *mut c_void, addr: *const c_char, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    let addr = unsafe { cstr(addr) };
    match runtime().block_on(s.load_identity(addr)) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_delete_identity(
    h: *mut c_void, addr: *const c_char,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.delete_identity(unsafe { cstr(addr) })) { Ok(_) => 0, Err(_) => -1 }
}

// ── SignalStore: Sessions ──────────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_session(
    h: *mut c_void, addr: *const c_char, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_session(unsafe { cstr(addr) })) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_put_session(
    h: *mut c_void, addr: *const c_char, data: *const u8, len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.put_session(unsafe { cstr(addr) }, unsafe { slice(data, len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_delete_session(
    h: *mut c_void, addr: *const c_char,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.delete_session(unsafe { cstr(addr) })) { Ok(_) => 0, Err(_) => -1 }
}

// ── SignalStore: PreKeys ───────────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_store_prekey(
    h: *mut c_void, id: u32, data: *const u8, len: usize, uploaded: i32,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.store_prekey(id, unsafe { slice(data, len) }, uploaded != 0)) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_load_prekey(
    h: *mut c_void, id: u32, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.load_prekey(id)) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_remove_prekey(h: *mut c_void, id: u32) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.remove_prekey(id)) { Ok(_) => 0, Err(_) => -1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_max_prekey_id(h: *mut c_void, out: *mut u32) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_max_prekey_id()) {
        Ok(v) => { unsafe { *out = v }; 0 }
        Err(_) => -1,
    }
}

// ── SignalStore: Signed PreKeys ────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_store_signed_prekey(
    h: *mut c_void, id: u32, data: *const u8, len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.store_signed_prekey(id, unsafe { slice(data, len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_load_signed_prekey(
    h: *mut c_void, id: u32, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.load_signed_prekey(id)) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_remove_signed_prekey(h: *mut c_void, id: u32) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.remove_signed_prekey(id)) { Ok(_) => 0, Err(_) => -1 }
}

// ── SignalStore: Sender Keys ───────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_put_sender_key(
    h: *mut c_void, addr: *const c_char, data: *const u8, len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.put_sender_key(unsafe { cstr(addr) }, unsafe { slice(data, len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_sender_key(
    h: *mut c_void, addr: *const c_char, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_sender_key(unsafe { cstr(addr) })) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_delete_sender_key(
    h: *mut c_void, addr: *const c_char,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.delete_sender_key(unsafe { cstr(addr) })) { Ok(_) => 0, Err(_) => -1 }
}

// ── AppSyncStore ───────────────────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_sync_key(
    h: *mut c_void, kid: *const u8, kid_len: usize, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_sync_key(unsafe { slice(kid, kid_len) })) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_set_sync_key(
    h: *mut c_void, kid: *const u8, kid_len: usize, data: *const u8, data_len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.set_sync_key(unsafe { slice(kid, kid_len) }, unsafe { slice(data, data_len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_version(
    h: *mut c_void, name: *const c_char, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_version(unsafe { cstr(name) })) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_set_version(
    h: *mut c_void, name: *const c_char, data: *const u8, len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.set_version(unsafe { cstr(name) }, unsafe { slice(data, len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_get_latest_sync_key_id(
    h: *mut c_void, out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.get_latest_sync_key_id()) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

// ── DeviceStore ────────────────────────────────────────────────────
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_save_device(
    h: *mut c_void, data: *const u8, len: usize,
) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.save_device(unsafe { slice(data, len) })) {
        Ok(_) => 0, Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_load_device(h: *mut c_void, out: *mut TryxBuffer) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.load_device()) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 1 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_device_exists(h: *mut c_void, out: *mut i32) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.device_exists()) {
        Ok(b) => { unsafe { *out = b as i32 }; 0 }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_create_device(h: *mut c_void, out: *mut i32) -> i32 {
    let s = handle!(h);
    match runtime().block_on(s.create_device()) {
        Ok(id) => { unsafe { *out = id }; 0 }
        Err(_) => -1,
    }
}

// ── ProtocolStore: bulk operations via JSON-serialized payloads ────
/// Generic opcode-based call for complex ProtocolStore/MsgSecretStore methods.
/// `opcode` values: see `pg::Opcode`. Input/output are JSON-serialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tryx_store_call(
    h: *mut c_void, opcode: u32,
    input: *const u8, input_len: usize,
    out: *mut TryxBuffer,
) -> i32 {
    let s = handle!(h);
    let input = unsafe { slice(input, input_len) };
    match runtime().block_on(s.dispatch(opcode, input)) {
        Ok(Some(v)) => { unsafe { *out = TryxBuffer::from_vec(v) }; 0 }
        Ok(None) => { unsafe { *out = TryxBuffer::null() }; 0 }
        Err(_) => -1,
    }
}
