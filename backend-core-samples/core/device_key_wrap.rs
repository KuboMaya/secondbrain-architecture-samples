//! Portfolio sample — X25519 ephemeral wrap for sync master key distribution.
//! Blob: version(1) || eph_pub(32) || nonce(12) || ciphertext  → Base64.
//! Matches PWA `syncCrypto.ts` unwrap (same KDF label).
//! Source: excerpt from `src-tauri/src/core/crypto.rs`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const X25519_LEN: usize = 32;
const WRAP_VERSION: u8 = 1;

/// Wrap a 32-byte master key for a recipient X25519 public key.
pub fn wrap_master_key(
    recipient_x25519_public_b64: &str,
    master_key: &[u8; KEY_LEN],
) -> Result<String, String> {
    let recipient_bytes = decode_fixed_32(recipient_x25519_public_b64, "recipient x25519 public")?;
    let recipient = X25519Public::from(recipient_bytes);

    let mut rng = OsRng;
    let eph_secret = StaticSecret::random_from_rng(&mut rng);
    let eph_public = X25519Public::from(&eph_secret);
    let shared = eph_secret.diffie_hellman(&recipient);
    let aes_key = derive_wrap_key(shared.as_bytes());

    let mut body = encrypt_aes_gcm(&aes_key, master_key)?;
    let mut out = Vec::with_capacity(1 + X25519_LEN + body.len());
    out.push(WRAP_VERSION);
    out.extend_from_slice(eph_public.as_bytes());
    out.append(&mut body);
    Ok(BASE64.encode(out))
}

/// Unwrap a master-key blob with this device's X25519 secret.
pub fn unwrap_master_key(
    my_x25519_secret_b64: &str,
    wrapped_b64: &str,
) -> Result<[u8; KEY_LEN], String> {
    let my_secret_bytes = decode_fixed_32(my_x25519_secret_b64, "x25519 secret")?;
    let my_secret = StaticSecret::from(my_secret_bytes);
    let blob = BASE64
        .decode(wrapped_b64.trim())
        .map_err(|e| format!("invalid key base64: {e}"))?;
    if blob.len() < 1 + X25519_LEN + NONCE_LEN + 1 {
        return Err("wrapped master key too short".into());
    }
    if blob[0] != WRAP_VERSION {
        return Err(format!("unsupported wrap version {}", blob[0]));
    }
    let mut eph_bytes = [0u8; X25519_LEN];
    eph_bytes.copy_from_slice(&blob[1..1 + X25519_LEN]);
    let eph_public = X25519Public::from(eph_bytes);
    let shared = my_secret.diffie_hellman(&eph_public);
    let aes_key = derive_wrap_key(shared.as_bytes());
    let plain = decrypt_aes_gcm(&aes_key, &blob[1 + X25519_LEN..])?;
    if plain.len() != KEY_LEN {
        return Err(format!("unwrapped master key length {}", plain.len()));
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&plain);
    Ok(key)
}

fn derive_wrap_key(shared_secret: &[u8]) -> [u8; KEY_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(b"secondbrain-device-wrap-v1");
    hasher.update(shared_secret);
    let digest = hasher.finalize();
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&digest);
    key
}

fn decode_fixed_32(encoded: &str, label: &str) -> Result<[u8; 32], String> {
    let bytes = BASE64
        .decode(encoded.trim())
        .map_err(|e| format!("invalid key base64: {e}"))?;
    if bytes.len() != 32 {
        return Err(format!("{label} must be 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn encrypt_aes_gcm(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut out = nonce_bytes.to_vec();
    out.extend(
        cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| e.to_string())?,
    );
    Ok(out)
}

fn decrypt_aes_gcm(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < NONCE_LEN {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| e.to_string())
}
