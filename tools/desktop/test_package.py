import importlib.util
import os
from pathlib import Path
import plistlib
import stat
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import tarfile
import shutil

spec = importlib.util.spec_from_file_location("desktop_package", Path(__file__).with_name("package.py"))
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "earth-two"
        self.binary.write_bytes(b"binary")
        self.assets = self.root / "assets"
        (self.assets / "characters").mkdir(parents=True)
        (self.assets / ".assetpack").write_text("packed")
        (self.assets / "characters/CREDITS.txt").write_text("credits")
        self.stage = self.root / "stage"
        self.stage.mkdir()

    def test_wrong_platform_and_arch_are_rejected(self):
        header = bytearray(64)
        header[:4] = b"\xcf\xfa\xed\xfe"
        struct.pack_into("<I", header, 4, 0x0100000C)
        self.binary.write_bytes(header)
        package.validate_binary(self.binary, "macos", "arm64")
        for target, arch in [("macos", "amd64"), ("linux", "arm64"), ("windows", "amd64")]:
            with self.assertRaises(ValueError):
                package.validate_binary(self.binary, target, arch)

    def test_mac_bundle_has_resources_and_executable(self):
        app = package.mac_app(self.binary, self.assets, self.stage, "1.2.3")
        with (app / "Contents/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        self.assertEqual(info["CFBundleExecutable"], "earth-two")
        self.assertEqual(info["CFBundleShortVersionString"], "1.2.3")
        self.assertEqual((app / "Contents/Resources/assets/characters/CREDITS.txt").read_text(), "credits")
        executable = app / "Contents/MacOS/earth-two"
        self.assertEqual(executable.read_bytes(), self.binary.read_bytes())
        # Windows does not expose POSIX execute bits for this extensionless
        # macOS binary; check its mode only on hosts that support those bits.
        if os.name != "nt":
            self.assertTrue(executable.stat().st_mode & stat.S_IXUSR)

    @unittest.skipIf(os.name == "nt", "macOS symlink layout")
    def test_dmg_keeps_full_version_and_arch_in_name(self):
        output = self.root / "earth-two-1.2.3-macos-arm64"
        with patch.object(package, "run") as run, patch.object(package, "create_dmg") as create:
            artifacts = package.package_mac(self.binary, self.assets, self.stage, output, "1.2.3", None, None)
        self.assertEqual(artifacts[0].name, "earth-two-1.2.3-macos-arm64.dmg")
        create.assert_called_once_with(self.stage, artifacts[0])
        self.assertEqual((self.stage / "Applications").readlink(), Path("/Applications"))
        self.assertIn(("codesign", "--force", "--sign", "-", self.stage / "Earth Two.app"), [call.args for call in run.call_args_list])

    def test_dmg_retries_busy_service_and_uses_hfs(self):
        busy = subprocess.CompletedProcess([], 1, "", "hdiutil: create failed - Resource busy\n")
        success = subprocess.CompletedProcess([], 0, "created\n", "")
        with patch.object(package.subprocess, "run", side_effect=[busy, success]) as run, \
                patch.object(package.time, "sleep") as sleep:
            package.create_dmg(self.stage, self.root / "game.dmg")
        self.assertEqual(run.call_count, 2)
        sleep.assert_called_once_with(2)
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("-fs") + 1], "HFS+")

    def test_dmg_errors_are_not_hidden_or_retried_forever(self):
        for message, attempts in [("Resource busy", 3), ("Permission denied", 1)]:
            with self.subTest(message=message):
                failure = subprocess.CompletedProcess([], 1, "", message)
                with patch.object(package.subprocess, "run", return_value=failure) as run, \
                        patch.object(package.time, "sleep"), \
                        self.assertRaises(subprocess.CalledProcessError):
                    package.create_dmg(self.stage, self.root / "game.dmg")
                self.assertEqual(run.call_count, attempts)

    def test_windows_zip_contains_only_runtime_and_credits(self):
        output = self.root / "earth-two-1.2.3-windows-amd64"
        with patch.object(package, "run"):
            artifacts = package.package_windows(self.binary, self.assets, self.stage, output, "1.2.3")
        with zipfile.ZipFile(artifacts[1]) as file:
            self.assertEqual(set(file.namelist()), {"Earth Two/earth-two.exe", "Earth Two/assets/.assetpack", "Earth Two/assets/characters/CREDITS.txt"})

    @unittest.skipIf(os.name == "nt", "Linux symlink layout")
    def test_deb_uses_opt_and_a_desktop_launcher(self):
        root = package.linux_payload(self.binary, self.assets, self.stage, "1.2.3", "amd64", "libc6 (>= 2.39), libgl1")
        self.assertEqual((root / "usr/bin/earth-two").readlink(), Path("/opt/earth-two/earth-two"))
        self.assertTrue((root / "opt/earth-two/assets/characters/CREDITS.txt").is_file())
        self.assertIn("Exec=/opt/earth-two/earth-two", (root / "usr/share/applications/earth-two.desktop").read_text())
        self.assertIn("Depends: libc6 (>= 2.39), libgl1", (root / "DEBIAN/control").read_text())

    def restrictive_assets(self):
        (self.assets / "world").mkdir(mode=0o700)
        manifest = self.assets / "world/world.json"
        manifest.write_text('{"pieces":{}}')
        for path in [self.assets, *self.assets.rglob("*")]:
            path.chmod(0o700 if path.is_dir() else 0o600)
        return manifest

    def assert_runtime_modes(self, root, executable):
        for path in [root, *root.rglob("*")]:
            if path.is_symlink():
                continue
            expected = 0o755 if path.is_dir() or path == executable else 0o644
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), expected, str(path))

    @unittest.skipIf(os.name == "nt", "POSIX installation permissions")
    def test_deb_assets_are_readable_after_root_install_with_restrictive_umask(self):
        source = self.restrictive_assets()
        old = os.umask(0o077)
        try:
            root = package.linux_payload(self.binary, self.assets, self.stage, "1.2.3", "amd64", "libc6")
        finally:
            os.umask(old)
        self.assert_runtime_modes(root, root / "opt/earth-two/earth-two")
        self.assertEqual(stat.S_IMODE(source.stat().st_mode), 0o600)
        self.assertEqual((root / "usr/bin/earth-two").readlink(), Path("/opt/earth-two/earth-two"))
        # On Linux CI, inspect the actual root-owned archive, not just staging.
        if shutil.which("dpkg-deb"):
            deb = self.root / "test.deb"
            subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(root), str(deb)], check=True, capture_output=True)
            data = subprocess.check_output(["dpkg-deb", "--fsys-tarfile", str(deb)])
            import io
            with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                for member in archive.getmembers():
                    if member.issym():
                        continue
                    self.assertEqual(member.uid, 0)
                    self.assertEqual(member.gid, 0)
                    expected = 0o755 if member.isdir() or member.name == "./opt/earth-two/earth-two" else 0o644
                    self.assertEqual(member.mode, expected, member.name)

    @unittest.skipIf(os.name == "nt", "POSIX archive permissions")
    def test_portable_and_mac_assets_do_not_keep_private_build_permissions(self):
        self.restrictive_assets()
        payload = self.stage / "portable"
        package.copy_runtime(self.binary, self.assets, payload)
        self.assert_runtime_modes(payload, payload / "earth-two")
        archive = self.root / "game.tar.gz"
        with tarfile.open(archive, "w:gz") as file:
            file.add(payload, arcname="earth-two")
        with tarfile.open(archive) as file:
            self.assertEqual(file.getmember("earth-two/assets/world/world.json").mode, 0o644)
            self.assertEqual(file.getmember("earth-two/assets/world").mode, 0o755)
        app = package.mac_app(self.binary, self.assets, self.stage, "1.2.3")
        self.assert_runtime_modes(app, app / "Contents/MacOS/earth-two")


if __name__ == "__main__":
    unittest.main()
