//! The desktop transport: an in-process SpacetimeDB client, as Go's
//! internals/account/session.go uses the Go SDK. Table callbacks feed an
//! events channel; connection and reducer failures feed an errors channel;
//! `Session` waits on those exactly as the Go code waits on its channels.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender},
    },
    time::Duration,
};

use spacetimedb_sdk::{DbContext, Table, TableWithPrimaryKey};

use crate::{
    Error,
    bindings::{
        DbConnection, account_table::AccountTableAccess,
        connection_ping_procedure::connection_ping, link_account_reducer::link_account,
        my_account_table::MyAccountTableAccess, register_account_reducer::register_account,
        set_account_display_name_reducer::set_account_display_name,
        set_account_email_reducer::set_account_email,
    },
    clock::Moment,
    lookup::database_identity,
    proof::Context,
    session::{Details, Transport},
};

pub struct NativeTransport {
    conn: Arc<DbConnection>,
    database: [u8; 32],
    events: Mutex<Receiver<()>>,
    errors: Mutex<Receiver<Error>>,
}

/// A one-slot, non-blocking reporter, as Go's buffered `errors` channel.
#[derive(Clone)]
struct Reporter(SyncSender<Error>);

impl Reporter {
    fn report(&self, error: impl Into<Error>) {
        let _ = self.0.try_send(error.into());
    }
}

/// Waits for `ready`, an error, or the deadline, whichever comes first.
fn wait<R>(
    ready: &Receiver<R>,
    errors: &Mutex<Receiver<Error>>,
    timeout: Duration,
) -> Result<R, Error> {
    let deadline = Moment::now().plus(timeout);
    loop {
        if let Ok(error) = errors.lock().unwrap().try_recv() {
            return Err(error);
        }
        let left = Moment::now().until(deadline);
        if left.is_zero() {
            return Err(Error::new("context deadline exceeded"));
        }
        match ready.recv_timeout(left.min(Duration::from_millis(20))) {
            Ok(value) => return Ok(value),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::new("account disconnected"));
            }
        }
    }
}

fn oneshot<R>() -> (Sender<R>, Receiver<R>) {
    mpsc::channel()
}

fn internal(e: spacetimedb_sdk::__codegen::InternalError) -> Error {
    Error::new(e.to_string())
}

impl Transport for NativeTransport {
    async fn connect(
        host: &str,
        database: &str,
        account_id: &str,
        timeout: Duration,
    ) -> Result<NativeTransport, Error> {
        let deadline = Moment::now().plus(timeout);
        let scope = database_identity(host, database, timeout)?;
        let (error_tx, error_rx) = mpsc::sync_channel(1);
        let (event_tx, event_rx) = mpsc::sync_channel(1);
        let report = Reporter(error_tx);
        let (ready_tx, ready_rx) = oneshot();
        let conn = DbConnection::builder()
            .with_uri(host)
            .with_database_name(database)
            .with_token(None::<String>)
            .on_connect(move |_, _, _| {
                let _ = ready_tx.send(());
            })
            .on_connect_error({
                let report = report.clone();
                move |_, e| report.report(e.to_string())
            })
            .on_disconnect({
                let report = report.clone();
                move |_, e| {
                    if let Some(e) = e {
                        report.report(e.to_string());
                    }
                }
            })
            .build()
            .map_err(|e| Error::new(e.to_string()))?;
        let conn = Arc::new(conn);
        let transport = NativeTransport {
            conn: Arc::clone(&conn),
            database: scope,
            events: Mutex::new(event_rx),
            errors: Mutex::new(error_rx),
        };
        let notify = move || {
            let _ = event_tx.try_send(());
        };
        conn.db.account().on_insert({
            let notify = notify.clone();
            move |_, _| notify()
        });
        conn.db.account().on_update({
            let notify = notify.clone();
            move |_, _, _| notify()
        });
        conn.db.my_account().on_insert({
            let notify = notify.clone();
            move |_, _| notify()
        });
        conn.db.my_account().on_delete(move |_, _| notify());
        // Pump messages on a thread; a failed run is reported, not a panic.
        std::thread::spawn({
            let conn = Arc::clone(&conn);
            let report = report.clone();
            move || {
                loop {
                    match conn.advance_one_message_blocking() {
                        Ok(()) => {}
                        Err(spacetimedb_sdk::Error::Disconnected) => return,
                        Err(e) => {
                            report.report(e.to_string());
                            return;
                        }
                    }
                }
            }
        });
        let finish = |result: Result<(), Error>| {
            if result.is_err() {
                transport.close();
            }
            result
        };
        finish(wait(&ready_rx, &transport.errors, Moment::now().until(deadline)).map(|_| ()))?;
        let (applied_tx, applied_rx) = oneshot();
        conn.subscription_builder()
            .on_applied(move |_| {
                let _ = applied_tx.send(());
            })
            .on_error({
                let report = report.clone();
                move |_, e| report.report(e.to_string())
            })
            .subscribe([
                format!("SELECT * FROM account WHERE id = '{account_id}'"),
                "SELECT * FROM my_account".to_string(),
            ]);
        finish(
            wait(
                &applied_rx,
                &transport.errors,
                Moment::now().until(deadline),
            )
            .map(|_| ()),
        )?;
        Ok(transport)
    }

    fn context(&self) -> Context {
        Context {
            database: self.database,
            sender: self
                .conn
                .try_identity()
                .map(|i| i.to_byte_array())
                .unwrap_or_default(),
            connection: self.conn.connection_id().as_le_byte_array(),
        }
    }

    fn is_active(&self) -> bool {
        self.conn.is_active()
    }

    fn close(&self) {
        let _ = self.conn.disconnect();
    }

    fn revision(&self) -> u64 {
        self.conn
            .db
            .account()
            .iter()
            .next()
            .map(|a| a.revision)
            .unwrap_or(0)
    }

    fn details(&self) -> Option<Details> {
        self.conn.db.my_account().iter().next().map(|a| Details {
            id: a.id,
            public_key: a.public_key,
            revision: a.revision,
            email: a.email,
            display_name: a.display_name,
        })
    }

    async fn call(
        &self,
        action: &str,
        text: &str,
        proof: Vec<u8>,
        timeout: Duration,
    ) -> Result<(), Error> {
        let (tx, rx) = oneshot::<Result<Result<(), String>, Error>>();
        let done = move |_: &_, result: Result<Result<(), String>, _>| {
            let _ = tx.send(result.map_err(internal));
        };
        let text = text.to_string();
        match action {
            "account.register" => self.conn.reducers.register_account_then(proof, done),
            "account.link" => self.conn.reducers.link_account_then(proof, done),
            "account.set_email" => self.conn.reducers.set_account_email_then(text, proof, done),
            "account.set_display_name" => self
                .conn
                .reducers
                .set_account_display_name_then(text, proof, done),
            _ => return Err(Error::new("unknown account action")),
        }
        .map_err(|e| Error::new(e.to_string()))?;
        match wait(&rx, &self.errors, timeout)?? {
            Ok(()) => Ok(()),
            Err(message) => Err(Error::new(message)),
        }
    }

    async fn ping(&self, nonce: u64, timeout: Duration) -> Result<u64, Error> {
        let (tx, rx) = oneshot::<Result<u64, Error>>();
        self.conn
            .procedures
            .connection_ping_then(nonce, move |_, result| {
                let _ = tx.send(result.map_err(internal));
            });
        wait(&rx, &self.errors, timeout)?
    }

    async fn next_event(&self, timeout: Duration) -> Result<(), Error> {
        let events = self.events.lock().unwrap();
        wait(&events, &self.errors, timeout)
    }
}
