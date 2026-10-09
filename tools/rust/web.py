#!/usr/bin/env python3
"""Build the Rust browser candidate with the existing packed asset directory."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, default=ROOT / "build/assets")
    args = parser.parse_args()
    if not (args.assets / "world/world.json").is_file():
        parser.error("packed assets are missing; run make assets first")

    with (ROOT / "Cargo.lock").open("rb") as stream:
        packages = tomllib.load(stream)["package"]
    versions = {p["version"] for p in packages if p["name"] == "wasm-bindgen"}
    if len(versions) != 1:
        parser.error("expected exactly one wasm-bindgen version in Cargo.lock")
    version = versions.pop()
    executable = shutil.which("wasm-bindgen")
    local = ROOT / "build/tools/bin/wasm-bindgen"
    if executable is None and local.is_file():
        executable = str(local)
    install = f"cargo install wasm-bindgen-cli --version {version} --locked --root build/tools"
    if executable is None:
        parser.error(f"wasm-bindgen is required; install it with: {install}")
    actual = subprocess.check_output([executable, "--version"], text=True).strip()
    if actual != f"wasm-bindgen {version}":
        parser.error(f"{actual} does not match Cargo.lock; run: {install}")

    subprocess.run([
        "cargo", "build", "--locked", "--release", "-p", "earth-two-client",
        "--target", "wasm32-unknown-unknown", "--target-dir", str(ROOT / "build/rust"),
    ], cwd=ROOT, check=True)
    subprocess.run(["npm", "ci", "--prefix", "web/spacetime"], cwd=ROOT, check=True)
    subprocess.run(["npm", "run", "build", "--prefix", "web/spacetime"], cwd=ROOT, check=True)
    output = ROOT / "build/rust-web"
    # Generate in a fresh directory so failed builds leave the last candidate intact.
    with tempfile.TemporaryDirectory(prefix="rust-web-", dir=ROOT / "build") as temp:
        staged = Path(temp) / "site"
        staged.mkdir()
        subprocess.run([
            executable,
            str(ROOT / "build/rust/wasm32-unknown-unknown/release/earth-two-client.wasm"),
            "--target", "web", "--out-dir", str(staged), "--out-name", "earth_two_client",
        ], check=True)
        shutil.copy2(ROOT / "apps/client/web/index.html", staged / "index.html")
        shutil.copy2(ROOT / "build/spacetime/account.js", staged / "account.js")
        shutil.copytree(args.assets, staged / "assets")
        previous = Path(temp) / "previous"
        if output.exists():
            output.rename(previous)
        try:
            staged.rename(output)
        except OSError:
            if previous.exists():
                previous.rename(output)
            raise
    print(f"Browser candidate: {output}")
    print("Serve with: python3 -m http.server 8080 --directory build/rust-web")


if __name__ == "__main__":
    main()
