//! Cross-language fixtures with identity/fixture_test.go: the Go-written
//! `fixtures/identity.json` must verify and decrypt here, and with
//! `EARTH_TWO_WRITE_FIXTURE=1` this test writes `fixtures/identity-rust.json`
//! for the Go test's verify branch. The key is synthetic test data.

use earth_two_identity::{Context, Key, import, verify};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Fixture {
    seed: String,
    public_key: String,
    account_id: String,
    database: String,
    sender: String,
    connection: String,
    action: String,
    payload: String,
    sequence: u64,
    now_unix_micro: i64,
    expires_unix_micro: i64,
    proof: String,
    backup_passphrase: String,
    backup: serde_json::Value,
}

#[derive(Serialize)]
struct RustFixture {
    account_id: String,
    proof: String,
    backup_passphrase: String,
    backup: serde_json::Value,
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn context(f: &Fixture) -> Context {
    let mut ctx = Context::default();
    ctx.database
        .copy_from_slice(&hex::decode(&f.database).unwrap());
    ctx.sender.copy_from_slice(&hex::decode(&f.sender).unwrap());
    ctx.connection
        .copy_from_slice(&hex::decode(&f.connection).unwrap());
    ctx
}

#[test]
fn go_fixture_verifies_and_decrypts() {
    let data = std::fs::read(fixture_path("identity.json")).expect("run the Go fixture test first");
    let f: Fixture = serde_json::from_slice(&data).expect("fixture json");
    let seed = hex::decode(&f.seed).unwrap();
    let k = Key::from_seed(&seed).expect("seed");
    assert_eq!(hex::encode(k.public_key()), f.public_key);
    assert_eq!(k.id(), f.account_id);
    assert!(earth_two_identity::is_account_id(&k.id()));
    let ctx = context(&f);
    let proof = hex::decode(&f.proof).unwrap();

    // Same request, same bytes: Ed25519 signatures are deterministic.
    let signed = k
        .sign(
            &ctx,
            &f.action,
            f.payload.as_bytes(),
            f.sequence,
            f.expires_unix_micro,
        )
        .expect("sign");
    assert_eq!(
        hex::encode(&signed),
        f.proof,
        "Rust signed different proof bytes"
    );
    let a = verify(
        &ctx,
        &f.action,
        f.payload.as_bytes(),
        &proof,
        f.now_unix_micro,
    )
    .expect("Go proof verifies");
    assert_eq!(a.account_id, f.account_id);
    assert_eq!(a.sequence, f.sequence);
    assert!(
        verify(&ctx, &f.action, b"other", &proof, f.now_unix_micro).is_err(),
        "payload not bound"
    );

    // The Go-encrypted backup opens with the Unicode passphrase.
    let backup = serde_json::to_vec_pretty(&f.backup).unwrap();
    let restored = import(&backup, &f.backup_passphrase).expect("Go backup decrypts");
    assert_eq!(restored.id(), f.account_id);
    assert!(import(&backup, "wrong passphrase").is_err());
    assert_eq!(
        earth_two_identity::document_account_id(&backup),
        f.account_id
    );

    // The seed never appears in a backup, as Go's test checks.
    let rust_backup = k.export(&f.backup_passphrase).expect("export");
    assert!(
        !rust_backup
            .windows(seed.len())
            .any(|w| w == seed.as_slice()),
        "backup leaked seed"
    );
    assert_eq!(
        import(&rust_backup, &f.backup_passphrase).unwrap().id(),
        f.account_id
    );

    if std::env::var("EARTH_TWO_WRITE_FIXTURE").as_deref() == Ok("1") {
        let out = RustFixture {
            account_id: k.id(),
            proof: hex::encode(&signed),
            backup_passphrase: f.backup_passphrase.clone(),
            backup: serde_json::from_slice(&rust_backup).unwrap(),
        };
        let mut text = serde_json::to_string_pretty(&out).unwrap();
        text.push('\n');
        std::fs::write(fixture_path("identity-rust.json"), text).expect("write");
    }
}

#[test]
fn rust_fixture_still_opens() {
    // The checked-in Rust-produced file, which the Go test also verifies.
    let Ok(data) = std::fs::read(fixture_path("identity-rust.json")) else {
        return;
    };
    let r: serde_json::Value = serde_json::from_slice(&data).unwrap();
    let backup = serde_json::to_vec(&r["backup"]).unwrap();
    let key = import(&backup, r["backup_passphrase"].as_str().unwrap()).expect("decrypt");
    assert_eq!(key.id(), r["account_id"].as_str().unwrap());
}
