# Publishing OTA releases

Release manifests and channel selections live in the shared game SpacetimeDB
database. Downloads live in S3. `release.py` packages tested builds, verifies and
uploads immutable files, registers a manifest, then promotes a channel. It uses
Python 3.12 and AWS CLI v2, with no Python package dependencies.

Deploy the server's OTA tables and reducers before using `publish`; their exact
interface is in [the server contract](../../docs/ota-server-contract.md). This
workflow never replaces the database module or clears its data.

## Local configuration

An ignored `.env` containing dummy values has been created for this checkout.
Fill those values; `.env.example` is the tracked template for other checkouts.
The tool reads assignments without executing shell code, and existing environment
variables take precedence. Tokens belong only in publisher/server configuration.

| Variable | Value |
| --- | --- |
| `AWS_S3_ACCESS_KEY_ID` | Publisher IAM access key |
| `AWS_S3_SECRET_ACCESS_KEY` | Its secret key |
| `AWS_S3_SESSION_TOKEN` | Temporary credential token, or empty |
| `AWS_S3_ENDPOINT` | HTTPS S3-compatible API endpoint |
| `AWS_S3_REGION` | S3 signing region |
| `AWS_S3_BUCKET` | Release bucket name |
| `OTA_S3_PREFIX` | `earth-two`, matching the storage stack parameter |
| `AWS_S3_PUBLIC_URL` | HTTPS CloudFront/custom download origin without the key prefix |
| `SPACETIMEDB_SERVER` | Same host as the game database; HTTP is allowed only on loopback |
| `SPACETIMEDB_DATABASE` | Same database as game logging and accounts |
| `SPACETIMEDB_PUBLISH_TOKEN` | Token for an explicitly authorized publisher identity |

Real publishing rejects dummy values. `--dry-run` accepts placeholders and makes
no network calls. The uploader maps `AWS_S3_*` credentials into standard AWS variables only for
its CLI subprocess and sends the configured endpoint on every S3 request.

## Storage setup

[infra/ota-storage.json](../../infra/ota-storage.json) defines a private, encrypted,
versioned S3 bucket, CloudFront downloads with origin access control, CORS for
game assets, and a publisher IAM policy limited to reading and writing objects
under `earth-two/releases/`. The bucket is retained on stack deletion. AWS
resources have not been created.

Deploy using infrastructure credentials when ready:

```sh
aws cloudformation deploy --template-file infra/ota-storage.json \
  --stack-name earth-two-ota --capabilities CAPABILITY_IAM
aws cloudformation describe-stacks --stack-name earth-two-ota \
  --query 'Stacks[0].Outputs'
```

Use stack outputs for `AWS_S3_BUCKET`, `AWS_S3_PUBLIC_URL` and `OTA_S3_PREFIX`.
Set `AWS_S3_ENDPOINT` to the regional S3 API endpoint for that bucket. An existing
S3-compatible provider can use its own endpoint and public download origin instead.
Attach the output publisher policy to the IAM identity used by CI. That narrow
policy cannot create the stack. Wait for the distribution to finish deploying
before publishing. CloudFront accesses the private bucket through origin access
control. [AWS origin access documentation](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/private-content-restricting-access-to-s3.html).

The bucket policy requires conditional writes to prevent overwriting release keys.
The tool sends that condition on every upload. An existing
object is accepted only when its checksum, size, type and metadata match. Failed
uploads never advance a channel. [S3 conditional writes](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html).

The tool uploads raw browser files with correct MIME types and lets the CDN
handle transport compression. `.gz` sidecars stay out of S3 releases; the current
web server continues using them for its own builds. No bucket-wide sync, delete,
public ACL or cache invalidation is needed.

## Package and publish

### Authorize the CI publisher

Use a separate SpacetimeDB identity for CI. With the trusted database owner's
token, call `POST /v1/database/<database>/call/authorize_ota_publisher`, using
`Authorization: Bearer <owner token>` and `Content-Type: application/json`:

```json
["<publisher identity as 64 lowercase hexadecimal characters>", true]
```

Set `SPACETIMEDB_PUBLISH_TOKEN` to that publisher's token, not the owner's token.
Passing `false` revokes the publisher. New databases establish the administrator
through the module's trusted initialization hook. When adding OTA to an existing
database, the database owner first configures module environment
`OTA_ADMIN_IDENTITY` to their full identity and uses that identity to authorize
the publisher. The server deployment owns this configuration; public clients
cannot claim the role.

### Build the release

Build the game, then package its executable and `assets/`. Packaging includes
only these inputs even if the directory contains dependency caches. ZIPs preserve
Unix executable permissions. Use the actual build OS and architecture, such as
`darwin-arm64` on an Apple silicon Mac:

```sh
make build
mkdir -p build/release-input
cp build/earth-two build/release-input/
cp -R build/assets build/release-input/assets
python3 tools/ota/release.py package \
  --root build/release-input --out build/ota-incoming/desktop \
  --platform darwin-arm64 --release-id local-release-1 \
  --commit "$(git rev-parse HEAD)" --version 0.1.0
```

For the browser:

```sh
make web
python3 tools/ota/release.py package \
  --root build/web --out build/ota-incoming/web \
  --platform web --release-id local-release-1 \
  --commit "$(git rev-parse HEAD)" --version 0.1.0
```

Use clean output directories and unique release IDs. All fragments must have the
same ID, version and commit. CI packages all desktop OSes and the browser; local
releases can contain fewer platforms.

```sh
python3 tools/ota/release.py merge --root build/ota-incoming --out build/ota-release
python3 tools/ota/release.py publish --root build/ota-release --channel beta --dry-run
python3 tools/ota/release.py publish --root build/ota-release --channel beta
```

Publishing captures the channel generation before uploading, sends SHA-256
checksums with S3 writes, verifies stored objects, and checks every public download
URL. Only then does it register the exact manifest bytes and promote with that
generation. Concurrent promotions conflict instead of overwriting one another.
Retry an interrupted release using the same files and ID. Inspect the channel
after an uncertain network result.

Promote a tested release to stable, or roll stable back to an older release:

```sh
python3 tools/ota/release.py promote --release-id RELEASE_ID --channel stable
```

Rollback increments the channel generation while selecting an older release.
It never rebuilds or deletes files. Keep old releases available for recovery.

## GitHub Actions setup

CI always tests the tools and packages browser, Linux, macOS and Windows releases.
Publishing runs after all build/test jobs pass, only on `main`, and only with
repository variable `OTA_PUBLISH_ENABLED=true`. Superseded `main` builds skip
promotion. The default publishing channel is `beta`.

Create an `ota-release` GitHub environment. Configure its secrets:

- `AWS_S3_ACCESS_KEY_ID`
- `AWS_S3_SECRET_ACCESS_KEY`
- `AWS_S3_SESSION_TOKEN`, if applicable
- `SPACETIMEDB_PUBLISH_TOKEN`

Configure environment or repository variables:

- `AWS_S3_ENDPOINT`, `AWS_S3_REGION`, `AWS_S3_BUCKET`, `AWS_S3_PUBLIC_URL`
- `OTA_S3_PREFIX=earth-two`
- `SPACETIMEDB_SERVER`, `SPACETIMEDB_DATABASE`
- `OTA_RELEASE_CHANNEL=beta`; use `stable` for automatic stable rollout
- `OTA_GAME_VERSION`, optional; defaults to the source commit

Finally set **repository** variable `OTA_PUBLISH_ENABLED=true` after setup is
complete. It must be at repository scope because CI evaluates it before entering
the publishing environment. Filling local `.env` does not configure GitHub.
[GitHub Actions secrets](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets).

Release IDs include commit, run ID and attempt. CI merges and verifies every
platform before uploading. Publishing jobs are serialized; generation checks
also guard against concurrent local publishers. PR jobs receive no credentials.

## Verification and client integration

```sh
python3 -m unittest discover -s tools/ota -p 'test_*.py' -v
actionlint .github/workflows/ci.yml
cfn-lint infra/ota-storage.json
```

These changes deliver packaging, upload and manifest rollout. Browser refresh
behavior stays as it is today. The desktop launcher that downloads and installs
the selected release is a later phase in [the OTA plan](../../docs/ota-updates.md).
These tools do not replace the running game. Independent native-update signing
and startup recovery also belong to that launcher phase.

Verification on 2026-10-08: the game test suite, release-tool tests, actionlint
and cfn-lint passed. Real macOS and browser builds were packaged and merged. The
actual release tool was exercised against the server session's SpacetimeDB 2.11.0
module in a disposable local database: registration, promotion, exact manifest
bytes, retries, publisher authorization and generation conflicts passed. S3
uploads and public download checks were mocked for that database test; no AWS
resources or production releases were created.
