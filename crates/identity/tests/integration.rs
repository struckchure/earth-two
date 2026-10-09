//! Port of internals/account/integration_test.go. Runs against an isolated
//! local database published with this repository's module, with fresh
//! synthetic keys; it never reads a real player's key.
//!
//!   EARTH_TWO_TEST_STDB_HOST=http://127.0.0.1:3001 \
//!   EARTH_TWO_TEST_STDB_DATABASE=earth-two-test cargo test -p earth-two-identity --test integration
#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use earth_two_identity::{
    Key, Session, Transport, clock::expires_in_a_minute, import, native::NativeTransport,
};

/// Drives a future to completion on this thread; the desktop transport's
/// futures only block, they never pend.
fn block_on<F: Future>(future: F) -> F::Output {
    use std::{
        pin::pin,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        thread::Thread,
    };
    struct Unpark(Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}

const TIMEOUT: Duration = Duration::from_secs(30);

// TestKeyOwnedAccountIntegration
#[test]
fn key_owned_account_integration() {
    let Ok(host) = std::env::var("EARTH_TWO_TEST_STDB_HOST") else {
        eprintln!(
            "set EARTH_TWO_TEST_STDB_HOST and EARTH_TWO_TEST_STDB_DATABASE for integration tests"
        );
        return;
    };
    let database = std::env::var("EARTH_TWO_TEST_STDB_DATABASE")
        .expect("EARTH_TWO_TEST_STDB_DATABASE is required");
    let key = Key::generate().unwrap();
    let first: Session<NativeTransport> = block_on(Session::connect(
        &host,
        &database,
        &key,
        Duration::from_secs(20),
    ))
    .expect("connect");
    let latency = block_on(first.ping(TIMEOUT)).expect("latency probe");
    assert!(latency > Duration::ZERO);
    let details = first.details().expect("new account");
    assert_eq!(details.id, key.id());
    assert_eq!(details.email, "");
    block_on(first.set_email("player@example.com", TIMEOUT)).expect("set email");
    const DISPLAY_NAME: &str = "Any name! 🦊 / 二";
    block_on(first.set_display_name(DISPLAY_NAME, TIMEOUT)).expect("set display name");
    block_on(first.set_email("", TIMEOUT)).expect("clear email");
    let backup = key.export("integration backup passphrase").unwrap();
    let restored = import(&backup, "integration backup passphrase").unwrap();
    let second: Session<NativeTransport> = block_on(Session::connect(
        &host,
        &database,
        &restored,
        Duration::from_secs(20),
    ))
    .expect("reconnect");
    assert_ne!(
        first.context().sender,
        second.context().sender,
        "expected a new transport identity"
    );
    let details = second.details().expect("restored account");
    assert_eq!(details.id, key.id());
    assert_eq!(
        details.display_name, DISPLAY_NAME,
        "display name lost on import"
    );
    let other_key = Key::generate().unwrap();
    let other: Session<NativeTransport> = block_on(Session::connect(
        &host,
        &database,
        &other_key,
        Duration::from_secs(20),
    ))
    .expect("other");
    block_on(other.set_display_name(DISPLAY_NAME, TIMEOUT))
        .expect("duplicate display name was rejected");
    block_on(second.set_email("restored@example.com", TIMEOUT)).expect("set email");
    let revision = second.revision();
    let proof = restored
        .sign(
            &second.context(),
            "account.set_email",
            b"signed@example.com",
            revision,
            expires_in_a_minute(),
        )
        .unwrap();
    let reject = |name: &str, email: &str, bytes: Vec<u8>, revision: u64| {
        let result = block_on(
            second
                .transport()
                .call("account.set_email", email, bytes, TIMEOUT),
        );
        assert!(result.is_err(), "{name}: no reducer error");
        assert_eq!(
            second.revision(),
            revision,
            "{name}: invalid action consumed a sequence"
        );
    };
    reject(
        "transport token alone",
        "signed@example.com",
        Vec::new(),
        revision,
    );
    reject(
        "altered payload",
        "tampered@example.com",
        proof.clone(),
        revision,
    );
    let wrong_connection = restored
        .sign(
            &first.context(),
            "account.set_email",
            b"signed@example.com",
            revision,
            expires_in_a_minute(),
        )
        .unwrap();
    reject(
        "another connection",
        "signed@example.com",
        wrong_connection,
        revision,
    );
    let mut wrong_scope = second.context();
    wrong_scope.database[0] = wrong_scope.database[0].wrapping_add(1);
    let wrong_database = restored
        .sign(
            &wrong_scope,
            "account.set_email",
            b"signed@example.com",
            revision,
            expires_in_a_minute(),
        )
        .unwrap();
    reject(
        "another database",
        "signed@example.com",
        wrong_database,
        revision,
    );
    block_on(second.transport().call(
        "account.set_email",
        "signed@example.com",
        proof.clone(),
        TIMEOUT,
    ))
    .expect("signed email");
    let mut waited = 0;
    while second
        .details()
        .map(|d| d.revision != revision + 1 || d.email != "signed@example.com")
        .unwrap_or(true)
    {
        block_on(second.transport().next_event(Duration::from_secs(5))).expect("commit");
        waited += 1;
        assert!(waited < 100);
    }
    let revision = revision + 1;
    reject("replay", "signed@example.com", proof, revision);
    // Go also checks the private account_email table is unreadable with a
    // one-off query; the Rust SDK has no one-off query, so the subscription
    // not containing it is the equivalent check here.
    assert!(
        block_on(second.set_email("bad email", TIMEOUT)).is_err(),
        "invalid email accepted"
    );
    assert_eq!(
        second.revision(),
        revision,
        "failed action consumed a sequence"
    );
    first.close();
    second.close();
    other.close();
}
