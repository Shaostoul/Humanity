//! Web Push, our own (2026-11-06): the payload encryption of RFC 8291 (one aes128gcm record,
//! RFC 8188) and the VAPID token of RFC 8292 (an ES256 JSON Web Token), built on the p256 crate.
//! `push_send.rs` sends what this builds; `api.rs` hands out the public half of the key.
//!
//! WHY OUR OWN, NOT A WEB-PUSH CRATE: the relay used web-push-native 0.4, which signs its VAPID
//! token through jwt-simple. jwt-simple carries `rsa` (RUSTSEC-2023-0071, the Marvin timing
//! attack, with no fixed release) although a VAPID token is ES256, never RSA, and it and
//! web-push-native (0.5 too, checked 2026-11-06) sit on elliptic-curve 0.13, whose crypto-bigint
//! 0.5 has RUSTSEC-2026-0387 (timing variability in its constant-time operations). The other
//! push crates looked at carry OpenSSL or the same tree. What Web Push needs is small and fully
//! specified: one P-256 key agreement, two HKDF-SHA-256 derivations, one AES-128-GCM record and
//! one ECDSA P-256 signature. Here it is on p256 0.14, hkdf 0.13 and sha2 0.11 (the same crates
//! the calls' DTLS already builds, so the desktop compiles nothing new for it) and aes-gcm 0.10
//! (the relay's own). Checked against the RFC 8291 worked example byte for byte, a browser's
//! half of the decryption, and a token check with the public key (`web_push_tests.rs`).
//!
//! THE KEY FILE (`data/vapid_private.key`) is the base64url of the raw 32-byte P-256 secret
//! scalar, the format jwt-simple wrote, so a server's key, and every browser subscription made
//! against its public key, carries over unchanged.

use base64ct::{Base64UrlUnpadded, Encoding};
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::sec1::ToSec1Point;
use p256::{PublicKey, SecretKey};
use sha2_11::Sha256; // sha2 0.11, the digest hkdf 0.13 is built on (Cargo.toml)

/// How long a push waits at the push service for an offline device, and how long its VAPID token
/// is good for: 12 hours, what the relay has always sent (RFC 8292 caps a token at 24 hours).
pub const VALID_SECS: u64 = 12 * 60 * 60;

/// The record size written into the aes128gcm header. A push message is ONE record (RFC 8291
/// section 4), so this only has to be at least the record's length; 4096 is what RFC 8291's own
/// example uses.
pub const RECORD_SIZE: u32 = 4096;

/// The aes128gcm header in front of the record: salt (16), record size (4), key id length (1)
/// and the key id, our one-off public key uncompressed (65).
pub const HEADER_LEN: usize = 16 + 4 + 1 + 65;

/// The longest message we encrypt: a push service has to take a body of 4096 octets (RFC 8030
/// section 7.2) and may refuse more, and the body is the header, the message, one padding
/// delimiter octet and the 16-octet tag.
pub const MAX_PLAINTEXT: usize = 4096 - HEADER_LEN - 1 - 16;

/// RFC 8291 section 3.4: the key-info prefix, "WebPush: info" and a zero octet.
const WEBPUSH_INFO: &[u8] = b"WebPush: info\0";
/// RFC 8188 section 2.2: the content-encryption key's info.
const CEK_INFO: &[u8] = b"Content-Encoding: aes128gcm\0";
/// RFC 8188 section 2.3: the nonce's info.
const NONCE_INFO: &[u8] = b"Content-Encoding: nonce\0";

/// The relay's VAPID key pair (P-256, used for ES256). No Debug on purpose: it is a secret.
pub struct VapidKey(SigningKey);

impl VapidKey {
    /// A new random key pair.
    pub fn generate() -> Result<Self, String> {
        random_secret().map(|s| Self(SigningKey::from(s)))
    }

    /// The key pair from the raw 32-byte secret scalar (the key file's contents, decoded).
    pub fn from_bytes(raw: &[u8]) -> Result<Self, String> {
        if raw.len() != 32 {
            return Err(format!("a VAPID key is 32 bytes, this one is {}", raw.len()));
        }
        SigningKey::from_slice(raw).map(Self).map_err(|_| "not a valid P-256 secret key".to_string())
    }

    /// The raw 32-byte secret scalar, for the key file.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes().into()
    }

    /// The public key as 65 uncompressed SEC1 octets (0x04, x, y): what a browser's
    /// `PushManager.subscribe({applicationServerKey})` takes, and the `k=` of the header.
    pub fn public_key_uncompressed(&self) -> Vec<u8> {
        self.0.verifying_key().to_sec1_point(false).as_bytes().to_vec()
    }

    /// The `Authorization` header value for a push to `endpoint` (RFC 8292 section 3):
    /// `vapid t=<ES256 JWT>, k=<public key>`. The token's audience is the endpoint's origin, its
    /// expiry `now_unix` + `VALID_SECS`, its subject `contact` (a mailto: or https: URL).
    pub fn authorization(&self, endpoint: &str, contact: &str, now_unix: u64) -> Result<String, String> {
        let url = url::Url::parse(endpoint).map_err(|e| format!("push endpoint is not a URL: {e}"))?;
        let origin = url.origin();
        if !origin.is_tuple() {
            return Err("push endpoint has no origin".into());
        }
        let header = serde_json::json!({ "typ": "JWT", "alg": "ES256" });
        let claims = serde_json::json!({
            "aud": origin.ascii_serialization(),
            "exp": now_unix + VALID_SECS,
            "sub": contact,
        });
        let signing_input = format!(
            "{}.{}",
            Base64UrlUnpadded::encode_string(header.to_string().as_bytes()),
            Base64UrlUnpadded::encode_string(claims.to_string().as_bytes()),
        );
        // ES256 (RFC 7518 section 3.4): ECDSA P-256 over SHA-256, the signature as the fixed
        // 64 octets r || s (never DER). The nonce is RFC 6979's, derived, so no RNG is involved.
        let sig: Signature = self.0.sign(signing_input.as_bytes());
        let token = format!("{signing_input}.{}", Base64UrlUnpadded::encode_string(&sig.to_bytes()));
        let k = Base64UrlUnpadded::encode_string(&self.public_key_uncompressed());
        Ok(format!("vapid t={token}, k={k}"))
    }
}

/// One push, ready to send: POST `body` to `url` with `headers`.
pub struct PushRequest {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// The push for one browser subscription: `p256dh` and `auth` are the subscription's keys as the
/// browser gave them (base64url), `message` the plaintext (our JSON notice).
pub fn build_request(
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    message: &[u8],
    vapid: &VapidKey,
    contact: &str,
    now_unix: u64,
) -> Result<PushRequest, String> {
    let ua_public = Base64UrlUnpadded::decode_vec(p256dh).map_err(|_| "p256dh is not base64url".to_string())?;
    let ua_auth = Base64UrlUnpadded::decode_vec(auth).map_err(|_| "auth is not base64url".to_string())?;
    let body = encrypt(message, &ua_public, &ua_auth)?;
    let authorization = vapid.authorization(endpoint, contact, now_unix)?;
    Ok(PushRequest {
        url: endpoint.to_string(),
        headers: vec![
            ("TTL", VALID_SECS.to_string()),
            ("Content-Encoding", "aes128gcm".to_string()),
            ("Content-Type", "application/octet-stream".to_string()),
            ("Authorization", authorization),
        ],
        body,
    })
}

/// RFC 8291: `message` encrypted for the subscriber whose public key is `ua_public` (SEC1, 65
/// octets uncompressed as browsers give it) and whose auth secret is `auth` (16 octets), with a
/// fresh salt and a fresh one-off key pair of ours.
pub fn encrypt(message: &[u8], ua_public: &[u8], auth: &[u8]) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| format!("no randomness for the salt: {e}"))?;
    let as_secret = random_secret()?;
    encrypt_with(message, ua_public, auth, &salt, &as_secret)
}

/// `encrypt` with the salt and our one-off secret given: the RFC 8291 example's inputs reproduce
/// its ciphertext exactly (the test). Never call it with a salt or a key used before.
pub(crate) fn encrypt_with(
    message: &[u8],
    ua_public: &[u8],
    auth: &[u8],
    salt: &[u8; 16],
    as_secret: &SecretKey,
) -> Result<Vec<u8>, String> {
    if auth.len() != 16 {
        return Err(format!("auth secret is 16 bytes, this one is {}", auth.len()));
    }
    if message.len() > MAX_PLAINTEXT {
        return Err(format!("push message is {} bytes, the most is {MAX_PLAINTEXT}", message.len()));
    }
    let ua_public = PublicKey::from_sec1_bytes(ua_public).map_err(|_| "p256dh is not a P-256 public key".to_string())?;
    let as_public = as_secret.public_key();
    let shared = as_secret.diffie_hellman(&ua_public);
    let (cek, nonce) = record_keys(shared.raw_secret_bytes().as_slice(), auth, &ua_public, &as_public, salt)?;

    use aes_gcm::aead::{Aead, KeyInit};
    let cipher = aes_gcm::Aes128Gcm::new_from_slice(&cek).map_err(|_| "bad content key length".to_string())?;
    // The one record is the last, so its padding delimiter is 0x02 (RFC 8188 section 2), and
    // with one record the sequence number XORed into the nonce is 0.
    let mut record = Vec::with_capacity(message.len() + 1);
    record.extend_from_slice(message);
    record.push(0x02);
    let sealed = cipher
        .encrypt(aes_gcm::Nonce::from_slice(&nonce), record.as_slice())
        .map_err(|_| "AES-GCM encryption failed".to_string())?;

    let keyid = as_public.to_sec1_point(false);
    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(salt);
    out.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    out.push(keyid.as_bytes().len() as u8);
    out.extend_from_slice(keyid.as_bytes());
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// The record's content-encryption key and nonce from the key agreement (RFC 8291 section 3.4,
/// then RFC 8188 sections 2.2 and 2.3). Both halves use it: the sender with its one-off secret
/// and the subscriber's public key, the subscriber (the tests' decryption) the other way round.
pub(crate) fn record_keys(
    ecdh_secret: &[u8],
    auth: &[u8],
    ua_public: &PublicKey,
    as_public: &PublicKey,
    salt: &[u8; 16],
) -> Result<([u8; 16], [u8; 12]), String> {
    let mut key_info = Vec::with_capacity(WEBPUSH_INFO.len() + 130);
    key_info.extend_from_slice(WEBPUSH_INFO);
    key_info.extend_from_slice(ua_public.to_sec1_point(false).as_bytes());
    key_info.extend_from_slice(as_public.to_sec1_point(false).as_bytes());
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(auth), ecdh_secret)
        .expand(&key_info, &mut ikm)
        .map_err(|_| "HKDF expand failed".to_string())?;

    let content = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    content.expand(CEK_INFO, &mut cek).map_err(|_| "HKDF expand failed".to_string())?;
    content.expand(NONCE_INFO, &mut nonce).map_err(|_| "HKDF expand failed".to_string())?;
    Ok((cek, nonce))
}

/// A random P-256 secret key from the OS's randomness. A random 32 bytes fall outside the curve's
/// order about once in 2^32 tries, so a second try is all it ever needs; eight is a fence.
fn random_secret() -> Result<SecretKey, String> {
    for _ in 0..8 {
        let mut raw = [0u8; 32];
        getrandom::getrandom(&mut raw).map_err(|e| format!("no randomness for a key: {e}"))?;
        if let Ok(s) = SecretKey::from_slice(&raw) {
            return Ok(s);
        }
    }
    Err("could not draw a valid P-256 key".into())
}

/// The relay's VAPID key from `path`, made and written there (private to the server's user on
/// unix, 0600) when there is none. A file that holds no key is replaced, as it always was, and
/// browsers subscribed to the old key then subscribe again; a file that cannot be READ (a
/// permission, a disk error) is left alone and Web Push stays off, so a passing fault never
/// throws a good key away. None means Web Push is off for this run.
pub fn load_or_create_key(path: &std::path::Path) -> Option<VapidKey> {
    match std::fs::read_to_string(path) {
        Ok(encoded) => {
            let read = Base64UrlUnpadded::decode_vec(encoded.trim())
                .map_err(|_| "not base64url".to_string())
                .and_then(|raw| VapidKey::from_bytes(&raw));
            match read {
                Ok(key) => {
                    tracing::info!("VAPID key loaded from {}", path.display());
                    return Some(key);
                }
                Err(e) => tracing::error!("The VAPID key at {} holds no key ({e}); making a new one", path.display()),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::error!("The VAPID key at {} could not be read ({e}); Web Push is off", path.display());
            return None;
        }
    }
    let key = match VapidKey::generate() {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("No VAPID key could be made ({e}); Web Push is off");
            return None;
        }
    };
    if let Err(e) = write_private(path, Base64UrlUnpadded::encode_string(&key.to_bytes()).as_bytes()) {
        tracing::error!("Failed to write VAPID key to {}: {e}", path.display());
        return None;
    }
    tracing::info!("Generated new VAPID keypair -> {}", path.display());
    Some(key)
}

/// `bytes` written to `path` through a new file that is private from its first byte on unix
/// (0600) and renamed into place, so the key is never readable by others, nor ever half written.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("key.new");
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let written = opts.open(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    match written.and_then(|()| std::fs::rename(&tmp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
#[path = "web_push_tests.rs"]
mod tests;
