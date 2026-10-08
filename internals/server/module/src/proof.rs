use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub struct Authorization {
    pub account_id: String,
    pub public_key: Vec<u8>,
    pub sequence: u64,
}

// Identical byte format to identity/proof.go. IDs are little-endian bytes from
// the SDK; displayed identity hex uses the opposite byte order.
pub fn verify(
    database: [u8; 32],
    sender: [u8; 32],
    connection: [u8; 16],
    action: &str,
    payload: &[u8],
    proof: &[u8],
    now: i64,
) -> Result<Authorization, String> {
    if proof.len() != 112 || action.is_empty() || action.len() > 255 {
        return Err("invalid proof".into());
    }
    let expires = u64::from_le_bytes(proof[40..48].try_into().unwrap());
    if now <= 0
        || expires > i64::MAX as u64
        || expires <= now as u64
        || expires - now as u64 > 120_000_000
    {
        return Err("expired or excessive proof lifetime".into());
    }
    let public_key: [u8; 32] = proof[..32].try_into().unwrap();
    let key = VerifyingKey::from_bytes(&public_key).map_err(|_| "invalid public key")?;
    let signature = Signature::from_slice(&proof[48..]).map_err(|_| "invalid signature")?;
    let mut message = b"earth-two/account-action/v1\0".to_vec();
    message.extend(database);
    message.extend(sender);
    message.extend(connection);
    message.extend(&proof[..48]);
    message.extend((action.len() as u16).to_le_bytes());
    message.extend(action.as_bytes());
    message.extend(Sha256::digest(payload));
    key.verify_strict(&message, &signature)
        .map_err(|_| "invalid signature")?;
    let mut id = Sha256::new();
    id.update(b"earth-two/account-id/v1\0");
    id.update(public_key);
    Ok(Authorization {
        account_id: format!("e2_{}", hex::encode(id.finalize())),
        public_key: public_key.to_vec(),
        sequence: u64::from_le_bytes(proof[32..40].try_into().unwrap()),
    })
}
