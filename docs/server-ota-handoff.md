# OTA database handoff

Status: implemented and tested against a disposable local SpacetimeDB 2.11.0
database. No S3, production database, `.env`, or OTA-branch CI changes were made.

## Module and build

The working module is `internals/server/module`, using the official Rust SDK
2.11.0. Native Go clients use DigitalXero's client SDK 0.6.2. Account ownership
is separate from the release-publisher role.

```sh
CARGO_TARGET_DIR="$PWD/build/server/target" cargo build \
  --manifest-path internals/server/module/Cargo.toml \
  --target wasm32-unknown-unknown --release --locked
spacetime publish "$SPACETIMEDB_DATABASE" \
  --server "$SPACETIMEDB_SERVER" \
  --bin-path build/server/target/wasm32-unknown-unknown/release/earth_two_server.wasm
```

Publish with the intended database owner's credentials. The server deployment
owns module publication; the OTA uploader only calls reducers after S3 verification.

## Authorization setup

On first publication, the trusted `init` hook records the publishing owner as
OTA administrator in private `ota_authority`, and authorizes that identity in
private `ota_publisher`. Ordinary callers cannot initialize or claim the role.

Use a separate CI publisher identity. With the owner's Bearer token, call:

```text
POST /v1/database/<database>/call/authorize_ota_publisher
Authorization: Bearer <owner token>
Content-Type: application/json

["<publisher's full 64-character lowercase SpacetimeDB identity>", true]
```

Passing `false` revokes its publishing role. This reducer accepts the identity
as a **hex string**, not a SATS identity object. The CI token represented by that
identity becomes `SPACETIMEDB_PUBLISH_TOKEN`. Never put owner or publisher tokens
in the public tables or client bundles.

For an existing database first receiving these tables, the trusted database
owner must set module environment `OTA_ADMIN_IDENTITY` to their full identity
when publishing the module, then call `authorize_ota_publisher` with that
identity's token. If no authority row and no trusted configuration exist, it
fails closed. The setting is optional for a new database because `init` establishes
the owner. Once initialized, the private authority row controls grants; changing
this environment value does not replace an existing administrator.

The host verifies the caller's JWT and supplies its authenticated identity.
Requests without the publisher's token receive an unrelated anonymous identity
and fail the private allowlist check. Player signing keys do not confer release
publishing privileges.

## Contract compatibility

Public SQL tables and column names match `ota-server-contract.md` exactly:

- `ota_release(release_id, manifest_json, manifest_sha256, release_base_url, created_at)`
- `ota_channel(channel, release_id, generation, previous_release_id, updated_at)`

Both required reducer argument arrays are unchanged:

```text
publish_ota_release(release_id, manifest_json, manifest_sha256, release_base_url)
promote_ota_channel(channel, release_id, expected_generation)
```

The timestamp fields are host-assigned `Timestamp` values. Manifests use schema
1, maximum 1 MiB, and a digest of the exact supplied UTF-8 bytes. Validation
rejects unsupported structure, unsafe paths, duplicate platforms/artifacts,
invalid sizes/digests, invalid entrypoints, and credential-bearing or non-HTTPS
download bases. Bases must end with `/releases/<release_id>`.

Releases are immutable. An identical retry is a no-op; changed JSON, digest or
base is rejected. Publication never promotes a channel. Channel promotion
checks the expected generation transactionally, starts at generation 1, records
the previous release, and checks overflow. Promoting the same target is a no-op
only when its expected generation matches. Rollback promotes the old release
with the current generation and advances the counter.

## Verification

```sh
CARGO_TARGET_DIR="$PWD/build/server/target" cargo test \
  --manifest-path internals/server/module/Cargo.toml --lib --locked
python3 tools/server/ota_integration.py --host http://127.0.0.1:3000 \
  --wasm build/server/target/wasm32-unknown-unknown/release/earth_two_server.wasm
```

The integration script creates a randomly named disposable local database and
fresh owner/publisher/player tokens in memory. It tests the actual HTTP JSON
argument arrays, public SQL queries, anonymous/player rejection, grants and
revocation, immutable retries, publication without promotion, missing releases,
channel validation, generation conflicts, same-target retries, rollback,
manifest digest/size validation, path traversal, invalid numeric types, and
credential-bearing download URLs. It never prints tokens or writes to S3.

Rust validation tests and this HTTP integration suite passed on 2026-10-08.
The OTA branch can now test its existing publisher against the built module.
