# Desktop installers

From the repository root, run `make package VERSION=0.1.0` on the target OS.
This builds the game with its public connection defaults, packs the runtime
assets, and writes installers and SHA-256 checksum files to `build/dist/`.
Python 3.11 or newer is required. Versions must be three numeric components.

| Host | Output | Packaging tools | Install |
| --- | --- | --- | --- |
| macOS | `.dmg` with `Earth Two.app` | Xcode command line tools (`hdiutil`, `codesign`) | Drag the app to Applications |
| Windows | `-setup.exe` and portable `.zip` | [NSIS](https://nsis.sourceforge.io/) on PATH | Run setup; installs for the current user and adds Start menu shortcuts |
| Debian/Ubuntu Linux | `.deb` and portable `.tar.gz` | `dpkg-dev` | `sudo apt install ./earth-two-<version>-linux-amd64.deb` |

The portable archives contain the executable and `assets/`. Extract them
together and launch the executable. Linux archives use the same system libraries
as the `.deb`; they are not AppImages. The `.deb` records dependencies derived
from the actual executable with `dpkg-shlibdeps`, including minimum library versions.

Assets in macOS bundles live in `Contents/Resources/assets`. Linux packages
install into `/opt/earth-two`, with a `/usr/bin/earth-two` symlink and a desktop
menu entry. Windows installers include an uninstaller registered in Settings.
Uninstalling does not remove player identity files in the user's home directory.
Only the built binary and packed asset graph are shipped; source files, `.env`,
server credentials, and build reports are excluded. Asset credits are retained.

To package a previously built binary without rebuilding:

```sh
python3 tools/desktop/package.py --version 0.1.0
python3 tools/desktop/package.py --platform windows --arch amd64 \
  --binary build/earth-two.exe --assets build/assets --version 0.1.0
```

The script checks the executable's platform and architecture before assigning
the package name. macOS and Linux installers require their native packaging
tools; Windows NSIS packages can also be generated from a Unix host.

CI builds Apple Silicon and Intel macOS DMGs, Windows x64 installers and ZIPs,
and Linux x64 DEBs and archives. They appear as `installers-<platform>-<arch>`
Actions artifacts. Set the repository variable `APP_VERSION` to a numeric
version (default `0.1.0`). These installer artifacts are separate from OTA
archives; this workflow does not publish installers to a download server.

## macOS signing

The default DMG uses an ad-hoc signed app for local testing. Public distribution
needs a Developer ID Application certificate and notarization, as described in
[Apple's distribution guide](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution).
The script supports an identity already installed in the local keychain:

```sh
MACOS_SIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
  make package VERSION=0.1.0
```

To explicitly submit a signed DMG to Apple and staple the accepted ticket,
first configure a `notarytool` keychain profile, then run:

```sh
python3 tools/desktop/package.py --version 0.1.0 \
  --sign-identity 'Developer ID Application: Your Name (TEAMID)' \
  --notary-profile earth-two
```

CI currently produces ad-hoc signed macOS apps and unsigned Windows installers.
Signing identities and notarization credentials are not stored in the repository.

## Checks

```sh
python3 -m unittest discover -s tools/desktop -p 'test_*.py' -v
make test
```
