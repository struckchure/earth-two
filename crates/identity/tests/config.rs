//! Ports of internals/account/config_native_test.go.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::Mutex,
    time::Duration,
};

use earth_two_identity::{
    config::{DEFAULT_DATABASE, DEFAULT_HOST, network_defaults},
    lookup::{database_identity, identity_url, parse_database_identity, path_escape},
};

// Environment is process-wide; keep the tests that touch it in sequence.
static ENV: Mutex<()> = Mutex::new(());

fn set(key: &str, value: Option<&str>) {
    // SAFETY: serialized by `ENV`, and no other thread in this test binary
    // reads the environment concurrently.
    unsafe {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

// TestNetworkDefaultsIgnoreDotEnvAndAllowExportedOverrides
#[test]
fn network_defaults_ignore_dotenv_and_allow_exported_overrides() {
    let _guard = ENV.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("earth-two-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    set("SPACETIMEDB_SERVER", Some(""));
    set("SPACETIMEDB_DATABASE", Some(""));
    assert_eq!(
        network_defaults(),
        (DEFAULT_HOST.to_string(), DEFAULT_DATABASE.to_string())
    );
    std::fs::write(
        ".env",
        "AWS_SECRET_ACCESS_KEY=synthetic-secret\nSPACETIMEDB_SERVER=\"http://127.0.0.1:3999\"\nSPACETIMEDB_DATABASE=custom-game\n",
    )
    .unwrap();
    assert_eq!(
        network_defaults(),
        (DEFAULT_HOST.to_string(), DEFAULT_DATABASE.to_string()),
        "runtime read build-only file"
    );
    set("SPACETIMEDB_SERVER", Some("http://127.0.0.1:4000"));
    assert_eq!(
        network_defaults().0,
        "http://127.0.0.1:4000",
        "exported setting was overridden"
    );
    assert_ne!(
        std::env::var("AWS_SECRET_ACCESS_KEY").ok().as_deref(),
        Some("synthetic-secret"),
        "game imported AWS credentials into its environment"
    );
    set("SPACETIMEDB_SERVER", None);
    set("SPACETIMEDB_DATABASE", None);
    std::env::set_current_dir(previous).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// A one-request-at-a-time HTTP server, as Go's httptest, until dropped.
fn serve(
    handler: impl Fn(&str) -> (u16, String) + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(false).unwrap();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming().take(8) {
            let Ok(mut stream) = stream else { break };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                break;
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                    break;
                }
            }
            if path == "/quit" {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                break;
            }
            let (status, body) = handler(&path);
            let reason = if status == 200 { "OK" } else { "Not Found" };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    (url, handle)
}

fn stop(url: &str, handle: std::thread::JoinHandle<()>) {
    let _ = ureq::get(format!("{url}/quit")).call();
    let _ = handle.join();
}

// TestDatabaseLookupDistinguishesWrongServiceAndMissingModule
#[test]
fn database_lookup_distinguishes_wrong_service_and_missing_module() {
    for game_server in [false, true] {
        let (url, handle) = serve(move |path| {
            if game_server && path == "/v1/ping" {
                (200, String::new())
            } else {
                (404, "404 page not found\n".to_string())
            }
        });
        let err = database_identity(&url, "missing-game", Duration::from_secs(5))
            .expect_err("missing database accepted");
        let want = if game_server {
            "start it with make server"
        } else {
            "not the game database server"
        };
        assert!(err.message().contains(want), "diagnostic: {err}");
        stop(&url, handle);
    }
}

#[test]
fn database_lookup_reverses_display_hex_into_proof_bytes() {
    let display: String = (0..32u8).map(|b| format!("{b:02x}")).collect();
    let quoted = format!("\"{display}\"");
    let (url, handle) = serve(move |path| {
        if path == "/v1/database/my%2Fgame/identity" {
            (200, quoted.clone())
        } else {
            (404, String::new())
        }
    });
    let id =
        database_identity(&format!("{url}/"), "my/game", Duration::from_secs(5)).expect("identity");
    assert_eq!(id[31], 0);
    assert_eq!(id[0], 31);
    stop(&url, handle);
    assert_eq!(
        parse_database_identity(&format!(" {display}\n")).unwrap(),
        id
    );
    assert!(parse_database_identity("zz").is_err());
}

#[test]
fn identity_url_follows_go_rules() {
    assert_eq!(
        identity_url("ws://127.0.0.1:3001", "earth-two").unwrap(),
        (
            "http://127.0.0.1:3001".to_string(),
            "http://127.0.0.1:3001/v1/database/earth-two/identity".to_string()
        )
    );
    assert_eq!(
        identity_url("https://example.com/base/?x=1#f", "a b")
            .unwrap()
            .1,
        "https://example.com/base/v1/database/a%20b/identity"
    );
    assert!(identity_url("ftp://example.com", "db").is_err());
    assert!(identity_url("http://user:pw@example.com", "db").is_err());
    assert!(identity_url("example.com", "db").is_err());
    assert_eq!(path_escape("a/b;c,d?e=f@g"), "a%2Fb%3Bc%2Cd%3Fe=f@g");
}
