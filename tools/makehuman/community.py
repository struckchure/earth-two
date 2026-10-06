"""Fetches the MakeHuman community clothes in cast.COMMUNITY and installs
them where MPFB finds them, the way the README installs the boxer shorts by
hand: each in its own folder under MPFB's clothes, its .mhclo renamed to the
folder's name, its material cut down to the base colour texture (the game
draws nothing else).

    python3 tools/makehuman/community.py

Only CC0 and CC-BY assets are taken, and CC-BY needs its credit:
assets/characters/CREDITS.txt lists them. It fetches each asset once and
skips what's installed; everything lands in the git-ignored build/makehuman/.
"""
import json
import os
import sys
import urllib.parse
import urllib.request

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cast import COMMUNITY  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CLOTHES = os.path.join(ROOT, "build", "makehuman", "blender", "extensions", ".user", "user_default", "mpfb", "data", "clothes")
INDEX_URL = "http://www.makehumancommunity.org/sites/default/files/assets.json"
INDEX = os.path.join(ROOT, "build", "makehuman", "dl", "community_assets.json")
LICENCES = {"CC0", "CC-BY"}
# The files an asset needs: its fitting, mesh, material and base colour.
NEEDED = ("mhclo", "obj", "mhmat", "diffuse", "thumb")
# What's kept of a material: the game draws only the base colour.
KEPT = ("name", "tag", "description", "diffuseColor", "shadeless", "transparent", "alphaToCoverage", "backfaceCull",
        "castShadows", "receiveShadows", "shininess", "opacity")


def get(url):
    # The community's file names have spaces in them.
    parts = urllib.parse.urlsplit(url)
    safe = parts._replace(path=urllib.parse.quote(parts.path))
    req = urllib.request.Request(urllib.parse.urlunsplit(safe), headers={"User-Agent": "earth-two character build"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def index():
    if not os.path.exists(INDEX):
        os.makedirs(os.path.dirname(INDEX), exist_ok=True)
        with open(INDEX, "wb") as f:
            f.write(get(INDEX_URL))
    with open(INDEX) as f:
        return json.load(f)


def install(name, asset):
    folder = os.path.join(CLOTHES, name)
    if os.path.exists(os.path.join(folder, name + ".mhclo")):
        return
    if asset.get("license") not in LICENCES:
        raise SystemExit("%s: %s is %s, which the game can't use" % (name, asset["title"], asset.get("license")))
    files = asset["files"]
    missing = [k for k in ("mhclo", "obj", "mhmat", "diffuse") if k not in files]
    if missing:
        raise SystemExit("%s: %s has no %s" % (name, asset["title"], ", ".join(missing)))
    os.makedirs(folder, exist_ok=True)
    saved = {}
    for kind in NEEDED:
        if kind not in files:
            continue
        base = urllib.parse.unquote(os.path.basename(urllib.parse.urlsplit(files[kind]).path))
        if kind == "mhclo":
            base = name + ".mhclo"
        elif kind == "thumb":
            base = name + ".thumb"
        with open(os.path.join(folder, base), "wb") as f:
            f.write(get(files[kind]))
        saved[kind] = base
    # The fitting names its mesh and material: point them at what's here.
    mhclo = os.path.join(folder, saved["mhclo"])
    lines = []
    for line in open(mhclo, encoding="utf-8", errors="replace"):
        key = line.split(None, 1)[0] if line.strip() else ""
        if key == "obj_file":
            line = "obj_file %s\n" % saved["obj"]
        elif key == "material":
            line = "material %s\n" % saved["mhmat"]
        lines.append(line)
    with open(mhclo, "w", encoding="utf-8") as f:
        f.writelines(lines)
    # The material: its base colour texture, and none of the maps the game
    # doesn't draw (they aren't fetched).
    mhmat = os.path.join(folder, saved["mhmat"])
    lines = [line for line in open(mhmat, encoding="utf-8", errors="replace")
             if line.split(None, 1)[0:1] and line.split(None, 1)[0] in KEPT]
    lines.append("diffuseTexture %s\n" % saved["diffuse"])
    with open(mhmat, "w", encoding="utf-8") as f:
        f.writelines(lines)
    print("installed %s: %s by %s (%s)" % (name, asset["title"], asset["username"], asset["license"]))


def main():
    assets = index()
    for name, nid in COMMUNITY.items():
        install(name, assets[str(nid)])


if __name__ == "__main__":
    main()
