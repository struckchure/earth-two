import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import subprocess

from publish import check_download, installer_files


class InstallerUploadInputsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.file = self.root / "earth-two-0.1.0-linux-amd64.deb"
        self.file.write_bytes(b"installer bytes")
        self.checksum = self.file.with_name(self.file.name + ".sha256")
        self.checksum.write_text(hashlib.sha256(self.file.read_bytes()).hexdigest() + "  " + self.file.name + "\n")

    def test_includes_installer_and_verified_checksum(self):
        self.assertEqual(installer_files(self.root), [self.file, self.checksum])

    def test_rejects_corrupted_installer(self):
        self.file.write_bytes(b"corrupt download")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            installer_files(self.root)

    def test_rejects_unrelated_files_before_upload(self):
        (self.root / ".env").write_text("not an installer")
        with self.assertRaisesRegex(ValueError, "unexpected installer file"):
            installer_files(self.root)

    def test_rejects_orphaned_checksum(self):
        self.file.unlink()
        with self.assertRaisesRegex(ValueError, "orphaned checksum"):
            installer_files(self.root)


class PublicDownloadTest(unittest.TestCase):
    @patch("publish.subprocess.run")
    def test_checks_final_origin_response(self, run):
        run.return_value = subprocess.CompletedProcess([], 0, "HTTP/1.1 200 Connection established\n\nHTTP/2 200 OK\nContent-Length: 12\n\n", "")
        check_download("https://downloads.example.org", {"path": "game.dmg", "size": 12})

    @patch("publish.subprocess.run")
    def test_rejects_redirects_wrong_sizes_and_compression(self, run):
        for headers in [
            "HTTP/2 302 Found\nContent-Length: 12\n",
            "HTTP/2 200 OK\nContent-Length: 2\n",
            "HTTP/2 200 OK\nContent-Length: 12\nContent-Encoding: gzip\n",
        ]:
            with self.subTest(headers=headers):
                run.return_value = subprocess.CompletedProcess([], 0, headers, "")
                with self.assertRaises(ValueError):
                    check_download("https://downloads.example.org", {"path": "game.dmg", "size": 12})

    @patch("publish.subprocess.run")
    def test_rejects_failed_transfer(self, run):
        run.return_value = subprocess.CompletedProcess([], 22, "", "HTTP error")
        with self.assertRaises(ValueError):
            check_download("https://downloads.example.org", {"path": "game.dmg", "size": 12})

class LatestInstallerTest(unittest.TestCase):
    def full_set(self, root):
        for suffix in ["windows-amd64-setup.exe", "macos-arm64.dmg", "macos-amd64.dmg", "linux-amd64.deb"]:
            path = root / ("earth-two-0.1.0-" + suffix)
            path.write_bytes(b"installer")
            path.with_name(path.name + ".sha256").write_text(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name)

    def test_latest_requires_complete_matching_platforms(self):
        from publish import require_landing_installers
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.full_set(root)
            files = installer_files(root)
            require_landing_installers(files)
            with self.assertRaises(ValueError):
                require_landing_installers([f for f in files if not f.name.endswith(".deb")])
            mixed = [Path(str(f).replace("0.1.0", "0.2.0")) if f.name.endswith(".deb") else f for f in files]
            with self.assertRaises(ValueError):
                require_landing_installers(mixed)

    def test_latest_promoted_only_after_all_public_downloads_verified(self):
        from publish import main
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "inputs"
            root.mkdir()
            self.full_set(root)
            out = Path(tmp) / "downloads.json"
            config = {"OTA_S3_PREFIX":"earth-two", "AWS_S3_PUBLIC_URL":"https://downloads.example.org", "AWS_S3_BUCKET":"bucket"}
            args = ["publish.py", "--root", str(root), "--out", str(out), "--release-id", "test", "--promote-latest"]
            for fails in [False, True]:
                with self.subTest(fails=fails), patch("sys.argv", args), patch("publish.release.load_env"), patch("publish.release.configured", side_effect=config.__getitem__), patch("publish.release.upload"), patch("publish.check_download") as verify, patch("publish.promote_latest") as promote:
                    if fails:
                        verify.side_effect = ValueError("download not ready")
                        with self.assertRaises(ValueError):
                            main()
                        promote.assert_not_called()
                    else:
                        def check_promotion(*_):
                            self.assertEqual(verify.call_count, 8)
                            self.assertTrue(out.is_file())
                        promote.side_effect = check_promotion
                        main()
                        promote.assert_called_once_with("bucket", "earth-two", out)

    @patch("publish.release.aws")
    def test_latest_is_short_cached_and_outside_immutable_releases(self, aws):
        import base64
        from publish import promote_latest
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "downloads.json"
            path.write_bytes(b'{}\n')
            aws.side_effect = [{}, {"ContentLength":3, "ChecksumSHA256":base64.b64encode(hashlib.sha256(path.read_bytes()).digest()).decode(), "ContentType":"application/json", "CacheControl":"public,max-age=60"}]
            promote_latest("bucket","earth-two",path)
            args = aws.call_args_list[0].args
            self.assertIn("earth-two/installers/latest.json",args)
            self.assertIn("public,max-age=60",args)
            self.assertNotIn("--if-none-match",args)


if __name__ == "__main__":
    unittest.main()
