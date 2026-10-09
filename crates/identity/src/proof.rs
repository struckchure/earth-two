//! Compact signed authorizations, byte-identical to identity/proof.go and
//! verified by internals/server/module/src/proof.rs.

use ed25519_dalek::{Signature, Signer, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::{Error, key::Key, key::account_id};

pub const PROOF_SIZE: usize = 32 + 8 + 8 + 64;
/// Microseconds; two minutes, as in Go's `MaxProofLifetime`.
pub const MAX_PROOF_LIFETIME_MICROS: u64 = 120_000_000;

/// Binds a signature to one database and live connection. IDs are the raw
/// little-endian bytes from the SpacetimeDB SDK, not their display strings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub database: [u8; 32],
    pub sender: [u8; 32],
    pub connection: [u8; 16],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authorization {
    pub account_id: String,
    pub public_key: Vec<u8>,
    pub sequence: u64,
}

fn message(ctx: &Context, header: &[u8], action: &str, payload: &[u8]) -> Vec<u8> {
    let mut b = b"earth-two/account-action/v1\0".to_vec();
    b.extend_from_slice(&ctx.database);
    b.extend_from_slice(&ctx.sender);
    b.extend_from_slice(&ctx.connection);
    b.extend_from_slice(header);
    b.extend_from_slice(&(action.len() as u16).to_le_bytes());
    b.extend_from_slice(action.as_bytes());
    b.extend_from_slice(&Sha256::digest(payload));
    b
}

impl Key {
    /// Returns a compact proof, never the private key. `sequence` must match
    /// the account's persisted revision; the reducer advances it in the same
    /// transaction. `expires_unix_micro` is a wall-clock time in microseconds.
    pub fn sign(
        &self,
        ctx: &Context,
        action: &str,
        payload: &[u8],
        sequence: u64,
        expires_unix_micro: i64,
    ) -> Result<Vec<u8>, Error> {
        if action.is_empty() || action.len() > 255 || expires_unix_micro <= 0 {
            return Err(Error::new("identity: invalid signing request"));
        }
        let mut b = self.public_key();
        b.extend_from_slice(&sequence.to_le_bytes());
        b.extend_from_slice(&(expires_unix_micro as u64).to_le_bytes());
        let signature = self.signing_key().sign(&message(ctx, &b, action, payload));
        b.extend_from_slice(&signature.to_bytes());
        Ok(b)
    }
}

/// Checks scope, action, payload and expiry at `now_unix_micro`. The caller
/// must also check `sequence` against durable account state before a mutation.
pub fn verify(
    ctx: &Context,
    action: &str,
    payload: &[u8],
    proof: &[u8],
    now_unix_micro: i64,
) -> Result<Authorization, Error> {
    if proof.len() != PROOF_SIZE || action.is_empty() || action.len() > 255 {
        return Err(Error::new("identity: invalid proof"));
    }
    let expires = u64::from_le_bytes(proof[40..48].try_into().unwrap());
    if now_unix_micro <= 0
        || expires > i64::MAX as u64
        || expires <= now_unix_micro as u64
        || expires - now_unix_micro as u64 > MAX_PROOF_LIFETIME_MICROS
    {
        return Err(Error::new("identity: expired or excessive proof lifetime"));
    }
    let public: [u8; 32] = proof[..32].try_into().unwrap();
    let invalid = || Error::new("identity: invalid signature");
    let key = VerifyingKey::from_bytes(&public).map_err(|_| invalid())?;
    let signature = Signature::from_slice(&proof[48..]).map_err(|_| invalid())?;
    // Go's ed25519.Verify rejects non-canonical signatures too, so strict
    // verification keeps both sides accepting the same proofs.
    key.verify_strict(&message(ctx, &proof[..48], action, payload), &signature)
        .map_err(|_| invalid())?;
    Ok(Authorization {
        account_id: account_id(&public)?,
        public_key: public.to_vec(),
        sequence: u64::from_le_bytes(proof[32..40].try_into().unwrap()),
    })
}
