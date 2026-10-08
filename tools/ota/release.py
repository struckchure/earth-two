#!/usr/bin/env python3
"""Package tested builds, upload immutable S3 files, then promote SpacetimeDB metadata."""

import argparse
import base64
import hashlib
import json
import mimetypes
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile


CHANNELS = {"preview", "beta", "stable"}
ID = re.compile(r"[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}\Z")
PLATFORM = re.compile(r"(?:web|(?:linux|darwin|windows)-(?:amd64|arm64))\Z")
HEX = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")
MAX_MANIFEST = 1024 * 1024
MAX_OBJECT = 5 * 1024**3  # Single PutObject; fail before uploading oversized releases.


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def safe_path(value):
    return (isinstance(value, str) and bool(value) and
            not any(c in value for c in "\\%?#") and
            not any(ord(c) < 32 or ord(c) == 127 for c in value) and
            all(part not in {"", ".", ".."} for part in value.split("/")))


def validate(manifest):
    if (not isinstance(manifest, dict) or type(manifest.get("schema_version")) is not int or
            manifest["schema_version"] != 1):
        raise ValueError("unsupported manifest schema")
    if set(manifest) != {"schema_version", "release_id", "game_version", "source_commit", "targets"}:
        raise ValueError("unknown or missing manifest field")
    if not isinstance(manifest.get("release_id"), str) or not ID.fullmatch(manifest["release_id"]):
        raise ValueError("invalid release ID")
    if not isinstance(manifest.get("source_commit"), str) or not COMMIT.fullmatch(manifest["source_commit"]):
        raise ValueError("invalid source commit")
    version = manifest.get("game_version")
    if not isinstance(version, str) or not 1 <= len(version) <= 128:
        raise ValueError("invalid game version")
    targets = manifest.get("targets")
    if not isinstance(targets, list) or not targets:
        raise ValueError("no release targets")
    platforms, paths = set(), set()
    for target in targets:
        if not isinstance(target, dict):
            raise ValueError("invalid target")
        if set(target) != {"platform", "entrypoint", "expanded_size", "artifacts"}:
            raise ValueError("unknown or missing target field")
        platform = target.get("platform", "")
        if not isinstance(platform, str) or not PLATFORM.fullmatch(platform) or platform in platforms:
            raise ValueError("invalid or duplicate platform")
        platforms.add(platform)
        size = target.get("expanded_size")
        if type(size) is not int or not 0 <= size < 2**64:
            raise ValueError("invalid expanded size")
        artifacts = target.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            raise ValueError("no target artifacts")
        target_paths = set()
        for artifact in artifacts:
            if not isinstance(artifact, dict):
                raise ValueError("invalid artifact")
            if set(artifact) != {"path", "size", "sha256", "content_type"}:
                raise ValueError("unknown or missing artifact field")
            path = artifact.get("path")
            prefix = "web/" if platform == "web" else "desktop/"
            if not safe_path(path) or not path.startswith(prefix) or path in paths:
                raise ValueError("unsafe or duplicate artifact path")
            paths.add(path)
            target_paths.add(path)
            if type(artifact.get("size")) is not int or not 0 <= artifact["size"] <= MAX_OBJECT:
                raise ValueError("invalid artifact size")
            if not isinstance(artifact.get("sha256"), str) or not HEX.fullmatch(artifact["sha256"]):
                raise ValueError("invalid artifact checksum")
            mime = artifact.get("content_type")
            if not isinstance(mime, str) or not mime or any(ord(c) < 32 or ord(c) == 127 for c in mime):
                raise ValueError("invalid artifact content type")
        if platform == "web":
            if target.get("entrypoint") != "web/index.html" or "web/index.html" not in target_paths:
                raise ValueError("missing browser entrypoint")
        elif target.get("entrypoint") != "" or target_paths != {f"desktop/{platform}.zip"}:
            raise ValueError("invalid desktop package")
    if len(canonical(manifest)) > MAX_MANIFEST:
        raise ValueError("manifest exceeds 1 MiB")


def read_manifest(path):
    if path.stat().st_size > MAX_MANIFEST:
        raise ValueError("manifest exceeds 1 MiB")
    result = json.loads(path.read_bytes())
    validate(result)
    return result


def source_files(root):
    if root.is_symlink() or not root.is_dir():
        raise ValueError("build root must be a real directory")
    files = []
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            path = Path(directory) / name
            if path.is_symlink():
                raise ValueError("release inputs must not contain symlinks")
            if not path.is_dir():
                if not path.is_file() or not safe_path(path.relative_to(root).as_posix()):
                    raise ValueError("release input is not a safe regular file")
                files.append(path)
    return sorted(files)


def artifact(root, relative):
    path = root / relative
    mime = {".wasm": "application/wasm", ".js": "text/javascript",
            ".data": "application/octet-stream", ".zip": "application/zip"}.get(path.suffix)
    return {"path": relative, "size": path.stat().st_size, "sha256": digest(path),
            "content_type": mime or mimetypes.guess_type(path.name)[0] or "application/octet-stream"}


def verify_files(manifest, root):
    validate(manifest)
    for target in manifest["targets"]:
        for item in target["artifacts"]:
            path = root / item["path"]
            if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(root.resolve()):
                raise ValueError("missing or unsafe artifact")
            if path.stat().st_size != item["size"] or digest(path) != item["sha256"]:
                raise ValueError("artifact size or digest mismatch")


def package(args):
    if not PLATFORM.fullmatch(args.platform):
        raise ValueError("unsupported platform")
    if args.root.is_symlink() or not args.root.is_dir():
        raise ValueError("build root must be a real directory")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    if args.out.exists():
        raise ValueError("output already exists; use a clean release directory")
    with tempfile.TemporaryDirectory(prefix="ota-", dir=args.out.parent) as scratch:
        out = Path(scratch) / "release"
        out.mkdir()
        target = {"platform": args.platform, "entrypoint": "", "expanded_size": 0, "artifacts": []}
        if args.platform == "web":
            required = {"index.html", "game.wasm", "wasm_exec.js", "fs.js", "raylib.js",
                        "raylib.wasm", "raylib.data", "jolt.js", "jolt.wasm"}
            files = source_files(args.root)
            if not required.issubset({p.relative_to(args.root).as_posix() for p in files}):
                raise ValueError("browser build is incomplete")
            for path in files:
                if path.suffix == ".gz":  # CDN transport compression; keep canonical raw bytes.
                    continue
                relative = "web/" + path.relative_to(args.root).as_posix()
                (out / relative).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, out / relative)
                target["artifacts"].append(artifact(out, relative))
                target["expanded_size"] += path.stat().st_size
            target["entrypoint"] = "web/index.html"
        else:
            binary = args.root / ("earth-two.exe" if args.platform.startswith("windows-") else "earth-two")
            if binary.is_symlink() or not binary.is_file():
                raise ValueError("desktop executable is missing")
            files = [binary] + source_files(args.root / "assets")
            relative = f"desktop/{args.platform}.zip"
            (out / "desktop").mkdir()
            with zipfile.ZipFile(out / relative, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
                for path in files:
                    name = path.relative_to(args.root).as_posix()
                    info = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
                    info.create_system = 3
                    mode = 0o755 if path == binary and not args.platform.startswith("windows-") else 0o644
                    info.external_attr = (stat.S_IFREG | mode) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    with path.open("rb") as src, archive.open(info, "w", force_zip64=True) as dst:
                        shutil.copyfileobj(src, dst)
                    target["expanded_size"] += path.stat().st_size
            target["artifacts"] = [artifact(out, relative)]
        manifest = {"schema_version": 1, "release_id": args.release_id,
                    "game_version": args.version or args.release_id, "source_commit": args.commit,
                    "targets": [target]}
        verify_files(manifest, out)
        (out / "manifest.json").write_bytes(canonical(manifest))
        out.rename(args.out)
    print(f"Packaged {args.platform}: {args.out}")


def merge(args):
    fragments = sorted(args.root.glob("*/manifest.json"))
    if not fragments:
        raise ValueError("no platform manifests")
    manifests = [read_manifest(p) for p in fragments]
    result = {k: v for k, v in manifests[0].items() if k != "targets"}
    result["targets"] = []
    for manifest, path in zip(manifests, fragments):
        if any(manifest[k] != result[k] for k in ("schema_version", "release_id", "game_version", "source_commit")):
            raise ValueError("platform manifests refer to different releases")
        verify_files(manifest, path.parent)
        result["targets"].extend(manifest["targets"])
    result["targets"].sort(key=lambda t: t["platform"])
    validate(result)
    platforms = {t["platform"] for t in result["targets"]}
    if args.require_all and ("web" not in platforms or any(
            not any(p.startswith(name + "-") for p in platforms) for name in ("linux", "darwin", "windows"))):
        raise ValueError("web, Linux, macOS and Windows builds are required")
    if args.out.exists():
        raise ValueError("output already exists; use a clean release directory")
    args.out.mkdir(parents=True)
    for manifest, path in zip(manifests, fragments):
        for target in manifest["targets"]:
            for item in target["artifacts"]:
                dest = args.out / item["path"]
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path.parent / item["path"], dest)
    (args.out / "manifest.json").write_bytes(canonical(result))
    verify_files(result, args.out)
    print(f"Merged {len(result['targets'])} platforms: {args.out}")


def load_env(path):
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        key, sep, value = line.partition("=")
        if not sep or not re.fullmatch(r"[A-Z][A-Z0-9_]*", key):
            raise ValueError("invalid .env assignment")
        value = value.strip()
        if value[:1] in {"'", '"'}:
            if value[-1:] != value[0] or len(value) < 2:
                raise ValueError("invalid quoted .env value")
            value = value[1:-1]
        os.environ.setdefault(key, value)


def configured(name, dry_run=False):
    value = os.environ.get(name, "").strip()
    if not value or (not dry_run and ("CHANGE_ME" in value or "example.invalid" in value)):
        raise ValueError(f"configure {name} before publishing")
    return value


def url_base(value, local=False):
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError("URL contains control characters")
    parsed = urllib.parse.urlsplit(value)
    loopback = parsed.hostname in {"localhost", "127.0.0.1", "::1"}
    if (not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment or
            (parsed.scheme != "https" and not (local and loopback and parsed.scheme == "http"))):
        raise ValueError("URLs must use HTTPS (HTTP only for a local database)")
    return value.rstrip("/")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None  # Never forward the publisher's token to another origin.


HTTP = urllib.request.build_opener(NoRedirect)


class Database:
    def __init__(self, server, database, token):
        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*|[a-f0-9]{64}", database):
            raise ValueError("invalid SpacetimeDB database name")
        self.base = url_base(server, local=True) + "/v1/database/" + database
        self.token = token

    def request(self, suffix, data, content_type):
        request = urllib.request.Request(self.base + suffix, data=data, headers={
            "Authorization": "Bearer " + self.token, "Content-Type": content_type}, method="POST")
        try:
            with HTTP.open(request, timeout=30) as response:
                return response.read(MAX_MANIFEST + 1)
        except urllib.error.HTTPError as error:
            raise ValueError(f"SpacetimeDB request failed (HTTP {error.code}); check schema and publisher authorization") from None

    def channel(self, channel):
        if channel not in CHANNELS:
            raise ValueError("invalid channel")
        raw = self.request("/sql", f"SELECT generation, release_id FROM ota_channel WHERE channel = '{channel}'".encode(),
                           "text/plain")
        tables = json.loads(raw)
        if not isinstance(tables, list) or len(tables) != 1 or "rows" not in tables[0]:
            raise ValueError("unexpected SpacetimeDB SQL response")
        rows = tables[0]["rows"]
        if not rows:
            return 0, ""
        if len(rows) != 1 or len(rows[0]) != 2:
            raise ValueError("unexpected channel row")
        generation, release = rows[0]
        if type(generation) is not int or not 1 <= generation < 2**64 or not isinstance(release, str):
            raise ValueError("invalid channel generation")
        return generation, release

    def call(self, reducer, args):
        self.request("/call/" + reducer, canonical(args), "application/json")


def aws(*args):
    endpoint = url_base(configured("AWS_S3_ENDPOINT"), local=True)
    region = configured("AWS_S3_REGION")
    command = ["aws", "--no-cli-pager", "--output", "json", "--endpoint-url", endpoint, "--region", region, "s3api", *args]
    environment = os.environ.copy()
    environment["AWS_ACCESS_KEY_ID"] = configured("AWS_S3_ACCESS_KEY_ID")
    environment["AWS_SECRET_ACCESS_KEY"] = configured("AWS_S3_SECRET_ACCESS_KEY")
    environment["AWS_DEFAULT_REGION"] = region
    environment.pop("AWS_SESSION_TOKEN", None)
    if token := os.environ.get("AWS_S3_SESSION_TOKEN"):
        environment["AWS_SESSION_TOKEN"] = token
    result = subprocess.run(command, env=environment, capture_output=True, text=True, check=False)
    if result.returncode:
        # Do not dump CLI or HTTP output, which may contain operational secrets.
        if "PreconditionFailed" in result.stderr:
            return None
        raise ValueError("S3 request failed; check AWS credentials, region, bucket and IAM permissions")
    return json.loads(result.stdout or "{}")


def upload(bucket, key, path, item):
    checksum = base64.b64encode(bytes.fromhex(item["sha256"])).decode()
    aws("put-object", "--bucket", bucket, "--key", key, "--body", str(path),
        "--if-none-match", "*", "--content-type", item["content_type"],
        "--cache-control", "public,max-age=31536000,immutable",
        "--metadata", "sha256=" + item["sha256"], "--checksum-sha256", checksum)
    remote = aws("head-object", "--bucket", bucket, "--key", key, "--checksum-mode", "ENABLED")
    if (not remote or remote.get("ContentLength") != item["size"] or
            remote.get("ChecksumSHA256") != checksum or
            remote.get("Metadata", {}).get("sha256") != item["sha256"] or
            remote.get("ContentType") != item["content_type"] or remote.get("ContentEncoding")):
        raise ValueError("S3 object verification failed; release ID may already have different content")


def check_download(base, item):
    url = base + "/" + urllib.parse.quote(item["path"], safe="/")
    request = urllib.request.Request(url, method="HEAD", headers={"Accept-Encoding": "identity"})
    try:
        with HTTP.open(request, timeout=30) as response:
            if int(response.headers.get("Content-Length", "-1")) != item["size"]:
                raise ValueError("download origin reports the wrong size")
            if response.headers.get("Content-Encoding"):
                raise ValueError("download origin ignored identity content encoding")
    except urllib.error.HTTPError as error:
        raise ValueError(f"download origin is not ready (HTTP {error.code}); channel was not promoted") from None


def publish(args):
    load_env(args.env_file)
    manifest = read_manifest(args.root / "manifest.json")
    verify_files(manifest, args.root)
    bucket = configured("AWS_S3_BUCKET", args.dry_run)
    prefix = configured("OTA_S3_PREFIX", args.dry_run).strip("/")
    if not safe_path(prefix):
        raise ValueError("invalid S3 prefix")
    key_base = prefix + "/releases/" + manifest["release_id"]
    release_base = url_base(configured("AWS_S3_PUBLIC_URL", args.dry_run)) + "/" + key_base
    server = configured("SPACETIMEDB_SERVER", args.dry_run)
    database = configured("SPACETIMEDB_DATABASE", args.dry_run)
    if args.channel not in CHANNELS:
        raise ValueError("invalid channel")
    artifacts = [a for t in manifest["targets"] for a in t["artifacts"]]
    manifest_item = {"path": "manifest.json", "size": (args.root / "manifest.json").stat().st_size,
                     "sha256": digest(args.root / "manifest.json"), "content_type": "application/json"}
    if args.dry_run:
        url_base(server, local=True)
        print(f"Would upload {len(artifacts) + 1} files to s3://{bucket}/{key_base}/")
        print(f"Would register {manifest['release_id']} and promote {args.channel} in {database}")
        return
    for key in ("AWS_S3_ACCESS_KEY_ID", "AWS_S3_SECRET_ACCESS_KEY", "AWS_S3_SESSION_TOKEN"):
        if "CHANGE_ME" in os.environ.get(key, ""):
            raise ValueError(f"replace the dummy {key} before publishing")
    configured("AWS_S3_REGION")
    url_base(configured("AWS_S3_ENDPOINT"), local=True)
    db = Database(server, database, configured("SPACETIMEDB_PUBLISH_TOKEN"))
    generation, _ = db.channel(args.channel)  # Capture before upload; concurrent promotion must conflict.
    for item in artifacts + [manifest_item]:
        print("Uploading", item["path"])
        upload(bucket, key_base + "/" + item["path"], args.root / item["path"], item)
        check_download(release_base, item)
    raw = (args.root / "manifest.json").read_bytes().decode("utf-8")
    db.call("publish_ota_release", [manifest["release_id"], raw, manifest_item["sha256"], release_base])
    db.call("promote_ota_channel", [args.channel, manifest["release_id"], generation])
    _, selected = db.channel(args.channel)
    if selected != manifest["release_id"]:
        raise ValueError("channel changed after promotion; inspect it before retrying")
    print(f"Published {manifest['release_id']} to {args.channel}")


def promote(args):
    load_env(args.env_file)
    if not ID.fullmatch(args.release_id):
        raise ValueError("invalid release ID")
    db = Database(configured("SPACETIMEDB_SERVER"), configured("SPACETIMEDB_DATABASE"),
                  configured("SPACETIMEDB_PUBLISH_TOKEN"))
    generation, _ = db.channel(args.channel)
    db.call("promote_ota_channel", [args.channel, args.release_id, generation])
    _, selected = db.channel(args.channel)
    if selected != args.release_id:
        raise ValueError("channel changed after promotion")
    print(f"Promoted {args.release_id} to {args.channel}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pack = commands.add_parser("package", help="package a tested platform build")
    pack.add_argument("--root", type=Path, required=True)
    pack.add_argument("--out", type=Path, required=True)
    pack.add_argument("--platform", required=True)
    pack.add_argument("--release-id", required=True)
    pack.add_argument("--commit", required=True)
    pack.add_argument("--version", default="")
    pack.set_defaults(run=package)
    combine = commands.add_parser("merge", help="combine platform artifacts into one release")
    combine.add_argument("--root", type=Path, required=True)
    combine.add_argument("--out", type=Path, required=True)
    combine.add_argument("--require-all", action="store_true")
    combine.set_defaults(run=merge)
    send = commands.add_parser("publish", help="upload, verify and promote a release")
    send.add_argument("--root", type=Path, required=True)
    send.add_argument("--channel", choices=sorted(CHANNELS), default="beta")
    send.add_argument("--env-file", type=Path, default=Path(".env"))
    send.add_argument("--dry-run", action="store_true")
    send.set_defaults(run=publish)
    select = commands.add_parser("promote", help="promote or roll back to an existing release")
    select.add_argument("--release-id", required=True)
    select.add_argument("--channel", choices=sorted(CHANNELS), required=True)
    select.add_argument("--env-file", type=Path, default=Path(".env"))
    select.set_defaults(run=promote)
    args = parser.parse_args()
    try:
        args.run(args)
    except (ValueError, OSError, urllib.error.URLError, json.JSONDecodeError) as error:
        print(f"release: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
