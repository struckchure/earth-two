//! Public network defaults. Go sets `account.DefaultHost`/`DefaultDatabase`
//! at link time from tools/buildenv; the Rust equivalent is the optional
//! compile-time environment below. Clients never read a `.env` file.

/// Build-time public defaults: `EARTH_TWO_DEFAULT_SPACETIMEDB_SERVER` and
/// `EARTH_TWO_DEFAULT_SPACETIMEDB_DATABASE` in the compiler's environment.
pub const DEFAULT_HOST: &str = match option_env!("EARTH_TWO_DEFAULT_SPACETIMEDB_SERVER") {
    Some(host) => host,
    None => "http://127.0.0.1:3001",
};
pub const DEFAULT_DATABASE: &str = match option_env!("EARTH_TWO_DEFAULT_SPACETIMEDB_DATABASE") {
    Some(database) => database,
    None => "earth-two",
};

/// Uses the compiled public settings. Exported variables may override them
/// for local development on desktop.
#[cfg(not(target_arch = "wasm32"))]
pub fn network_defaults() -> (String, String) {
    let get = |key: &str, fallback: &str| match std::env::var(key) {
        Ok(value) if !value.is_empty() => value,
        _ => fallback.to_string(),
    };
    (
        get("SPACETIMEDB_SERVER", DEFAULT_HOST),
        get("SPACETIMEDB_DATABASE", DEFAULT_DATABASE),
    )
}
