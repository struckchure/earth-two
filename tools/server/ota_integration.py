"""Exercise OTA's real HTTP API on a new disposable local database.

No S3 writes or existing databases are touched. Generated Bearer credentials
remain in memory and are never logged. Requires a running local SpacetimeDB.
"""
import argparse
import hashlib
import json
import pathlib
import urllib.error
import urllib.parse
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="http://127.0.0.1:3001")
    parser.add_argument("--wasm", required=True)
    args = parser.parse_args()
    origin = urllib.parse.urlsplit(args.host)
    if origin.hostname not in {"localhost", "127.0.0.1", "::1"}:
        parser.error("integration tests require a disposable local database host")
    database = "earth-two-ota-test-" + uuid.uuid4().hex[:12]

    def request(path, data=None, token=None, method="GET", raw=False):
        headers = {}
        if token:
            headers["Authorization"] = "Bearer " + token
        if data is not None and not raw:
            data = json.dumps(data, separators=(",", ":")).encode()
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(args.host.rstrip("/") + path, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                return response.status, response.read().decode()
        except urllib.error.HTTPError as error:
            return error.code, error.read().decode()

    def identity():
        status, body = request("/v1/identity", b"", method="POST", raw=True)
        assert status == 200, (status, body)
        return json.loads(body)

    owner, publisher, player = identity(), identity(), identity()
    prefix = "/v1/database/" + database
    status, body = request(prefix, pathlib.Path(args.wasm).read_bytes(), owner["token"], "PUT", raw=True)
    assert 200 <= status < 300, (status, body)

    def call(name, data, token, ok=True):
        status, body = request(prefix + "/call/" + name, data, token, "POST")
        assert (200 <= status < 300) == ok, (name, status, body)

    def sql(query):
        status, body = request(prefix + "/sql", query.encode(), method="POST", raw=True)
        assert status == 200, (status, body)
        return json.loads(body)[0]["rows"]

    def manifest(release):
        return {"schema_version": 1, "release_id": release, "game_version": "0.1.0", "source_commit": "a" * 40,
                "targets": [{"platform": "web", "entrypoint": "web/index.html", "expanded_size": 1,
                             "artifacts": [{"path": "web/index.html", "size": 1, "sha256": "b" * 64, "content_type": "text/html"}]}]}

    def publish(release, value=None, token=None, ok=True, base=None, digest=None):
        body = json.dumps(manifest(release) if value is None else value, separators=(",", ":"))
        sha = digest or hashlib.sha256(body.encode()).hexdigest()
        call("publish_ota_release", [release, body, sha, base or "https://downloads.example.com/earth-two/releases/" + release], token, ok)

    # Neither anonymous callers nor signed-in ordinary players can publish,
    # promote, or claim the publisher role, including before the first release.
    for token in [None, player["token"]]:
        publish("release-a", token=token, ok=False)
        call("promote_ota_channel", ["stable", "release-a", 0], token, False)
        call("authorize_ota_publisher", [player["identity"], True], token, False)
    assert sql("SELECT release_id FROM ota_release") == []

    call("authorize_ota_publisher", [publisher["identity"], True], owner["token"])
    publish("release-a", token=publisher["token"])
    publish("release-a", token=publisher["token"])  # identical retry
    assert len(sql("SELECT release_id FROM ota_release")) == 1
    assert sql("SELECT channel FROM ota_channel") == []  # publishing never promotes
    altered = manifest("release-a"); altered["game_version"] = "changed"
    publish("release-a", altered, publisher["token"], False)
    publish("release-a", token=publisher["token"], ok=False, base="https://other.example.com/releases/release-a")
    publish("release-b", token=publisher["token"])

    call("promote_ota_channel", ["stable", "release-a", 0], publisher["token"])
    call("promote_ota_channel", ["stable", "release-a", 1], publisher["token"])  # current target no-op
    assert sql("SELECT generation, previous_release_id FROM ota_channel WHERE channel = 'stable'")[0] == [1, ""]
    call("promote_ota_channel", ["stable", "release-b", 0], publisher["token"], False)
    call("promote_ota_channel", ["stable", "missing", 1], publisher["token"], False)
    call("promote_ota_channel", ["unknown", "release-b", 0], publisher["token"], False)
    call("promote_ota_channel", ["stable", "release-b", 1], publisher["token"])
    assert sql("SELECT generation, previous_release_id FROM ota_channel WHERE channel = 'stable'")[0] == [2, "release-a"]
    call("promote_ota_channel", ["stable", "release-a", 2], publisher["token"])  # rollback is another promotion
    assert sql("SELECT generation, previous_release_id FROM ota_channel WHERE channel = 'stable'")[0] == [3, "release-b"]

    for path in ["web/../index.html", "web//index.html", "web/%2e%2e/index.html", "web\\index.html", "web/index.html?token=x"]:
        bad = manifest("invalid"); bad["targets"][0]["artifacts"][0]["path"] = path
        publish("invalid", bad, publisher["token"], False)
    for value in [True, -1, 0.5]:
        bad = manifest("invalid"); bad["targets"][0]["artifacts"][0]["size"] = value
        publish("invalid", bad, publisher["token"], False)
    for base in ["http://downloads.example.com/releases/invalid", "https://user:password@downloads.example.com/releases/invalid",
                 "https://downloads.example.com/releases/invalid?token=x", "https://downloads.example.com/releases/wrong"]:
        publish("invalid", token=publisher["token"], ok=False, base=base)
    publish("invalid", token=publisher["token"], ok=False, digest="0" * 64)
    huge = json.dumps(manifest("oversized")) + " " * (1024 * 1024)
    call("publish_ota_release", ["oversized", huge, hashlib.sha256(huge.encode()).hexdigest(), "https://downloads.example.com/releases/oversized"], publisher["token"], False)
    assert len(sql("SELECT release_id FROM ota_release")) == 2
    call("authorize_ota_publisher", [publisher["identity"], False], owner["token"])
    publish("revoked", token=publisher["token"], ok=False)
    print("PASS: authenticated OTA HTTP reducers, immutable retries, channel CAS, rollback, validation, and publisher revocation")
    print("Disposable local database:", database)


if __name__ == "__main__":
    main()
