use serde::Deserialize;
use sha2::{Digest, Sha256};
use spacetimedb::{Identity, ReducerContext, Table, Timestamp};
use std::collections::HashSet;
use url::Url;

// Module environment is managed by database owners, never ordinary clients.
// This optional bootstrap also supports adding OTA to an existing database.
#[spacetimedb::env]
#[allow(dead_code)]
pub struct Env {
    pub OTA_ADMIN_IDENTITY: Option<String>,
}

#[spacetimedb::table(accessor = ota_authority)]
pub struct OtaAuthority {
    #[primary_key]
    pub id: u8,
    pub owner: Identity,
}

#[spacetimedb::table(accessor = ota_publisher)]
pub struct OtaPublisher {
    #[primary_key]
    pub identity: Identity,
}

#[spacetimedb::table(accessor = ota_release, public)]
pub struct OtaRelease {
    #[primary_key]
    pub release_id: String,
    pub manifest_json: String,
    pub manifest_sha256: String,
    pub release_base_url: String,
    pub created_at: Timestamp,
}

#[spacetimedb::table(accessor = ota_channel, public)]
pub struct OtaChannel {
    #[primary_key]
    pub channel: String,
    pub release_id: String,
    pub generation: u64,
    pub previous_release_id: String,
    pub updated_at: Timestamp,
}

#[spacetimedb::reducer(init)]
pub fn init_ota(ctx: &ReducerContext) -> Result<(), String> {
    let owner = configured_owner(ctx)?.unwrap_or(ctx.sender());
    ctx.db.ota_authority().insert(OtaAuthority { id: 0, owner });
    ctx.db
        .ota_publisher()
        .insert(OtaPublisher { identity: owner });
    Ok(())
}

fn configured_owner(ctx: &ReducerContext) -> Result<Option<Identity>, String> {
    ctx.env
        .OTA_ADMIN_IDENTITY()
        .map(|value| {
            if !lower_hex(&value, 64) {
                return Err("OTA_ADMIN_IDENTITY must be a full lowercase identity".into());
            }
            Identity::from_hex(value).map_err(|_| "invalid OTA_ADMIN_IDENTITY".into())
        })
        .transpose()
}

#[spacetimedb::reducer]
pub fn authorize_ota_publisher(
    ctx: &ReducerContext,
    identity: String,
    authorized: bool,
) -> Result<(), String> {
    let authority = ctx.db.ota_authority().id().find(0);
    let owner = match authority {
        Some(ref row) => row.owner,
        None => configured_owner(ctx)?.ok_or(
            "OTA authority is not initialized; database owner must configure OTA_ADMIN_IDENTITY",
        )?,
    };
    if ctx.sender() != owner {
        return Err("only the trusted OTA administrator can authorize publishers".into());
    }
    if !lower_hex(&identity, 64) {
        return Err("publisher identity must be 64 lowercase hex characters".into());
    }
    let identity = Identity::from_hex(identity).map_err(|_| "invalid publisher identity")?;
    if identity == Identity::ZERO {
        return Err("zero identity cannot publish releases".into());
    }
    if authority.is_none() {
        ctx.db.ota_authority().insert(OtaAuthority { id: 0, owner });
    }
    if authorized {
        if ctx.db.ota_publisher().identity().find(identity).is_none() {
            ctx.db.ota_publisher().insert(OtaPublisher { identity });
        }
    } else {
        ctx.db.ota_publisher().identity().delete(identity);
    }
    Ok(())
}

fn require_publisher(ctx: &ReducerContext) -> Result<(), String> {
    // The host validates the Bearer token before assigning ctx.sender(). An
    // HTTP call without it gets a new anonymous identity, never a publisher's.
    if ctx.sender() == Identity::ZERO
        || ctx
            .db
            .ota_publisher()
            .identity()
            .find(ctx.sender())
            .is_none()
    {
        return Err("authenticated OTA publisher required".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u64,
    release_id: String,
    game_version: String,
    source_commit: String,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    platform: String,
    entrypoint: String,
    expanded_size: u64,
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: String,
    size: u64,
    sha256: String,
    content_type: String,
}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn release_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn platform(value: &str) -> bool {
    value == "web"
        || ["linux", "darwin", "windows"].iter().any(|os| {
            ["amd64", "arm64"]
                .iter()
                .any(|arch| value == format!("{os}-{arch}"))
        })
}

fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && !value
            .bytes()
            .any(|b| b.is_ascii_control() || b"\\%?#".contains(&b))
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn validate(id: &str, json: &str, digest: &str, base: &str) -> Result<(), String> {
    if !release_id(id) || json.len() > 1024 * 1024 || !lower_hex(digest, 64) {
        return Err("invalid release ID, digest, or manifest size".into());
    }
    if hex::encode(Sha256::digest(json.as_bytes())) != digest {
        return Err("manifest digest mismatch".into());
    }
    let url = Url::parse(base).map_err(|_| "invalid release download URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().ends_with(&format!("/releases/{id}"))
        || base.chars().any(|c| c.is_control())
        || !base.ends_with(&format!("/releases/{id}"))
    {
        return Err(
            "release base must be credential-free HTTPS ending in /releases/<release_id>".into(),
        );
    }
    let m: Manifest = serde_json::from_str(json).map_err(|_| "invalid manifest schema")?;
    if m.schema_version != 1
        || m.release_id != id
        || m.game_version.is_empty()
        || m.game_version.chars().count() > 128
        || !(lower_hex(&m.source_commit, 40) || lower_hex(&m.source_commit, 64))
        || m.targets.is_empty()
    {
        return Err("unsupported or inconsistent manifest".into());
    }
    let mut platforms = HashSet::new();
    let mut paths = HashSet::new();
    for t in m.targets {
        if !platform(&t.platform) || !platforms.insert(t.platform.clone()) || t.artifacts.is_empty()
        {
            return Err("invalid or duplicate target platform".into());
        }
        let _expanded_size = t.expanded_size; // deserializing u64 rejects negatives, fractions, and booleans
        let mut entrypoint_found = false;
        if t.platform == "web" {
            if t.entrypoint != "web/index.html" {
                return Err("web target requires web/index.html".into());
            }
        } else if !t.entrypoint.is_empty()
            || t.artifacts.len() != 1
            || t.artifacts[0].path != format!("desktop/{}.zip", t.platform)
        {
            return Err("desktop target requires one correctly named ZIP artifact".into());
        }
        for a in t.artifacts {
            let prefix = if t.platform == "web" {
                "web/"
            } else {
                "desktop/"
            };
            if !safe_path(&a.path)
                || !a.path.starts_with(prefix)
                || !paths.insert(a.path.clone())
                || !lower_hex(&a.sha256, 64)
                || a.content_type.is_empty()
                || a.content_type.bytes().any(|b| b.is_ascii_control())
            {
                return Err("unsafe, duplicate, or invalid artifact".into());
            }
            let _size = a.size;
            entrypoint_found |= a.path == t.entrypoint;
        }
        if t.platform == "web" && !entrypoint_found {
            return Err("web entrypoint artifact is missing".into());
        }
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn publish_ota_release(
    ctx: &ReducerContext,
    release_id: String,
    manifest_json: String,
    manifest_sha256: String,
    release_base_url: String,
) -> Result<(), String> {
    require_publisher(ctx)?;
    validate(
        &release_id,
        &manifest_json,
        &manifest_sha256,
        &release_base_url,
    )?;
    if let Some(old) = ctx.db.ota_release().release_id().find(&release_id) {
        if old.manifest_json == manifest_json
            && old.manifest_sha256 == manifest_sha256
            && old.release_base_url == release_base_url
        {
            return Ok(());
        }
        return Err("release ID already exists with different immutable contents".into());
    }
    ctx.db.ota_release().insert(OtaRelease {
        release_id,
        manifest_json,
        manifest_sha256,
        release_base_url,
        created_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn promote_ota_channel(
    ctx: &ReducerContext,
    channel: String,
    release_id: String,
    expected_generation: u64,
) -> Result<(), String> {
    require_publisher(ctx)?;
    if !["preview", "beta", "stable"].contains(&channel.as_str()) {
        return Err("unsupported OTA channel".into());
    }
    if ctx
        .db
        .ota_release()
        .release_id()
        .find(&release_id)
        .is_none()
    {
        return Err("OTA release does not exist".into());
    }
    let old = ctx.db.ota_channel().channel().find(&channel);
    let generation = old.as_ref().map_or(0, |row| row.generation);
    if generation != expected_generation {
        return Err("OTA channel generation conflict".into());
    }
    if old.as_ref().is_some_and(|row| row.release_id == release_id) {
        return Ok(());
    }
    let next = generation
        .checked_add(1)
        .ok_or("OTA channel generation exhausted")?;
    let row = OtaChannel {
        channel,
        release_id,
        generation: next,
        previous_release_id: old
            .as_ref()
            .map(|r| r.release_id.clone())
            .unwrap_or_default(),
        updated_at: ctx.timestamp,
    };
    if old.is_some() {
        ctx.db.ota_channel().channel().update(row);
    } else {
        ctx.db.ota_channel().insert(row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> serde_json::Value {
        serde_json::json!({"schema_version":1,"release_id":"test-release","game_version":"0.1.0","source_commit":"a".repeat(40),
            "targets":[{"platform":"web","entrypoint":"web/index.html","expanded_size":1,"artifacts":[{"path":"web/index.html","size":1,"sha256":"b".repeat(64),"content_type":"text/html"}]}]})
    }
    fn check(m: &serde_json::Value) -> Result<(), String> {
        let json = m.to_string();
        validate(
            "test-release",
            &json,
            &hex::encode(Sha256::digest(json.as_bytes())),
            "https://downloads.example.com/earth-two/releases/test-release",
        )
    }
    #[test]
    fn manifest_rejects_unsafe_paths_and_invalid_types() {
        assert!(check(&manifest()).is_ok());
        for path in [
            "web/../index.html",
            "web//index.html",
            "web/%2e%2e/index.html",
            "web\\index.html",
            "/web/index.html",
            "web/index.html?token=x",
            "web/index.html#x",
        ] {
            let mut m = manifest();
            m["targets"][0]["artifacts"][0]["path"] = path.into();
            assert!(check(&m).is_err(), "{path}");
        }
        for value in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(true),
        ] {
            let mut m = manifest();
            m["targets"][0]["artifacts"][0]["size"] = value;
            assert!(check(&m).is_err());
        }
        let mut m = manifest();
        let duplicate = m["targets"][0]["artifacts"][0].clone();
        m["targets"][0]["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(check(&m).is_err());
    }
}
