import importlib.util
import os
from pathlib import Path
import plistlib
import stat
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile

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
        self.assertTrue((app / "Contents/MacOS/earth-two").stat().st_mode & stat.S_IXUSR)

    @unittest.skipIf(os.name == "nt", "macOS symlink layout")
    def test_dmg_keeps_full_version_and_arch_in_name(self):
        output = self.root / "earth-two-1.2.3-macos-arm64"
        with patch.object(package, "run") as run:
            artifacts = package.package_mac(self.binary, self.assets, self.stage, output, "1.2.3", None, None)
        self.assertEqual(artifacts[0].name, "earth-two-1.2.3-macos-arm64.dmg")
        self.assertEqual((self.stage / "Applications").readlink(), Path("/Applications"))
        self.assertIn(("codesign", "--force", "--sign", "-", self.stage / "Earth Two.app"), [call.args for call in run.call_args_list])

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


if __name__ == "__main__":
    unittest.main()
