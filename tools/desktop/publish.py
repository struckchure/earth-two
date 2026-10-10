#!/usr/bin/env python3
"""Upload verified desktop installers to the configured S3 download origin."""
import argparse
import base64
from email import message_from_string
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ota"))
import release


def check_download(base, item):
    # Some S3 public origins reject urllib's transport. curl also checks HTTPS
    # without forwarding credentials or following redirects to another origin.
    result = subprocess.run([
        "curl", "--fail", "--silent", "--show-error", "--head",
        "--max-time", "30", "--proto", "=https",
        "--header", "Accept-Encoding: identity",
        base + "/" + quote(item["path"]),
    ], capture_output=True, text=True, check=False)
    if result.returncode:
        raise ValueError("public installer download is not ready")
    response = result.stdout.strip().split("\n\n")[-1]
    status, _, headers = response.partition("\n")
    headers = message_from_string(headers)
    if (len(status.split()) < 2 or status.split()[1] != "200" or
            int(headers.get("Content-Length", "-1")) != item["size"] or
            headers.get("Content-Encoding")):
        raise ValueError("public installer response does not match the uploaded file")


def installer_files(root):
    files = sorted(p for p in root.rglob("*") if p.is_file())
    if not files:
        raise ValueError("no installers found")
    names = set()
    for path in files:
        if path.name in names:
            raise ValueError(f"duplicate installer filename: {path.name}")
        names.add(path.name)
        if not re.fullmatch(r"earth-two-\d+\.\d+\.\d+-(?:macos|windows|linux)-(?:arm64|amd64)(?:-setup\.exe|\.dmg|\.deb|\.tar\.gz|\.zip)(?:\.sha256)?", path.name):
            raise ValueError(f"unexpected installer file: {path.name}")
        if path.suffix == ".sha256":
            continue
        checksum = path.with_name(path.name + ".sha256")
        if not checksum.is_file():
            raise ValueError(f"missing checksum: {path.name}")
        parts = checksum.read_text().split()
        if len(parts) != 2 or parts[0] != release.digest(path) or parts[1].lstrip("*") != path.name:
            raise ValueError(f"checksum mismatch: {path.name}")
    for path in files:
        if path.suffix == ".sha256" and not path.with_suffix("").is_file():
            raise ValueError(f"orphaned checksum: {path.name}")
    return files


def require_landing_installers(files):
    """The latest pointer must never publish a partial or mixed-version set."""
    required = {"windows-amd64-setup.exe", "macos-arm64.dmg", "macos-amd64.dmg", "linux-amd64.deb"}
    versions = set()
    found = set()
    for path in files:
        match = re.fullmatch(r"earth-two-(\d+\.\d+\.\d+)-(.+)", path.name)
        if match and match[2] in required:
            versions.add(match[1])
            found.add(match[2])
    if found != required or len(versions) != 1:
        raise ValueError("latest installers require all four landing-page targets at one version")


def promote_latest(bucket, prefix, manifest):
    # This is the only mutable object; release installers stay immutable.
    item = release.artifact(manifest.parent, manifest.name)
    checksum = base64.b64encode(bytes.fromhex(item["sha256"])).decode()
    key = f"{prefix}/installers/latest.json"
    release.aws("put-object", "--bucket", bucket, "--key", key, "--body", str(manifest),
                "--content-type", "application/json", "--cache-control", "public,max-age=60",
                "--metadata", "sha256=" + item["sha256"], "--checksum-sha256", checksum)
    remote = release.aws("head-object", "--bucket", bucket, "--key", key, "--checksum-mode", "ENABLED")
    if (not remote or remote.get("ContentLength") != item["size"] or
            remote.get("ChecksumSHA256") != checksum or remote.get("ContentType") != "application/json" or
            remote.get("CacheControl") != "public,max-age=60" or remote.get("ContentEncoding")):
        raise ValueError("latest installer manifest verification failed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--release-id", required=True)
    parser.add_argument("--env-file", type=Path, default=Path(".env"))
    parser.add_argument("--out", type=Path, required=True, help="write verified download URLs as JSON")
    parser.add_argument("--promote-latest", action="store_true", help="update the landing-page manifest after verification")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", args.release_id):
        parser.error("invalid release ID")
    files = installer_files(args.root)
    if args.promote_latest:
        require_landing_installers(files)
    release.load_env(args.env_file)
    prefix = release.configured("OTA_S3_PREFIX").strip("/")
    if not release.safe_path(prefix):
        raise ValueError("invalid S3 prefix")
    key_base = f"{prefix}/releases/{args.release_id}/installers"
    public_base = release.url_base(release.configured("AWS_S3_PUBLIC_URL")) + "/" + key_base
    bucket = release.configured("AWS_S3_BUCKET")
    downloads = []
    # Verify the storage path with tiny checksum files before transferring binaries.
    for path in sorted(files, key=lambda p: (p.suffix != ".sha256", p.name)):
        item = release.artifact(path.parent, path.name)
        # A compressed archive is a downloadable file, not HTTP compression.
        if path.name.endswith(".tar.gz"):
            item["content_type"] = "application/gzip"
        elif path.suffix == ".sha256":
            item["content_type"] = "text/plain"
        print(f"Uploading {path.name} ({item['size']} bytes)", flush=True)
        release.upload(bucket, key_base + "/" + path.name, path, item)
        check_download(public_base, item)
        downloads.append({**item, "url": public_base + "/" + quote(path.name)})
        print(f"Verified {path.name}", flush=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps({"release_id": args.release_id, "downloads": downloads}, indent=2) + "\n")
    if args.promote_latest:
        promote_latest(bucket, prefix, args.out)
    print(f"Download URLs: {args.out}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError) as error:
        sys.exit(str(error))
