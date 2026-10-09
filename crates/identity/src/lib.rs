//! Player-owned Ed25519 accounts for Earth Two, ported from Go's `identity`
//! and `internals/account` packages. A transport token identifies a
//! connection; only the player's signing key authorizes changes.
//!
//! Keys, proofs and encrypted backups interoperate byte-for-byte with the Go
//! client and the existing SpacetimeDB module. Nothing here renders; the
//! client's `identity` module owns the form, storage glue and HUD text.

pub mod backup;
pub mod clock;
pub mod config;
pub mod key;
pub mod lookup;
pub mod proof;
pub mod session;

#[cfg(not(target_arch = "wasm32"))]
pub mod file;

#[cfg(not(target_arch = "wasm32"))]
mod bindings;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod bridge;

pub use backup::{BACKUP_ITERATIONS, MAX_BACKUP_SIZE, document_account_id, import};
pub use key::{Key, account_id, is_account_id};
pub use proof::{Authorization, Context, MAX_PROOF_LIFETIME_MICROS, PROOF_SIZE, verify};
pub use session::{Details, Session, Transport};

/// The transport this platform connects with: in-process on desktop, the
/// web networking adapter in the browser.
#[cfg(not(target_arch = "wasm32"))]
pub type PlatformTransport = native::NativeTransport;
#[cfg(target_arch = "wasm32")]
pub type PlatformTransport = bridge::BridgeTransport;

/// Errors carry the same plain messages the Go code shows players.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error(e.to_string())
    }
}

impl From<String> for Error {
    fn from(message: String) -> Error {
        Error(message)
    }
}
