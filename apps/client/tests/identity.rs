//! Ports of game/identity_test.go, without a window. The panel is the
//! resource the menus drive; here it is driven directly, as the Go tests do.
#![cfg(not(target_arch = "wasm32"))]

use std::{path::PathBuf, sync::Mutex, time::Duration};

use earth_two_client::identity::{IdentityAction, IdentityPanel, TextInput};
use earth_two_identity::{document_account_id, file::read_backup, import};

// EARTH_TWO_IDENTITY_PATH is process-wide; run these tests one at a time.
static ENV: Mutex<()> = Mutex::new(());

fn set_identity_path(path: &PathBuf) {
    // SAFETY: serialized by `ENV`; the panel reads the variable on its own
    // thread only after this call returns.
    unsafe { std::env::set_var("EARTH_TWO_IDENTITY_PATH", path) }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("earth-two-identity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn wait_identity(p: &mut IdentityPanel) {
    let started = std::time::Instant::now();
    while p.busy && started.elapsed() < Duration::from_secs(30) {
        p.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!p.busy && !p.failed, "identity operation: {}", p.message);
}

// TestInGameIdentityCreateLockExportImport
#[test]
fn in_game_identity_create_lock_export_import() {
    let _guard = ENV.lock().unwrap();
    let dir = temp_dir("create");
    let path = dir.join("active.json");
    set_identity_path(&path);
    let mut p = IdentityPanel::new();
    const PASSWORD: &str = "synthetic in-game identity passphrase";
    p.fields[0] = PASSWORD.to_string();
    p.act(IdentityAction::Create);
    wait_identity(&mut p);
    let key = p.key.clone().expect("created key");
    let id = key.id();
    let public_file = std::fs::read_to_string(dir.join("pub")).expect("public key companion");
    assert_eq!(public_file, format!("{}\n", hex::encode(key.public_key())));
    assert_eq!(
        document_account_id(&p.backup),
        id,
        "created key was not saved"
    );
    assert!(p.fields[0].is_empty(), "passphrase remained visible");
    p.act(IdentityAction::Lock);
    assert!(p.key.is_none());
    assert!(
        p.enabled(IdentityAction::Export),
        "locked identity cannot export its backup"
    );
    let backup_path = dir.join("transfer.json");
    p.fields[1] = backup_path.to_string_lossy().into_owned();
    p.act(IdentityAction::Export);
    wait_identity(&mut p);
    let data = read_backup(&backup_path).expect("exported file");
    let restored = import(&data, PASSWORD).expect("export decrypts");
    assert_eq!(restored.id(), id);

    set_identity_path(&dir.join("other-device.json"));
    let mut other = IdentityPanel::new();
    other.fields[0] = PASSWORD.to_string();
    other.fields[1] = backup_path.to_string_lossy().into_owned();
    other.act(IdentityAction::Import);
    wait_identity(&mut other);
    assert_eq!(
        other.key.as_ref().unwrap().id(),
        id,
        "import changed account identity"
    );
    assert_eq!(document_account_id(&other.backup), id);
    assert!(
        !other.enabled(IdentityAction::Email),
        "disconnected identity allowed an email mutation"
    );
    // Importing over an existing identity archives the previous one.
    other.fields[0] = PASSWORD.to_string();
    other.act(IdentityAction::Import);
    wait_identity(&mut other);
    assert!(
        dir.join("identities")
            .join(format!("{id}.earth-two-key.json"))
            .exists(),
        "previous identity was not archived"
    );
    p.close();
    other.close();
    let _ = std::fs::remove_dir_all(dir);
}

// The screen/layout half of TestIdentityMenuAndLayout belongs to the menus;
// this covers the form's own rules: the button list and field editing.
#[test]
fn identity_form_rules() {
    let _guard = ENV.lock().unwrap();
    let dir = temp_dir("rules");
    set_identity_path(&dir.join("active.json"));
    let mut p = IdentityPanel::new();
    assert_eq!(IdentityAction::ITEMS.len(), 9);
    assert_eq!(IdentityAction::ITEMS[0].0, "Create identity");
    assert_eq!(IdentityAction::ITEMS[8].1, IdentityAction::Back);
    assert!(p.enabled(IdentityAction::Back));
    assert!(
        !p.enabled(IdentityAction::Create),
        "create without a passphrase"
    );
    assert_eq!(p.account_line(), "Locked · No identity saved");
    assert_eq!(p.connection_label().0, "Disconnected");
    p.act(IdentityAction::Unlock);
    assert!(p.failed);
    assert_eq!(
        p.message,
        "Enter your key passphrase, or unlock your account first."
    );

    let typed = TextInput {
        chars: "pass 二🦊".chars().collect(),
        ..Default::default()
    };
    p.edit(0, &typed);
    assert_eq!(p.fields[0], "pass 二🦊");
    assert_eq!(p.field_text(0), "•".repeat(7));
    p.edit(
        0,
        &TextInput {
            backspace: true,
            ..Default::default()
        },
    );
    assert_eq!(p.fields[0], "pass 二");
    p.edit(
        0,
        &TextInput {
            modifier: true,
            chars: vec!['x'],
            ..Default::default()
        },
    );
    assert_eq!(p.fields[0], "pass 二", "modifier chords must not type");
    p.edit(
        0,
        &TextInput {
            modifier: true,
            select_all: true,
            ..Default::default()
        },
    );
    assert_eq!(p.field_text(0), "");
    p.edit(
        0,
        &TextInput {
            chars: vec!['\x07', 'a', '\x7f'],
            ..Default::default()
        },
    );
    assert_eq!(p.fields[0], "a", "control characters typed");
    let long = TextInput {
        chars: vec!['e'; 300],
        ..Default::default()
    };
    p.edit(2, &long);
    assert_eq!(p.fields[2].len(), 254, "email limit");
    assert!(p.enabled(IdentityAction::Create));
    p.close();
    let _ = std::fs::remove_dir_all(dir);
}

// TestInGameAccountConnectionSurvivesOperationCompletion
#[test]
fn in_game_account_connection_survives_operation_completion() {
    let Ok(host) = std::env::var("EARTH_TWO_TEST_STDB_HOST") else {
        eprintln!("requires the local identity module");
        return;
    };
    let database =
        std::env::var("EARTH_TWO_TEST_STDB_DATABASE").expect("test database is required");
    let _guard = ENV.lock().unwrap();
    let dir = temp_dir("live");
    set_identity_path(&dir.join("identity.json"));
    let mut p = IdentityPanel::new();
    p.host = host;
    p.database = database;
    p.fields[0] = "synthetic live menu passphrase".to_string();
    p.act(IdentityAction::Create);
    wait_identity(&mut p);
    p.act(IdentityAction::Connect);
    wait_identity(&mut p);
    let session = p
        .session
        .clone()
        .expect("account disconnected when its menu operation completed");
    assert!(session.is_active());
    earth_two_client::identity::jobs::block_on(
        session.set_email("menu@example.com", Duration::from_secs(5)),
    )
    .expect("set email");
    p.fields[2].clear();
    p.act(IdentityAction::Email);
    wait_identity(&mut p);
    // The HUD label should pick up a latency reading within a few polls.
    let started = std::time::Instant::now();
    while p.latency.is_zero() && started.elapsed() < Duration::from_secs(5) {
        p.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(p.connection_label().0.starts_with("Connected"));
    p.close();
    let _ = std::fs::remove_dir_all(dir);
}
