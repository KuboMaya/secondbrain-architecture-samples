/**
 * Portfolio sample — unwrap X25519-wrapped master key → non-extractable AES-GCM CryptoKey.
 * Wire format matches Rust `wrap_master_key`: version(1) || eph_pub(32) || nonce(12) || ct.
 * Source: adapted from `src/lib/syncCrypto.ts` (IDB helpers abbreviated).
 */

const WRAP_VERSION = 1;
const MASTER_KEY_ID = "sync_master_aes";
const WRAP_KDF_LABEL = "secondbrain-device-wrap-v1";

// In production these open IndexedDB stores `crypto_keys` / `device_meta`.
declare function idbGet<T>(store: string, key: string): Promise<T | undefined>;
declare function idbPut(store: string, key: string, value: unknown): Promise<void>;
declare function getPwaIdentity(): Promise<{ id: string; x25519_secret_b64: string } | null>;
declare function b64ToBytes(b64: string): Uint8Array;

async function deriveWrapAesKey(sharedSecret: Uint8Array): Promise<CryptoKey> {
  const enc = new TextEncoder();
  const label = enc.encode(WRAP_KDF_LABEL);
  const material = new Uint8Array(label.length + sharedSecret.length);
  material.set(label, 0);
  material.set(sharedSecret, label.length);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", material));
  return crypto.subtle.importKey("raw", digest, { name: "AES-GCM" }, false, ["decrypt"]);
}

/**
 * Unwrap master key blob; store AES-GCM CryptoKey with extractable: false.
 */
export async function unwrapAndStoreMasterKey(
  wrappedB64: string,
  x25519SecretB64?: string,
): Promise<boolean> {
  const identity = await getPwaIdentity();
  const secretB64 = x25519SecretB64 ?? identity?.x25519_secret_b64;
  if (!secretB64) throw new Error("PWA identity missing");

  const blob = b64ToBytes(wrappedB64);
  if (blob.length < 1 + 32 + 12 + 1) {
    throw new Error("wrapped master key too short");
  }
  if (blob[0] !== WRAP_VERSION) {
    throw new Error(`unsupported wrap version ${blob[0]}`);
  }
  const ephPub = blob.slice(1, 33);
  const aesBlob = blob.slice(33);

  // PKCS8 prefix + 32-byte X25519 seed (WebCrypto import).
  const privPkcs8Prefix = Uint8Array.from([
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22, 0x04, 0x20,
  ]);
  const secret = b64ToBytes(secretB64);
  if (secret.length !== 32) throw new Error("x25519 secret must be 32 bytes");
  const pkcs8 = new Uint8Array(privPkcs8Prefix.length + 32);
  pkcs8.set(privPkcs8Prefix, 0);
  pkcs8.set(secret, privPkcs8Prefix.length);

  const privateKey = await crypto.subtle.importKey(
    "pkcs8",
    pkcs8,
    { name: "X25519" },
    false,
    ["deriveBits"],
  );
  const publicKey = await crypto.subtle.importKey(
    "raw",
    ephPub,
    { name: "X25519" },
    false,
    [],
  );
  const shared = new Uint8Array(
    await crypto.subtle.deriveBits({ name: "X25519", public: publicKey }, privateKey, 256),
  );
  const wrapKey = await deriveWrapAesKey(shared);
  const nonce = aesBlob.slice(0, 12);
  const ct = aesBlob.slice(12);
  const masterRaw = new Uint8Array(
    await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce }, wrapKey, ct),
  );
  if (masterRaw.length !== 32) {
    throw new Error(`unwrapped master key length ${masterRaw.length}`);
  }

  const masterKey = await crypto.subtle.importKey(
    "raw",
    masterRaw,
    { name: "AES-GCM" },
    false, // non-extractable — cannot exfiltrate via JS after install
    ["encrypt", "decrypt"],
  );
  await idbPut("crypto_keys", MASTER_KEY_ID, masterKey);
  return true;
}
