//! Connects player-owned keys to SpacetimeDB transport identities, as Go's
//! internals/account Session. Connection tokens authorize reads; signed
//! reducer arguments authorize changes.
//!
//! The transport is a trait: desktop talks to the database in-process
//! (`native`), the browser through the existing JS adapter (`bridge`). This
//! state machine is the same on both: connect, subscribe, link or register,
//! then wait for the revision the signed action should produce.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::{
    Error,
    clock::{Moment, expires_in_a_minute, now_unix_micro},
    key::Key,
    proof::Context,
};

/// Shared by both transports, without importing a native networking SDK
/// into the browser game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    pub id: String,
    pub public_key: Vec<u8>,
    pub revision: u64,
    pub email: String,
    pub display_name: String,
}

/// One live, subscribed database connection for one account.
pub trait Transport: Sized {
    /// Opens a connection and subscribes to the account's row and the
    /// `my_account` view. The deadline covers lookup, connect and subscribe.
    fn connect(
        host: &str,
        database: &str,
        account_id: &str,
        timeout: Duration,
    ) -> impl Future<Output = Result<Self, Error>>;

    /// Raw little-endian database, sender and connection IDs for proofs.
    fn context(&self) -> Context;
    fn is_active(&self) -> bool;
    fn close(&self);
    /// The subscribed account row's revision, or 0 before registration.
    fn revision(&self) -> u64;
    /// The `my_account` view row, present once this connection is linked.
    fn details(&self) -> Option<Details>;

    /// Calls one account reducer. `text` is the email or display name, or
    /// empty. Resolves when the module reports the reducer's outcome, and
    /// fails with the reducer's message when it rejected the action.
    fn call(
        &self,
        action: &str,
        text: &str,
        proof: Vec<u8>,
        timeout: Duration,
    ) -> impl Future<Output = Result<(), Error>>;

    /// The read-only round-trip probe; returns the nonce the module echoed.
    fn ping(&self, nonce: u64, timeout: Duration) -> impl Future<Output = Result<u64, Error>>;

    /// Waits for the next subscription change, failing on disconnect or
    /// when the timeout passes first.
    fn next_event(&self, timeout: Duration) -> impl Future<Output = Result<(), Error>>;
}

pub struct Session<T: Transport> {
    key: Key,
    transport: T,
    // Serialize signed operations on this connection, as Go's `mu`.
    busy: AtomicBool,
}

/// Releases the session's signed-operation slot when dropped.
struct Serialized<'a>(&'a AtomicBool);

impl Drop for Serialized<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn remaining(deadline: Moment) -> Result<Duration, Error> {
    let left = Moment::now().until(deadline);
    if left.is_zero() {
        return Err(Error::new("context deadline exceeded"));
    }
    Ok(left)
}

impl<T: Transport> Session<T> {
    /// Connects, subscribes and links this connection to the account,
    /// registering it on first use. `timeout` bounds the whole handshake:
    /// Go waits 20 s on desktop and 45 s in the browser.
    pub async fn connect(
        host: &str,
        database: &str,
        key: &Key,
        timeout: Duration,
    ) -> Result<Session<T>, Error> {
        let deadline = Moment::now().plus(timeout);
        let transport = T::connect(host, database, &key.id(), timeout).await?;
        let session = Session {
            key: key.clone(),
            transport,
            busy: AtomicBool::new(false),
        };
        if let Err(e) = session.link(deadline).await {
            session.close();
            return Err(e);
        }
        Ok(session)
    }

    async fn link(&self, deadline: Moment) -> Result<(), Error> {
        let revision = self.revision();
        let action = if revision == 0 {
            "account.register"
        } else {
            "account.link"
        };
        let proof = self.key.sign(
            &self.context(),
            action,
            &[],
            revision,
            expires_in_a_minute(),
        )?;
        self.transport
            .call(action, "", proof, remaining(deadline)?)
            .await?;
        self.committed(deadline, revision + 1, None).await
    }

    async fn committed(
        &self,
        deadline: Moment,
        revision: u64,
        email: Option<&str>,
    ) -> Result<(), Error> {
        loop {
            if let Some(d) = self.transport.details()
                && d.revision == revision
                && email.is_none_or(|e| d.email == e)
            {
                return Ok(());
            }
            self.transport.next_event(remaining(deadline)?).await?;
        }
    }

    pub fn close(&self) {
        self.transport.close();
    }

    pub fn is_active(&self) -> bool {
        self.transport.is_active()
    }

    pub fn context(&self) -> Context {
        self.transport.context()
    }

    pub fn revision(&self) -> u64 {
        self.transport.revision()
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    /// The underlying connection, for tests that send raw proofs.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn details(&self) -> Result<Details, Error> {
        self.transport
            .details()
            .ok_or_else(|| Error::new("account: connection is not linked"))
    }

    async fn mutate(&self, action: &str, text: &str, deadline: Moment) -> Result<u64, Error> {
        let revision = self.revision();
        let proof = self.key.sign(
            &self.context(),
            action,
            text.as_bytes(),
            revision,
            expires_in_a_minute(),
        )?;
        self.transport
            .call(action, text, proof, remaining(deadline)?)
            .await?;
        Ok(revision)
    }

    pub async fn set_email(&self, email: &str, timeout: Duration) -> Result<(), Error> {
        let _guard = self.serialized()?;
        let deadline = Moment::now().plus(timeout);
        let revision = self.mutate("account.set_email", email, deadline).await?;
        self.committed(deadline, revision + 1, Some(email)).await
    }

    pub async fn set_display_name(&self, name: &str, timeout: Duration) -> Result<(), Error> {
        let _guard = self.serialized()?;
        let deadline = Moment::now().plus(timeout);
        let revision = self
            .mutate("account.set_display_name", name, deadline)
            .await?;
        self.committed(deadline, revision + 1, None).await?;
        if self.details()?.display_name != name {
            return Err(Error::new("display name has not synchronized"));
        }
        Ok(())
    }

    /// The round trip of the module's `connection_ping` procedure.
    pub async fn ping(&self, timeout: Duration) -> Result<Duration, Error> {
        if !self.is_active() {
            return Err(Error::new("account disconnected"));
        }
        let started = Moment::now();
        // Go uses UnixNano as the nonce; microseconds times 1000 is the same
        // value at the resolution available everywhere.
        let nonce = (now_unix_micro() as u64).wrapping_mul(1000);
        let echoed = self.transport.ping(nonce, timeout).await?;
        if echoed != nonce {
            return Err(Error::new("invalid latency probe response"));
        }
        Ok(started.elapsed())
    }

    fn serialized(&self) -> Result<Serialized<'_>, Error> {
        // The form never starts two signed actions at once; a blocking lock
        // would wait forever on a single-threaded browser, so refuse instead.
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(Error::new("account: another account action is in progress"));
        }
        Ok(Serialized(&self.busy))
    }
}
