//! Portfolio sample — resilient local crypto (never abort on missing key).
//! Highlights: AES-256-GCM when key present; deterministic mock prefix otherwise.
//! Enables compile/run/test without secrets — local-first friendly.
//! Source: excerpt from `src-tauri/src/core/crypto.rs` (keypair / wrap omitted).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::RngCore;

const MOCK_PREFIX: &str = "[ENCRYPTED_MOCK]";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Encrypt plaintext. Without a usable key, returns `[ENCRYPTED_MOCK]…` (never errors).
pub fn encrypt(encryption_key: Option<&str>, plaintext: &str) -> String {
    match resolve_key(encryption_key) {
        None => format!("{MOCK_PREFIX}{plaintext}"),
        Some(key) => match encrypt_aes_gcm(&key, plaintext.as_bytes()) {
            Ok(bytes) => BASE64.encode(bytes),
            Err(err) => {
                tracing::warn!(error = %err, "aes-gcm encrypt failed; using mock");
                format!("{MOCK_PREFIX}{plaintext}")
            }
        },
    }
}

/// Decrypt ciphertext. Mock payloads unwrap without a key; AES needs a valid key.
pub fn decrypt(encryption_key: Option<&str>, ciphertext: &str) -> String {
    if let Some(plain) = ciphertext.strip_prefix(MOCK_PREFIX) {
        return plain.to_string();
    }

    match resolve_key(encryption_key) {
        None => ciphertext.to_string(),
        Some(key) => match BASE64.decode(ciphertext) {
            Ok(bytes) => match decrypt_aes_gcm(&key, &bytes) {
                Ok(plain) => String::from_utf8_lossy(&plain).into_owned(),
                Err(err) => {
                    tracing::warn!(error = %err, "aes-gcm decrypt failed");
                    ciphertext.to_string()
                }
            },
            Err(_) => ciphertext.to_string(),
        },
    }
}

fn resolve_key(encryption_key: Option<&str>) -> Option<[u8; KEY_LEN]> {
    let raw = encryption_key?.trim();
    if raw.is_empty() {
        return None;
    }
    let bytes = BASE64.decode(raw).ok()?;
    if bytes.len() != KEY_LEN {
        return None;
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&bytes);
    Some(key)
}

fn encrypt_aes_gcm(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut out = nonce_bytes.to_vec();
    out.extend(cipher.encrypt(nonce, plaintext)?);
    Ok(out)
}

fn decrypt_aes_gcm(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    if data.len() < NONCE_LEN {
        return Err(aes_gcm::Error);
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext)
}
