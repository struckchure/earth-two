//! Ports of identity/identity_test.go.

use earth_two_identity::{Context, Key, MAX_PROOF_LIFETIME_MICROS, import, verify};

fn test_key() -> Key {
    Key::generate().expect("key")
}

const NOW: i64 = 1_800_000_000_000_000;
const MINUTE: i64 = 60_000_000;

// TestProofBindsEveryAuthorizationField
#[test]
fn proof_binds_every_authorization_field() {
    let k = test_key();
    let mut ctx = Context::default();
    ctx.database[0] = 1;
    ctx.sender[0] = 2;
    ctx.connection[0] = 3;
    let proof = k
        .sign(&ctx, "account.set_email", b"a@example.com", 7, NOW + MINUTE)
        .expect("sign");
    let a = verify(&ctx, "account.set_email", b"a@example.com", &proof, NOW).expect("verify");
    assert_eq!(a.account_id, k.id());
    assert_eq!(a.sequence, 7);
    for field in [
        "database",
        "sender",
        "connection",
        "action",
        "payload",
        "sequence",
        "signature",
        "truncated",
        "expiry",
    ] {
        let (mut c, mut action, mut payload, mut p, mut at) = (
            ctx,
            "account.set_email",
            b"a@example.com".to_vec(),
            proof.clone(),
            NOW,
        );
        match field {
            "database" => c.database[0] += 1,
            "sender" => c.sender[0] += 1,
            "connection" => c.connection[0] += 1,
            "action" => action = "account.link",
            "payload" => payload = b"b@example.com".to_vec(),
            "sequence" => p[32] += 1,
            "signature" => p[100] = p[100].wrapping_add(1),
            "truncated" => {
                p.pop();
            }
            "expiry" => at = NOW + MINUTE,
            _ => unreachable!(),
        }
        assert!(
            verify(&c, action, &payload, &p, at).is_err(),
            "altered {field} authorized"
        );
    }
    let p = k
        .sign(
            &ctx,
            "account.link",
            &[],
            7,
            NOW + MAX_PROOF_LIFETIME_MICROS as i64 + 1_000_000,
        )
        .expect("sign");
    assert!(
        verify(&ctx, "account.link", &[], &p, NOW).is_err(),
        "excessive lifetime accepted"
    );
}

// TestBackupRestoresIdentityAndSigningAuthority
#[test]
fn backup_restores_identity_and_signing_authority() {
    let k = test_key();
    let backup = k.export("a long test backup passphrase").expect("export");
    let restored = import(&backup, "a long test backup passphrase").expect("restore");
    assert_eq!(restored.id(), k.id());
    assert_eq!(restored.public_key(), k.public_key());
    // Go checks the seed is absent from the document; with a known seed that
    // check is in fixture.rs. Here: salt and nonce are fresh per export.
    let again = k.export("a long test backup passphrase").expect("export");
    assert_ne!(backup, again, "salt and nonce must be fresh");
    let mut ctx = Context::default();
    ctx.database[0] = 1;
    let now = earth_two_identity::clock::now_unix_micro();
    let proof = restored
        .sign(&ctx, "account.link", &[], 8, now + MINUTE)
        .expect("sign");
    verify(&ctx, "account.link", &[], &proof, now).expect("restored key signs");
    assert!(
        import(&backup, "wrong passphrase").is_err(),
        "wrong passphrase accepted"
    );
    let document: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&backup).expect("json");
    for field in ["account_id", "ciphertext", "iterations", "version"] {
        let mut copy = document.clone();
        let value = match field {
            "account_id" => serde_json::Value::from("another-account"),
            "ciphertext" => serde_json::Value::from("AAAA"),
            _ => serde_json::Value::from(999_999_999),
        };
        copy.insert(field.to_string(), value);
        let corrupt = serde_json::to_vec(&copy).expect("json");
        assert!(
            import(&corrupt, "a long test backup passphrase").is_err(),
            "damaged {field} accepted"
        );
    }
    let mut trailing = backup.clone();
    trailing.extend_from_slice(b" {}");
    assert!(
        import(&trailing, "a long test backup passphrase").is_err(),
        "trailing data accepted"
    );
    assert!(k.export("").is_err(), "empty passphrase accepted");
}

#[test]
fn backup_document_layout_matches_go() {
    // Go's MarshalIndent order and names, so either client can read a file
    // the other wrote and the saved-key ID check works without a passphrase.
    let k = test_key();
    let backup = k.export("p").expect("export");
    let text = String::from_utf8(backup.clone()).expect("utf-8");
    let keys: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix('"'))
        .filter_map(|l| l.split_once('"'))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        keys,
        [
            "format",
            "version",
            "account_id",
            "public_key",
            "kdf",
            "iterations",
            "salt",
            "cipher",
            "nonce",
            "ciphertext"
        ]
    );
    assert!(text.contains("\"iterations\": 600000"));
    assert_eq!(earth_two_identity::document_account_id(&backup), k.id());
    assert_eq!(earth_two_identity::document_account_id(b"{}"), "");
}
