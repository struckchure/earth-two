//! Player-owned Ed25519 accounts. A transport token identifies a connection;
//! only the player's signing key authorizes changes.

use ed25519_dalek::{SecretKey, SigningKey};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::Error;

pub const SEED_SIZE: usize = 32;
pub const PUBLIC_KEY_SIZE: usize = 32;

/// A private key in memory. Its Debug and string representations never
/// include the seed. `export` encrypts the seed for storage and transfer.
#[derive(Clone)]
pub struct Key {
    private: SigningKey,
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id())
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id())
    }
}

impl Key {
    pub fn generate() -> Result<Key, Error> {
        let mut seed = Zeroizing::new([0u8; SEED_SIZE]);
        getrandom::getrandom(seed.as_mut())
            .map_err(|_| Error::new("identity: no secure randomness available"))?;
        Ok(Key {
            private: SigningKey::from_bytes(&seed),
        })
    }

    pub fn from_seed(seed: &[u8]) -> Result<Key, Error> {
        let seed: SecretKey = seed
            .try_into()
            .map_err(|_| Error::new("identity: invalid Ed25519 seed"))?;
        Ok(Key {
            private: SigningKey::from_bytes(&seed),
        })
    }

    pub fn public_key(&self) -> Vec<u8> {
        self.private.verifying_key().to_bytes().to_vec()
    }

    pub fn id(&self) -> String {
        account_id(&self.public_key()).unwrap_or_default()
    }

    pub(crate) fn signing_key(&self) -> &SigningKey {
        &self.private
    }

    /// The seed, zeroized when dropped. Only the backup format reads it.
    pub(crate) fn seed(&self) -> Zeroizing<[u8; SEED_SIZE]> {
        Zeroizing::new(self.private.to_bytes())
    }
}

/// The account ID is independent of email, connection tokens and database
/// hosts. Same bytes as identity/key.go and the server module's proof.rs.
pub fn account_id(public_key: &[u8]) -> Result<String, Error> {
    if public_key.len() != PUBLIC_KEY_SIZE {
        return Err(Error::new("identity: invalid public key"));
    }
    let mut h = Sha256::new();
    h.update(b"earth-two/account-id/v1\0");
    h.update(public_key);
    Ok(format!("e2_{}", hex::encode(h.finalize())))
}

/// Whether text has the shape of an account ID: `e2_` and 64 lowercase hex.
pub fn is_account_id(id: &str) -> bool {
    id.len() == 67
        && id.starts_with("e2_")
        && id[3..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
