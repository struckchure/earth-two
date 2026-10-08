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


if __name__ == "__main__":
    unittest.main()
