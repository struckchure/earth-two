import argparse
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.commit = "a" * 40

    def desktop(self, platform="linux-amd64", release_id="test-release", out=None):
        build = self.root / (platform + "-build")
        build.mkdir(exist_ok=True)
        (build / "assets").mkdir(exist_ok=True)
        (build / "assets" / "world.json").write_text('{"world":1}')
        name = "earth-two.exe" if platform.startswith("windows-") else "earth-two"
        (build / name).write_bytes(b"executable")
        args = argparse.Namespace(root=build, out=out or self.root / platform,
                                  platform=platform, release_id=release_id,
                                  commit=self.commit, version="0.1.0")
        release.package(args)
        return args.out, release.read_manifest(args.out / "manifest.json")

    def web(self, out=None):
        build = self.root / "web-build"
        build.mkdir()
        for name in ("index.html", "game.wasm", "wasm_exec.js", "fs.js", "raylib.js",
                     "raylib.wasm", "raylib.data", "jolt.js", "jolt.wasm"):
            (build / name).write_bytes(name.encode())
        (build / "game.wasm.gz").write_bytes(b"sidecar")
        args = argparse.Namespace(root=build, out=out or self.root / "web", platform="web",
                                  release_id="test-release", commit=self.commit, version="0.1.0")
        release.package(args)
        return args.out

    def test_zip_contains_only_game_and_assets_and_preserves_execution_mode(self):
        build = self.root / "linux-amd64-build"
        build.mkdir()
        (build / ".env").write_text("MUST_NOT_SHIP=1")
        (build / "deps").mkdir()
        (build / "deps" / "private.txt").write_text("not game data")
        root, manifest = self.desktop()
        item = manifest["targets"][0]["artifacts"][0]
        with zipfile.ZipFile(root / item["path"]) as archive:
            self.assertEqual(set(archive.namelist()), {"earth-two", "assets/world.json"})
            self.assertEqual((archive.getinfo("earth-two").external_attr >> 16) & 0o777, 0o755)
        self.assertEqual(item["sha256"], release.digest(root / item["path"]))
        second, _ = self.desktop(out=self.root / "repeat")
        self.assertEqual((root / item["path"]).read_bytes(), (second / item["path"]).read_bytes())

    def test_browser_raw_files_and_mime_types(self):
        root = self.web()
        manifest = release.read_manifest(root / "manifest.json")
        items = {a["path"]: a for a in manifest["targets"][0]["artifacts"]}
        self.assertNotIn("web/game.wasm.gz", items)
        self.assertEqual(items["web/game.wasm"]["content_type"], "application/wasm")
        self.assertEqual(items["web/raylib.data"]["content_type"], "application/octet-stream")

    def test_incomplete_browser_build_rejected(self):
        build = self.root / "bad-web"
        build.mkdir()
        (build / "index.html").write_text("html")
        args = argparse.Namespace(root=build, out=self.root / "out", platform="web",
                                  release_id="test", version="1", commit=self.commit)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            release.package(args)
        self.assertFalse(args.out.exists())

    def test_symlink_inputs_rejected(self):
        root, _ = self.desktop()
        link = self.root / "linux-amd64-build" / "assets" / "link"
        try:
            link.symlink_to(self.root / "linux-amd64-build" / "earth-two")
        except OSError:
            self.skipTest("symlink creation unavailable")
        with self.assertRaisesRegex(ValueError, "symlinks"):
            self.desktop(out=self.root / "unsafe")

    def test_tampered_artifact_rejected(self):
        root, manifest = self.desktop()
        (root / manifest["targets"][0]["artifacts"][0]["path"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "mismatch"):
            release.verify_files(manifest, root)

    def test_malformed_paths_platforms_and_checksums_rejected(self):
        _, manifest = self.desktop()
        for bad in ("../escape", "/absolute", "desktop/../escape", "desktop/%2e%2e/escape", "desktop/a\\b"):
            candidate = copy.deepcopy(manifest)
            candidate["targets"][0]["artifacts"][0]["path"] = bad
            with self.subTest(path=bad), self.assertRaises(ValueError):
                release.validate(candidate)
        for field, value in (("size", True), ("sha256", "wrong")):
            candidate = copy.deepcopy(manifest)
            candidate["targets"][0]["artifacts"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                release.validate(candidate)
        manifest["targets"].append(copy.deepcopy(manifest["targets"][0]))
        with self.assertRaises(ValueError):
            release.validate(manifest)

    def test_nonobject_targets_and_artifacts_rejected(self):
        _, original = self.desktop()
        for mutate in (lambda m: m.update(schema_version=True),
                       lambda m: m.update(unknown="field"),
                       lambda m: m.update(targets=[None]),
                       lambda m: m["targets"][0].update(expanded_size=2**64),
                       lambda m: m["targets"][0]["artifacts"][0].update(content_type="application/zip\t"),
                       lambda m: m["targets"][0].update(artifacts=[None])):
            manifest = copy.deepcopy(original)
            mutate(manifest)
            with self.assertRaises(ValueError):
                release.validate(manifest)

    def test_merge_requires_all_platforms_and_matching_commits(self):
        incoming = self.root / "incoming"
        incoming.mkdir()
        self.desktop(out=incoming / "linux-amd64")
        args = argparse.Namespace(root=incoming, out=self.root / "merged", require_all=True)
        with self.assertRaisesRegex(ValueError, "required"):
            release.merge(args)
        self.desktop("darwin-arm64", out=incoming / "darwin-arm64")
        self.desktop("windows-amd64", out=incoming / "windows-amd64")
        self.web(out=incoming / "web")
        release.merge(args)
        self.assertEqual(len(release.read_manifest(args.out / "manifest.json")["targets"]), 4)
        bad = incoming / "web" / "manifest.json"
        data = json.loads(bad.read_text())
        data["source_commit"] = "b" * 40
        bad.write_bytes(release.canonical(data))
        args.out = self.root / "bad-merged"
        with self.assertRaisesRegex(ValueError, "different releases"):
            release.merge(args)

    def environment(self):
        return {"AWS_S3_ENDPOINT":"https://s3.test","AWS_S3_ACCESS_KEY_ID":"synthetic-access-key","AWS_S3_SECRET_ACCESS_KEY":"synthetic-secret-key","AWS_S3_REGION": "us-east-1", "AWS_S3_BUCKET": "test-bucket", "OTA_S3_PREFIX": "earth-two",
                "AWS_S3_PUBLIC_URL": "https://downloads.test", "SPACETIMEDB_SERVER": "https://db.test",
                "SPACETIMEDB_DATABASE": "earth-two", "SPACETIMEDB_PUBLISH_TOKEN": "secret-publisher"}

    def test_promotion_happens_only_after_every_upload_and_verification(self):
        root, manifest = self.desktop()
        events = []

        class DB:
            def __init__(self, *args):
                pass

            def channel(self, channel):
                events.append("read-channel")
                return 7, manifest["release_id"]

            def call(self, name, args):
                events.append(name)
                if name == "promote_ota_channel":
                    self_test.assertEqual(args, ["beta", "test-release", 7])
                else:
                    self_test.assertEqual(hashlib.sha256(args[1].encode()).hexdigest(), args[2])

        self_test = self
        args = argparse.Namespace(root=root, env_file=self.root / "absent", channel="beta", dry_run=False)
        with patch.dict(os.environ, self.environment(), clear=True), patch.object(release, "Database", DB), \
                patch.object(release, "upload", side_effect=lambda *a: events.append("upload")), \
                patch.object(release, "check_download", side_effect=lambda *a: events.append("verify")):
            release.publish(args)
        self.assertEqual(events, ["read-channel", "upload", "verify", "upload", "verify",
                                  "publish_ota_release", "promote_ota_channel", "read-channel"])

    def test_failed_upload_or_download_never_registers_or_promotes(self):
        root, _ = self.desktop()
        args = argparse.Namespace(root=root, env_file=self.root / "absent", channel="beta", dry_run=False)
        for stage in ("upload", "check_download"):
            with self.subTest(stage=stage), patch.dict(os.environ, self.environment(), clear=True), \
                    patch.object(release, "Database") as db, patch.object(release, "upload"), \
                    patch.object(release, "check_download"):
                db.return_value.channel.return_value = (0, "")
                with patch.object(release, stage, side_effect=ValueError("interrupted")):
                    with self.assertRaisesRegex(ValueError, "interrupted"):
                        release.publish(args)
                db.return_value.call.assert_not_called()

    def test_register_failure_never_promotes(self):
        root, _ = self.desktop()
        args = argparse.Namespace(root=root, env_file=self.root / "absent", channel="beta", dry_run=False)
        with patch.dict(os.environ, self.environment(), clear=True), patch.object(release, "Database") as db, \
                patch.object(release, "upload"), patch.object(release, "check_download"):
            db.return_value.channel.return_value = (0, "")
            db.return_value.call.side_effect = ValueError("rejected")
            with self.assertRaisesRegex(ValueError, "rejected"):
                release.publish(args)
            self.assertEqual(db.return_value.call.call_count, 1)
            self.assertEqual(db.return_value.call.call_args.args[0], "publish_ota_release")

    def test_generation_conflict_is_not_automatically_overwritten(self):
        root, _ = self.desktop()
        args = argparse.Namespace(root=root, env_file=self.root / "absent", channel="beta", dry_run=False)
        with patch.dict(os.environ, self.environment(), clear=True), patch.object(release, "Database") as db, \
                patch.object(release, "upload"), patch.object(release, "check_download"):
            db.return_value.channel.return_value = (9, "another-release")
            db.return_value.call.side_effect = [None, ValueError("generation conflict")]
            with self.assertRaisesRegex(ValueError, "generation conflict"):
                release.publish(args)
            self.assertEqual(db.return_value.call.call_count, 2)
            self.assertEqual(db.return_value.channel.call_count, 1)
            self.assertEqual(db.return_value.call.call_args.args[1], ["beta", "test-release", 9])

    def test_exact_manifest_bytes_preserved_when_registering(self):
        root, _ = self.desktop()
        path = root / "manifest.json"
        path.write_bytes(path.read_bytes().replace(b"\n", b"\r\n"))
        args = argparse.Namespace(root=root, env_file=self.root / "absent", channel="beta", dry_run=False)
        with patch.dict(os.environ, self.environment(), clear=True), patch.object(release, "Database") as db, \
                patch.object(release, "upload"), patch.object(release, "check_download"):
            db.return_value.channel.return_value = (1, "test-release")
            release.publish(args)
            arguments = db.return_value.call.call_args_list[0].args[1]
            self.assertEqual(arguments[1].encode(), path.read_bytes())
            self.assertEqual(arguments[2], hashlib.sha256(arguments[1].encode()).hexdigest())

    def test_spacetimedb_http_payload_and_channel_response(self):
        db = release.Database("https://db.test", "earth-two", "publisher-token")
        with patch.object(release.HTTP, "open", return_value=io.BytesIO(b'[{"rows":[[3,"release-three"]]}]')) as http:
            self.assertEqual(db.channel("beta"), (3, "release-three"))
            request = http.call_args.args[0]
            self.assertEqual(request.full_url, "https://db.test/v1/database/earth-two/sql")
            self.assertEqual(request.get_header("Authorization"), "Bearer publisher-token")
            self.assertEqual(request.data, b"SELECT generation, release_id FROM ota_channel WHERE channel = 'beta'")
        with patch.object(release.HTTP, "open", return_value=io.BytesIO(b"")) as http:
            db.call("promote_ota_channel", ["beta", "release-three", 2])
            self.assertEqual(json.loads(http.call_args.args[0].data), ["beta", "release-three", 2])
        with patch.object(release.HTTP, "open") as http, self.assertRaises(ValueError):
            db.channel("beta' OR true")
        http.assert_not_called()

    def test_custom_s3_names_are_mapped_only_into_cli_environment(self):
        result = argparse.Namespace(returncode=0, stdout="{}", stderr="")
        with patch.dict(os.environ, self.environment(), clear=True), patch.object(release.subprocess, "run", return_value=result) as run:
            release.aws("head-bucket", "--bucket", "test-bucket")
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--endpoint-url")+1], "https://s3.test")
            self.assertEqual(command[command.index("--region")+1], "us-east-1")
            child = run.call_args.kwargs["env"]
            self.assertEqual(child["AWS_ACCESS_KEY_ID"], "synthetic-access-key")
            self.assertEqual(child["AWS_SECRET_ACCESS_KEY"], "synthetic-secret-key")
            self.assertNotIn("synthetic-secret-key", command)
            self.assertNotIn("AWS_ACCESS_KEY_ID", os.environ)

    def test_s3_uses_conditional_write_and_verifies_existing_objects(self):
        root, manifest = self.desktop()
        item = manifest["targets"][0]["artifacts"][0]
        checksum = release.base64.b64encode(bytes.fromhex(item["sha256"])).decode()
        head = {"ContentLength": item["size"], "Metadata": {"sha256": item["sha256"]},
                "ChecksumSHA256": checksum, "ContentType": item["content_type"]}
        with patch.object(release, "aws", side_effect=[None, head]) as aws:
            release.upload("bucket", "key", root / item["path"], item)
            self.assertIn("--if-none-match", aws.call_args_list[0].args)
            self.assertIn("--checksum-sha256", aws.call_args_list[0].args)
        head["ChecksumSHA256"] = "different"
        with patch.object(release, "aws", side_effect=[None, head]), self.assertRaisesRegex(ValueError, "verification"):
            release.upload("bucket", "key", root / item["path"], item)

    def test_env_is_not_executed_and_does_not_override_ci(self):
        env = self.root / ".env"
        env.write_text('AWS_S3_REGION=local\nAWS_S3_BUCKET="$(never-run-this)"\n')
        with patch.dict(os.environ, {"AWS_S3_REGION": "ci"}, clear=True):
            release.load_env(env)
            self.assertEqual(os.environ["AWS_S3_REGION"], "ci")
            self.assertEqual(os.environ["AWS_S3_BUCKET"], "$(never-run-this)")

    def test_dry_run_never_uses_network_and_placeholders_block_real_publish(self):
        root, _ = self.desktop()
        env = self.root / ".env"
        env.write_text(Path(".env.example").read_text())
        args = argparse.Namespace(root=root, env_file=env, channel="beta", dry_run=True)
        with patch.dict(os.environ, {}, clear=True), patch.object(release, "aws") as aws, \
                patch.object(release, "Database") as db:
            release.publish(args)
            aws.assert_not_called()
            db.assert_not_called()
            args.dry_run = False
            with self.assertRaisesRegex(ValueError, "configure"):
                release.publish(args)

    def test_credentials_in_urls_and_redirects_rejected(self):
        for url in ("http://db.test", "https://user:secret@db.test", "https://db.test?token=secret"):
            with self.subTest(url=url), self.assertRaises(ValueError):
                release.url_base(url, local=True)
        self.assertEqual(release.url_base("http://127.0.0.1:3000/", local=True), "http://127.0.0.1:3000")
        self.assertIsNone(release.NoRedirect().redirect_request(None, None, 302, "", {}, "https://other.test"))


if __name__ == "__main__":
    unittest.main()
