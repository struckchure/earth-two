//! The database identity lookup from internals/account/session.go. The
//! immutable database identity is the proof's audience: a rename does not
//! change it, and another database cannot replay proofs.

use crate::Error;

/// The HTTP origin and `/v1/database/<name>/identity` URL for `host`, which
/// may use an `http`, `https`, `ws` or `wss` scheme without credentials.
pub fn identity_url(host: &str, database: &str) -> Result<(String, String), Error> {
    let invalid = || Error::new("account: invalid SpacetimeDB host");
    let (scheme, rest) = host.split_once("://").ok_or_else(invalid)?;
    let scheme = match scheme {
        "ws" => "http",
        "wss" => "https",
        "http" | "https" => scheme,
        _ => return Err(Error::new("account: invalid host scheme")),
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.contains('@') || authority.contains(['/', '\\', ' ']) {
        return Err(invalid());
    }
    let origin = format!("{scheme}://{authority}");
    let path = path.trim_end_matches('/');
    Ok((
        origin.clone(),
        format!(
            "{origin}{path}/v1/database/{}/identity",
            path_escape(database)
        ),
    ))
}

/// Go's url.PathEscape: a path segment keeps unreserved and sub-delimiter
/// characters, escaping everything else, including `/`, `;`, `,` and `?`.
pub fn path_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let keep = b.is_ascii_alphanumeric() || b"-_.~".contains(&b) || b"$&+:=@!'()*".contains(&b);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Turns the endpoint's text (bare or JSON-quoted hex, big-endian display
/// order) into the raw little-endian bytes proofs use.
pub fn parse_database_identity(body: &str) -> Result<[u8; 32], Error> {
    let text = body.trim();
    let text = if text.starts_with('"') {
        serde_json::from_str::<String>(text)
            .map_err(|_| Error::new("account: invalid database identity"))?
    } else {
        text.to_string()
    };
    let raw = hex::decode(text).map_err(|_| Error::new("account: invalid database identity"))?;
    if raw.len() != 32 {
        return Err(Error::new("account: invalid database identity"));
    }
    let mut out = [0u8; 32];
    for (i, b) in raw.iter().enumerate() {
        out[31 - i] = *b;
    }
    Ok(out)
}

/// Explains a 404 the way Go does, after probing `/v1/ping` to tell a wrong
/// server from a missing module.
pub fn missing_database_error(host: &str, database: &str, ping_ok: Option<bool>) -> Error {
    match ping_ok {
        Some(false) => Error::new(format!("{host} is not the game database server")),
        _ => Error::new(format!(
            "game database {database:?} is not published; start it with make server"
        )),
    }
}

/// Fetches the database identity over HTTP on desktop.
#[cfg(not(target_arch = "wasm32"))]
pub fn database_identity(
    host: &str,
    database: &str,
    timeout: std::time::Duration,
) -> Result<[u8; 32], Error> {
    let (origin, url) = identity_url(host, database)?;
    let agent = ureq::config::Config::builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .new_agent();
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|e| Error::new(e.to_string()))?;
    let status = response.status().as_u16();
    if status == 404 {
        let ping_ok = agent
            .get(format!("{origin}/v1/ping"))
            .call()
            .ok()
            .map(|r| r.status().as_u16() == 200);
        return Err(missing_database_error(host, database, ping_ok));
    }
    if status != 200 {
        return Err(Error::new(format!(
            "game database {database:?} at {host} returned {status}"
        )));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(257)
        .read_to_string()
        .map_err(|e| Error::new(e.to_string()))?;
    parse_database_identity(&body)
}
