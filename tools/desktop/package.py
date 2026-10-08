#!/usr/bin/env python3
"""Package a native Earth Two binary and the assetpack output. No Python deps."""

import argparse
import hashlib
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile


HERE = Path(__file__).resolve().parent
PLATFORMS = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}
ARCHES = {"arm64": "arm64", "aarch64": "arm64", "x86_64": "amd64", "AMD64": "amd64"}


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def validate_binary(binary, target, arch):
    """Reject stale/wrong-platform binaries before assigning a release name."""
    with binary.open("rb") as file:
        header = file.read(64)
        if len(header) < 64:
            raise ValueError(f"{binary} is not a supported native executable")
        actual = None
        if header[:4] == b"\xcf\xfa\xed\xfe":
            machine = struct.unpack_from("<I", header, 4)[0]
            actual = ("macos", {0x01000007: "amd64", 0x0100000C: "arm64"}.get(machine))
        elif header[:2] == b"MZ":
            file.seek(struct.unpack_from("<I", header, 60)[0])
            pe = file.read(6)
            if len(pe) == 6 and pe[:4] == b"PE\0\0":
                actual = ("windows", {0x8664: "amd64", 0xAA64: "arm64"}.get(struct.unpack_from("<H", pe, 4)[0]))
        elif header[:4] == b"\x7fELF" and header[4:6] == b"\x02\x01":
            actual = ("linux", {62: "amd64", 183: "arm64"}.get(struct.unpack_from("<H", header, 18)[0]))
    if actual != (target, arch):
        raise ValueError(f"{binary} is {actual}, expected {(target, arch)}; build for the target first")


def copy_runtime(binary, assets, destination, executable="earth-two"):
    destination.mkdir(parents=True)
    shutil.copy2(binary, destination / executable)
    (destination / executable).chmod(0o755)
    shutil.copytree(assets, destination / "assets")


def mac_app(binary, assets, stage, version):
    app = stage / "Earth Two.app"
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    shutil.copy2(binary, contents / "MacOS/earth-two")
    (contents / "MacOS/earth-two").chmod(0o755)
    shutil.copytree(assets, contents / "Resources/assets")
    with (contents / "Info.plist").open("wb") as file:
        plistlib.dump({
            "CFBundleName": "Earth Two", "CFBundleDisplayName": "Earth Two",
            "CFBundleIdentifier": "com.struckchure.earth-two",
            "CFBundleExecutable": "earth-two", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version, "CFBundleVersion": version,
            "NSHighResolutionCapable": True,
        }, file)
    return app


def create_dmg(stage, dmg):
    command = ["hdiutil", "create", "-volname", "Earth Two", "-srcfolder", str(stage),
               "-fs", "HFS+", "-format", "UDZO", "-ov", str(dmg)]
    for attempt in range(3):
        result = subprocess.run(command, capture_output=True, text=True)
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if result.returncode == 0:
            return
        # The hosted macOS disk-image service occasionally stays busy after
        # creating its temporary volume. Do not retry other packaging errors.
        if "resource busy" not in result.stderr.lower() or attempt == 2:
            raise subprocess.CalledProcessError(result.returncode, command,
                                                output=result.stdout, stderr=result.stderr)
        delay = 2 * (attempt + 1)
        print(f"Disk-image service is busy; retrying in {delay}s", file=sys.stderr)
        time.sleep(delay)


def package_mac(binary, assets, stage, output, version, identity, notary_profile):
    app = mac_app(binary, assets, stage, version)
    signing = ["codesign", "--force", "--sign", identity or "-"]
    if identity:
        signing += ["--options", "runtime", "--timestamp"]
    run(*signing, app)
    run("codesign", "--verify", "--strict", app)
    (stage / "Applications").symlink_to("/Applications", target_is_directory=True)
    dmg = Path(str(output) + ".dmg")
    create_dmg(stage, dmg)
    run("hdiutil", "verify", dmg)
    if identity:
        run("codesign", "--sign", identity, "--timestamp", dmg)
    if notary_profile:
        run("xcrun", "notarytool", "submit", dmg, "--keychain-profile", notary_profile, "--wait")
        run("xcrun", "stapler", "staple", dmg)
        run("xcrun", "stapler", "validate", dmg)
    return [dmg]


def package_windows(binary, assets, stage, output, version):
    payload = stage / "Earth Two"
    copy_runtime(binary, assets, payload, "earth-two.exe")
    archive = Path(str(output) + ".zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as file:
        for path in sorted(payload.rglob("*")):
            if path.is_file():
                file.write(path, path.relative_to(stage))
    installer = Path(str(output) + "-setup.exe")
    # NSIS uses / switches on Windows and - switches on Unix hosts.
    prefix = "/" if os.name == "nt" else "-"
    run("makensis", prefix + "V2", prefix + "DVERSION=" + version,
        prefix + "DPAYLOAD=" + str(payload), prefix + "DOUTPUT=" + str(installer), HERE / "installer.nsi")
    return [installer, archive]


def linux_payload(binary, assets, stage, version, arch, dependencies):
    root = stage / "deb"
    copy_runtime(binary, assets, root / "opt/earth-two")
    (root / "usr/bin").mkdir(parents=True)
    (root / "usr/bin/earth-two").symlink_to("/opt/earth-two/earth-two")
    desktop = root / "usr/share/applications/earth-two.desktop"
    desktop.parent.mkdir(parents=True)
    desktop.write_text("[Desktop Entry]\nType=Application\nName=Earth Two\n"
                       "Comment=A free-roam science-fiction role game\nExec=/opt/earth-two/earth-two\n"
                       "TryExec=/opt/earth-two/earth-two\nIcon=applications-games\nTerminal=false\nCategories=Game;\n")
    control = root / "DEBIAN/control"
    control.parent.mkdir()
    size = sum(p.stat().st_size for p in (root / "opt").rglob("*") if p.is_file())
    control.write_text(f"Package: earth-two\nVersion: {version}\nArchitecture: {arch}\n"
                       "Maintainer: Earth Two <struckchure@users.noreply.github.com>\n"
                       f"Installed-Size: {(size + 1023) // 1024}\nSection: games\nPriority: optional\n"
                       f"Depends: {dependencies}\nDescription: Earth Two science-fiction role game\n"
                       " Explore the world, take contracts, and rise through the ranks.\n")
    return root


def package_linux(binary, assets, stage, output, version, arch):
    payload = stage / "Earth Two"
    copy_runtime(binary, assets, payload)
    archive = Path(str(output) + ".tar.gz")
    with tarfile.open(archive, "w:gz") as file:
        file.add(payload, arcname="earth-two")
    # dpkg-shlibdeps derives versioned dependencies from the actual binary,
    # including the build host's minimum glibc and libstdc++ versions.
    debian = stage / "debian"
    debian.mkdir()
    (debian / "control").write_text("Source: earth-two\nSection: games\nPriority: optional\n"
                                  "Maintainer: Earth Two <struckchure@users.noreply.github.com>\n"
                                  "Standards-Version: 4.7.0\n\nPackage: earth-two\nArchitecture: any\n"
                                  "Description: Earth Two science-fiction role game\n")
    dependencies = subprocess.check_output(
        ["dpkg-shlibdeps", "-O", str(binary)], cwd=stage, text=True).strip()
    dependencies = dependencies.removeprefix("shlibs:Depends=")
    if not dependencies or "\n" in dependencies:
        raise ValueError("dpkg-shlibdeps did not return runtime dependencies")
    root = linux_payload(binary, assets, stage, version, arch, dependencies)
    deb = Path(str(output) + ".deb")
    run("dpkg-deb", "--root-owner-group", "--build", root, deb)
    run("dpkg-deb", "--info", deb)
    return [deb, archive]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=["macos", "windows", "linux"], default=PLATFORMS.get(platform.system()))
    parser.add_argument("--arch", choices=["amd64", "arm64"], default=ARCHES.get(platform.machine()))
    parser.add_argument("--version", default="0.1.0", help="three-part numeric version, e.g. 0.1.0")
    parser.add_argument("--binary", type=Path, help="defaults to build/earth-two[.exe]")
    parser.add_argument("--assets", type=Path, default=Path("build/assets"))
    parser.add_argument("--out", type=Path, default=Path("build/dist"))
    parser.add_argument("--sign-identity", default=os.environ.get("MACOS_SIGN_IDENTITY"))
    parser.add_argument("--notary-profile", help="explicitly submit the signed DMG using this notarytool keychain profile")
    args = parser.parse_args()
    try:
        if not args.platform or not args.arch:
            raise ValueError("specify --platform and --arch for this host")
        if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", args.version) or any(int(n) > 65535 for n in args.version.split(".")):
            raise ValueError("version must be three numbers between 0 and 65535")
        if args.notary_profile and (args.platform != "macos" or not args.sign_identity):
            raise ValueError("--notary-profile requires macOS and a Developer ID --sign-identity")
        binary = (args.binary or Path("build/earth-two" + (".exe" if args.platform == "windows" else ""))).resolve()
        validate_binary(binary, args.platform, args.arch)
        assets = args.assets.resolve()
        if not (assets / ".assetpack").is_file():
            raise ValueError("assets must be output from go run ./tools/assetpack")
        for path in assets.rglob("*"):
            if path.is_symlink() or path.name.startswith(".env") or path.name.endswith(".earth-two-key.json"):
                raise ValueError(f"unexpected file in runtime assets: {path}")
        out = args.out.resolve()
        out.mkdir(parents=True, exist_ok=True)
        output = out / f"earth-two-{args.version}-{args.platform}-{args.arch}"
        with tempfile.TemporaryDirectory(prefix=".package-", dir=out) as directory:
            stage = Path(directory)
            if args.platform == "macos":
                artifacts = package_mac(binary, assets, stage, output, args.version, args.sign_identity, args.notary_profile)
            elif args.platform == "windows":
                artifacts = package_windows(binary, assets, stage, output, args.version)
            else:
                artifacts = package_linux(binary, assets, stage, output, args.version, args.arch)
        for path in artifacts:
            with path.open("rb") as file:
                digest = hashlib.file_digest(file, "sha256").hexdigest()
            Path(str(path) + ".sha256").write_text(f"{digest}  {path.name}\n")
            print(path)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"package: {error}\n")


if __name__ == "__main__":
    main()
