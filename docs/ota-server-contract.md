# OTA server integration contract

Use the existing game SpacetimeDB database for update metadata. S3 stores
immutable downloads; the database stores manifests and the release selected by
each channel. The OTA branch owns `tools/ota`, S3 configuration and release CI.
The server branch owns these tables and reducers in its working module.

## Public tables

`ota_release`:

- `release_id: string`, primary key
- `manifest_json: string`, immutable JSON described below
- `manifest_sha256: string`, lowercase SHA-256 of the exact UTF-8 JSON bytes
- `release_base_url: string`, HTTPS URL ending in `/releases/<release_id>`
- `created_at: Timestamp`, assigned by the reducer

`ota_channel`:

- `channel: string`, primary key; allow only `preview`, `beta`, `stable`
- `release_id: string`, references an existing `ota_release`
- `generation: u64`, starts at 1 and increments on every promotion
- `previous_release_id: string`, empty for the first promotion
- `updated_at: Timestamp`, assigned by the reducer

Keep credentials, publisher authorization and tokens in private tables or module
configuration. The public rows contain no secrets. Table and column names above
are the HTTP SQL contract, regardless of module implementation language.

## Reducers

`publish_ota_release(release_id: string, manifest_json: string,
manifest_sha256: string, release_base_url: string)`:

1. Require the caller's authenticated SpacetimeDB identity to be explicitly
   authorized for release publishing. Anonymous callers and ordinary players
   must fail. Initialize authorization through a trusted module-owner mechanism;
   never let the first caller claim the publisher role.
2. Validate JSON structure, schema version, matching release ID, the digest of
   the exact supplied bytes, HTTPS download base, and safe relative artifact
   paths. Limit JSON to 1 MiB. Release IDs match `[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}`.
3. Insert an immutable release. An identical retry is a no-op; reject attempts
   to reuse a release ID with different JSON, digest or base URL.
4. Do not promote a channel in this reducer. The uploader calls it only after
   every artifact and the manifest have been uploaded and verified in S3.

`promote_ota_channel(channel: string, release_id: string,
expected_generation: u64)`:

1. Require the same release authorization and a supported channel.
2. Require an existing release. Treat a missing channel as generation 0.
3. Reject if the current generation differs from `expected_generation`.
4. Atomically set the target, preserve the previous release, and increment the
   generation with overflow protection. Promoting the current target is a no-op
   when the expected generation matches, so an idempotent retry is harmless.
5. Rollback uses this same reducer with an older release ID and a fresh expected
   generation. Do not decrement generation or delete releases.

SpacetimeDB executes reducers transactionally and exposes them through
`POST /v1/database/<database>/call/<reducer>` with a JSON argument array and
`Authorization: Bearer <publisher token>`. The release tool uses this HTTP API
and bounded read-only SQL queries against the public tables.
[SpacetimeDB HTTP API](https://spacetimedb.com/docs/http/database/),
[reducer transactions](https://spacetimedb.com/docs/functions/reducers/).

## Manifest schema version 1

```json
{
  "schema_version": 1,
  "release_id": "example-release",
  "game_version": "0.1.0",
  "source_commit": "0123456789012345678901234567890123456789",
  "targets": [
    {
      "platform": "linux-amd64",
      "entrypoint": "",
      "expanded_size": 12345,
      "artifacts": [
        {
          "path": "desktop/linux-amd64.zip",
          "size": 1234,
          "sha256": "0123456789012345678901234567890123456789012345678901234567890123456789",
          "content_type": "application/zip"
        }
      ]
    }
  ]
}
```

The numbers and digest above are illustrative. `targets` is nonempty with unique
platform IDs. Supported platforms are `web` or `(linux|darwin|windows)-(amd64|arm64)`.
Game versions are nonempty, at most 128 characters; source commits are 40 or 64
lowercase hexadecimal characters. `expanded_size` and artifact sizes are
nonnegative integers. Artifact paths are unique across the release, relative,
contain no backslashes, query/fragment delimiters, percent escapes, empty segments,
ASCII control characters, `.` or `..`; they begin with `web/` or `desktop/`.
Digests are 64 lowercase hex
characters. Desktop targets contain one ZIP artifact named
`desktop/<platform>.zip`. Web has `entrypoint: "web/index.html"`, and that path
must appear in its artifacts. Desktop entrypoints are empty. No direct credentials
or expiring signed URLs appear in manifests.

To download, append an artifact path to the stored `release_base_url`. The base
contains the S3 key prefix, for example
`https://downloads.example.com/earth-two/releases/example-release`. Browser raw
artifact sizes and hashes refer to uncompressed files; HTTP content encoding is
a transport detail. CI combines the tested platform manifests before upload.

## Configuration and handoff

The publisher uses `SPACETIMEDB_SERVER`, `SPACETIMEDB_DATABASE` and the secret
`SPACETIMEDB_PUBLISH_TOKEN`. Use the same server and database variables as the
logging work. The server branch should document how to authorize the identity
represented by that token. The OTA branch never publishes or replaces the
database module; that belongs to the server's deployment workflow.

Please put final module location, authorization setup, reducer compatibility and
verification results in `docs/server-ota-handoff.md` in the server checkout. The
OTA branch will verify its publisher against that implementation before delivery.

The server session implemented this contract in
`internals/server/module/src/ota.rs`. Paired verification on 2026-10-08 used the
actual OTA release tool with real packaged desktop/browser manifests against a
new disposable local SpacetimeDB 2.11.0 database. HTTP registration, promotion,
public SQL reads, identical retries, publisher authorization and generation
conflicts passed. S3 calls were mocked because AWS credentials remain placeholders.
