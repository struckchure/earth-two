"""Start a persistent local game database and the native companion.

Only loopback hosts are supported. Existing modules/data are never replaced.
Publisher credentials are kept in a private local file, never printed.
"""
import argparse
import json
import os
import pathlib
import signal
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request


def connection_settings():
    settings = {}
    path = pathlib.Path(".env")
    if path.exists():
        for line in path.read_text().splitlines():
            key, separator, value = line.strip().removeprefix("export ").partition("=")
            if separator and key.strip() in {"SPACETIMEDB_SERVER", "SPACETIMEDB_DATABASE"}:
                settings[key.strip()] = value.strip().strip("\"'")
    return (os.environ.get("SPACETIMEDB_SERVER") or settings.get("SPACETIMEDB_SERVER") or "http://127.0.0.1:3001",
            os.environ.get("SPACETIMEDB_DATABASE") or settings.get("SPACETIMEDB_DATABASE") or "earth-two")


def main():
    default_host, default_database = connection_settings()
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default=default_host)
    parser.add_argument("--database", default=default_database)
    parser.add_argument("--data-dir", default=str(pathlib.Path.home() / "earth-two" / "server"))
    parser.add_argument("--wasm", required=True)
    parser.add_argument("--database-only", action="store_true")
    args = parser.parse_args()
    origin = urllib.parse.urlsplit(args.server)
    if origin.scheme != "http" or origin.hostname not in {"localhost", "127.0.0.1", "::1"} or origin.path not in {"", "/"} or origin.query or origin.fragment or origin.username:
        parser.error("make server only manages an HTTP loopback database; deploy remote modules explicitly")
    if not args.database or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in args.database):
        parser.error("local database names must use lowercase letters, digits and hyphens")
    wasm = pathlib.Path(args.wasm).resolve()
    if not wasm.is_file():
        parser.error("module is missing; run make server-module first")
    root = pathlib.Path(args.data_dir).resolve()
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    processes = []

    def request(path, data=None, token=None, method="GET"):
        headers = {"Authorization": "Bearer " + token} if token else {}
        req = urllib.request.Request(args.server.rstrip("/") + path, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as error:
            return error.code, error.read()

    def interrupted(_signal, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    log = (root / "database.log").open("ab")
    try:
        try:
            status, _ = request("/v1/ping")
        except urllib.error.URLError:
            status = None
        if status is None:
            process = subprocess.Popen(["spacetime", "start", "--listen-addr", origin.netloc,
                                        "--data-dir", str(root / "database"), "--non-interactive"], stdout=log, stderr=log)
            processes.append(process)
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError(f"SpacetimeDB exited; see {root / 'database.log'}")
                try:
                    status, _ = request("/v1/ping")
                    if status == 200:
                        break
                except urllib.error.URLError:
                    pass
                time.sleep(0.1)
            else:
                raise RuntimeError("local database did not become ready")
        elif status != 200:
            raise RuntimeError(f"{args.server} belongs to another service; choose a free database port")

        prefix = "/v1/database/" + args.database
        status, _ = request(prefix + "/identity")
        if status == 404:
            owner_path = root / "publisher.json"
            if owner_path.exists():
                owner = json.loads(owner_path.read_text())
            else:
                status, body = request("/v1/identity", b"", method="POST")
                if status != 200:
                    raise RuntimeError("could not create the local module publisher identity")
                owner = json.loads(body)
                with os.fdopen(os.open(owner_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as file:
                    json.dump(owner, file)
            status, body = request(prefix, wasm.read_bytes(), owner["token"], "PUT")
            if not 200 <= status < 300:
                raise RuntimeError("local module publication failed: " + body.decode(errors="replace")[:500])
            print(f"Published {args.database} on {args.server}", flush=True)
        elif status != 200:
            raise RuntimeError(f"database lookup returned {status}")
        else:
            print(f"Using existing {args.database}; its data and module were preserved", flush=True)
        print(f"Game database ready: {args.server} / {args.database}", flush=True)

        if not args.database_only:
            companion = subprocess.Popen(["go", "run", "./cmd/server", "-host", args.server, "-database", args.database])
            processes.append(companion)
        while True:
            for process in processes:
                if process.poll() is not None:
                    raise RuntimeError("a game server process stopped")
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        log.close()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(str(error))
