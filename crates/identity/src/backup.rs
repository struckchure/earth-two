//! Portable, authenticated encrypted key backups. Format v1 is shared with
//! identity/backup.go: PBKDF2-SHA256 (600 000 iterations) and AES-256-GCM over
//! the Ed25519 seed, with the public identity bound as associated data.

use aes_gcm::{
    Aes256Gcm, Key as AesKey, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{Error, key::Key, key::account_id, key::is_account_id};

/// Fixed by format v1 so imports cannot request unbounded work. WebCrypto, Go
/// and this crate implement the same PBKDF2-SHA256/AES-256-GCM format.
pub const BACKUP_ITERATIONS: u32 = 600_000;
pub const MAX_BACKUP_SIZE: usize = 4096;

/// Go encodes `[]byte` as standard padded base64 in JSON.
fn to_base64<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&STANDARD.encode(bytes))
}

fn from_base64<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let text = String::deserialize(d)?;
    STANDARD.decode(text).map_err(serde::de::Error::custom)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    format: String,
    version: i64,
    account_id: String,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    public_key: Vec<u8>,
    kdf: String,
    iterations: i64,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    salt: Vec<u8>,
    cipher: String,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    nonce: Vec<u8>,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    ciphertext: Vec<u8>,
}

impl Backup {
    fn aad(&self) -> Vec<u8> {
        let mut out = b"earth-two/key-backup/v1\0".to_vec();
        out.extend_from_slice(&self.public_key);
        out.extend_from_slice(&(self.iterations as u32).to_le_bytes());
        out.extend_from_slice(&self.salt);
        out
    }
}

fn backup_cipher(password: &str, salt: &[u8]) -> Result<Aes256Gcm, Error> {
    if password.is_empty() {
        return Err(Error::new("identity: a backup passphrase is required"));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, BACKUP_ITERATIONS, key.as_mut());
    Ok(Aes256Gcm::new(AesKey::<Aes256Gcm>::from_slice(
        key.as_ref(),
    )))
}

impl Key {
    /// Creates a portable, authenticated encrypted backup. The server never
    /// receives this document or its passphrase.
    pub fn export(&self, password: &str) -> Result<Vec<u8>, Error> {
        let mut b = Backup {
            format: "earth-two-key".into(),
            version: 1,
            account_id: self.id(),
            public_key: self.public_key(),
            kdf: "pbkdf2-sha256".into(),
            iterations: BACKUP_ITERATIONS as i64,
            salt: vec![0; 16],
            cipher: "aes-256-gcm".into(),
            nonce: vec![0; 12],
            ciphertext: Vec::new(),
        };
        let random = |buf: &mut [u8]| {
            getrandom::getrandom(buf)
                .map_err(|_| Error::new("identity: no secure randomness available"))
        };
        random(&mut b.salt)?;
        random(&mut b.nonce)?;
        let aead = backup_cipher(password, &b.salt)?;
        let seed = self.seed();
        let aad = b.aad();
        b.ciphertext = aead
            .encrypt(
                Nonce::from_slice(&b.nonce),
                Payload {
                    msg: seed.as_ref(),
                    aad: &aad,
                },
            )
            .map_err(|_| Error::new("identity: backup encryption failed"))?;
        serde_json::to_vec_pretty(&b).map_err(|_| Error::new("identity: backup encoding failed"))
    }
}

pub fn import(data: &[u8], password: &str) -> Result<Key, Error> {
    if data.len() > MAX_BACKUP_SIZE {
        return Err(Error::new("identity: backup is too large"));
    }
    // serde_json rejects unknown fields (deny_unknown_fields) and trailing
    // data, which Go checks with DisallowUnknownFields and a second Decode.
    let b: Backup = serde_json::from_slice(data).map_err(|e| {
        if e.is_syntax() && e.to_string().starts_with("trailing") {
            Error::new("identity: trailing backup data")
        } else {
            Error::new("identity: invalid backup document")
        }
    })?;
    if b.format != "earth-two-key"
        || b.version != 1
        || b.kdf != "pbkdf2-sha256"
        || b.iterations != BACKUP_ITERATIONS as i64
        || b.cipher != "aes-256-gcm"
        || b.public_key.len() != 32
        || b.salt.len() != 16
        || b.nonce.len() != 12
        || b.ciphertext.len() != 48
    {
        return Err(Error::new("identity: unsupported or invalid backup format"));
    }
    let aead = backup_cipher(password, &b.salt)?;
    let seed = Zeroizing::new(
        aead.decrypt(
            Nonce::from_slice(&b.nonce),
            Payload {
                msg: &b.ciphertext,
                aad: &b.aad(),
            },
        )
        .map_err(|_| Error::new("identity: incorrect passphrase or damaged backup"))?,
    );
    let k = Key::from_seed(&seed)?;
    if k.id() != b.account_id || k.public_key() != b.public_key {
        return Err(Error::new(
            "identity: backup public identity does not match its private key",
        ));
    }
    Ok(k)
}

/// The account ID a saved backup document claims, checked against its public
/// key, or empty when the document is not a valid v1 backup. This is the
/// game's `identityBackupID`; it needs no passphrase.
pub fn document_account_id(data: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Header {
        #[serde(default)]
        format: String,
        #[serde(default)]
        version: i64,
        #[serde(default)]
        account_id: String,
        #[serde(default, deserialize_with = "from_base64")]
        public_key: Vec<u8>,
    }
    let Ok(h) = serde_json::from_slice::<Header>(data) else {
        return String::new();
    };
    if h.format != "earth-two-key" || h.version != 1 {
        return String::new();
    }
    match account_id(&h.public_key) {
        Ok(id) if id == h.account_id && is_account_id(&id) => id,
        _ => String::new(),
    }
}

/// The public key a backup document carries, for the companion `pub` file.
pub fn document_public_key(data: &[u8]) -> Result<Vec<u8>, Error> {
    #[derive(Deserialize)]
    struct Metadata {
        #[serde(default, deserialize_with = "from_base64")]
        public_key: Vec<u8>,
    }
    serde_json::from_slice::<Metadata>(data)
        .map(|m| m.public_key)
        .map_err(|_| Error::new("identity: invalid backup document"))
}
