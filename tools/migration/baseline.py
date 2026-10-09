#!/usr/bin/env python3
"""Capture evidence from an isolated, immutable Go reference checkout."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tarfile
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = "c561bd69e06500aa3c79e9bb0d0d5e974694e7c7"


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def summarize_tests(path):
    """Keep package failures (including compilation), test failures and skips separate."""
    tests, packages = {}, {}
    malformed = 0
    for line in path.read_text().splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            malformed += 1
            continue
        action = event.get("Action")
        if action not in ("pass", "fail", "skip"):
            continue
        package, test = event.get("Package", ""), event.get("Test")
        if test:
            tests[(package, test)] = action
        else:
            packages[package] = action
    return {
        "test_events": {a: sum(v == a for v in tests.values()) for a in ("pass", "fail", "skip")},
        "package_events": {a: sum(v == a for v in packages.values()) for a in ("pass", "fail", "skip")},
        "failed_tests": [f"{p}/{t}" for (p, t), a in sorted(tests.items()) if a == "fail"],
        "skipped_tests": [f"{p}/{t}" for (p, t), a in sorted(tests.items()) if a == "skip"],
        "failed_packages": [p for p, a in sorted(packages.items()) if a == "fail"],
        "non_json_lines": malformed,
        "count_note": "Go reports parent tests and subtests separately; these are event counts.",
    }


def inventory(source):
    files = []
    for path in sorted(source.rglob("*.go")):
        relative = path.relative_to(source).as_posix()
        if relative.startswith("build/"):
            continue
        text = path.read_text()
        files.append({
            "path": relative,
            "build_tags": re.findall(r"^//go:build (.+)$", text, re.M),
            "tests": re.findall(r"^func (Test\w+)\(", text, re.M),
            "benchmarks": re.findall(r"^func (Benchmark\w+)\(", text, re.M),
            "fuzz_tests": re.findall(r"^func (Fuzz\w+)\(", text, re.M),
            "environment": sorted(set(re.findall(r'os\.(?:Getenv|LookupEnv)\("([^"\n]+)"', text))),
        })
    return files


def run_process(command, *, timeout, **kwargs):
    """Bound native compiler/game children as well as the launcher on POSIX."""
    with subprocess.Popen(command, start_new_session=os.name == "posix", **kwargs) as process:
        try:
            return process.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            elif os.name == "nt":
                subprocess.run(["taskkill", "/F", "/T", "/PID", str(process.pid)],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            else:
                process.kill()
            process.wait()
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", default=REFERENCE)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--capture", action="store_true", help="open native tour window; capture day, night and storm")
    parser.add_argument("--web", action="store_true", help="build original browser release (does not prove runtime support)")
    args = parser.parse_args()
    if args.prepare_only and (args.capture or args.web):
        parser.error("--prepare-only cannot be combined with --capture or --web")
    # Resolve a commit before interpreting any output location or running builds.
    revision = subprocess.check_output(
        ["git", "rev-parse", "--verify", "--end-of-options", args.reference + "^{commit}"],
        cwd=ROOT, text=True,
    ).strip()
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = (args.output or ROOT / "build/migration-baseline" / f"go-{revision[:7]}-{stamp}").resolve()
    # Reusing evidence directories could mix captures from different source trees.
    if out.exists():
        parser.error(f"output already exists; choose a fresh directory: {out}")
    out.mkdir(parents=True)
    source = out / "reference"
    source.mkdir()
    archive = out / "reference.tar"
    subprocess.run(["git", "archive", "--format=tar", "--output", str(archive), revision], cwd=ROOT, check=True)
    with tarfile.open(archive) as bundle:
        members = bundle.getmembers()
        for member in members:
            target = (source / member.name).resolve()
            if not target.is_relative_to(source) or not (member.isdir() or member.isfile()):
                raise ValueError(f"unsafe archive member: {member.name}")
        # Only validated regular files/directories, also on Python 3.9.
        bundle.extractall(source, members=members)
    metadata = {
        "reference_commit": revision,
        "reference_tree": subprocess.check_output(["git", "rev-parse", revision + "^{tree}"], cwd=ROOT, text=True).strip(),
        "source_archive_sha256": digest(archive),
        "runner_sha256": digest(Path(__file__)),
        "started_utc": stamp,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "commands": [],
        "scope": "Local Go reference; no Rust parity, remote platform or browser runtime claim.",
    }
    asset_hashes = {p.relative_to(source).as_posix(): digest(p) for p in sorted((source / "assets").rglob("*")) if p.is_file()}
    write_json(out / "assets.json", asset_hashes)
    metadata["asset_manifest_sha256"] = digest(out / "assets.json")
    metadata["lockfiles"] = {name: digest(source / name) for name in ("go.mod", "go.sum")}
    write_json(out / "inventory.json", inventory(source))
    write_json(out / "run.json", metadata)
    print(f"Reference evidence: {out}", flush=True)
    if args.prepare_only:
        metadata.update(status="prepared", finished_utc=datetime.now(timezone.utc).isoformat())
        write_json(out / "run.json", metadata)
        return 0
    env = {k: v for k, v in os.environ.items() if not k.startswith("EARTH_TWO_")}
    env.update(GOWORK="off", GOFLAGS="", EARTH_TWO_IDENTITY_PATH=str(out / "disposable-identity/pk"))

    def run(name, command, extra=None, timeout=1800):
        started = time.monotonic()
        entry = {"name": name, "argv": list(map(str, command)), "cwd": str(source)}
        print(f"Running {name}", flush=True)
        with (out / f"{name}.stdout.log").open("w") as stdout, (out / f"{name}.stderr.log").open("w") as stderr:
            try:
                entry["exit_code"] = run_process(command, cwd=source, env=env | (extra or {}), stdout=stdout, stderr=stderr, timeout=timeout)
            except subprocess.TimeoutExpired:
                entry.update(exit_code=124, error="timeout")
            except OSError as error:
                entry.update(exit_code=127, error=str(error))
        entry["elapsed_seconds"] = round(time.monotonic() - started, 3)
        metadata["commands"].append(entry)
        write_json(out / "run.json", metadata)
        print(f"{name}: exit {entry['exit_code']} ({entry['elapsed_seconds']}s)", flush=True)
        return entry["exit_code"] == 0

    run("go-version", ["go", "version"])
    run("go-environment", ["go", "env", "-json", "GOOS", "GOARCH", "GOVERSION", "CGO_ENABLED", "CC", "CXX"])
    if not run("deps", ["go", "run", "./tools/deps"]):
        metadata.update(status="failed", finished_utc=datetime.now(timezone.utc).isoformat())
        write_json(out / "run.json", metadata)
        return 1
    work = source / "build/deps/native.work"
    env["GOWORK"] = str(work)
    metadata["prepared_workspace"] = work.read_text()
    run("tests", ["go", "test", "-json", "-count=1", "./..."])
    write_json(out / "test-summary.json", summarize_tests(out / "tests.stdout.log"))
    run("benchmarks", ["go", "test", "./game", "-run", "^$", "-bench", "BenchmarkMinimapSelection|BenchmarkTerrainTileMask|BenchmarkTerrainSampling|BenchmarkViewSees", "-benchmem", "-count=5"])
    run("assets", ["go", "run", "./tools/assetpack"])
    run("packed-assets", ["go", "test", "./tools/assetpack", "-count=1", "-run", "TestPackedRepository"], {"EARTH_TWO_PACK_CHECK": str(source / "build/assets")})
    run("desktop", ["go", "build", "-o", str(out / ("earth-two-reference.exe" if os.name == "nt" else "earth-two-reference")), "./cmd/desktop"])
    if args.capture:
        # Separate process keeps capture provenance, completeness and timeouts explicit.
        run("captures", [sys.executable, str(ROOT / "tools/migration/capture.py"), str(out)], timeout=1200)
    if args.web:
        # Use the reference Makefile, including its SDK adapter and compression.
        run("web", ["make", "web"], timeout=2400)
    metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
    metadata["status"] = "passed" if all(c["exit_code"] == 0 for c in metadata["commands"]) else "failed"
    write_json(out / "run.json", metadata)
    return 0 if metadata["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
