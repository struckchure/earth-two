"""Models from Sketchfab, for what Poly Haven doesn't have: the vehicles
and ships. A Sourced piece (or p.source) names one as "sketchfab:<uid>"
and polyhaven.py fetches it from here; after that it's fitted, decimated
and baked like any sourced model.

Only CC0 and CC-BY models are taken. CC-BY needs its credit: the title, the
author, a link and the licence go into assets/world/CREDITS.txt (credits()).
Non-commercial and editorial licences are refused, because the game isn't
free to use them.

Downloading needs a Sketchfab API key (sketchfab.com, Settings, Password
& API): SKETCHFAB_API_KEY in the repo's .env, which git ignores. Each model is
fetched once into build/sketchfab/<uid>/, so only the first build needs it.
"""
import json
import os
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
CACHE = ROOT / "build" / "sketchfab"
API = "https://api.sketchfab.com/v3"
PREFIX = "sketchfab:"
AGENT = {"User-Agent": "earth-two world build"}
LICENCES = {"cc0", "by"}


def ours(asset: str) -> bool:
    return asset.startswith(PREFIX)


def _uid(asset: str) -> str:
    return asset[len(PREFIX):] if ours(asset) else asset


def _token() -> str:
    """The API key: SKETCHFAB_API_KEY from the environment or the repo's
    .env (kept out of git), or ~/.config/sketchfab/token."""
    t = os.environ.get("SKETCHFAB_API_KEY", "").strip()
    env = ROOT / ".env"
    if not t and env.exists():
        for line in env.read_text().splitlines():
            key, _, value = line.partition("=")
            if key.strip() in ("SKETCHFAB_API_KEY", "SKETCHFAB_TOKEN"):
                t = value.strip().strip("'\"")
    path = Path.home() / ".config" / "sketchfab" / "token"
    if not t and path.exists():
        t = path.read_text().strip()
    if not t:
        raise SystemExit("sketchfab: no API key (SKETCHFAB_API_KEY in .env, or ~/.config/sketchfab/token)")
    return t


def _get(url: str, auth: bool = False) -> bytes:
    headers = dict(AGENT)
    if auth:
        headers["Authorization"] = f"Token {_token()}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=300) as r:
        return r.read()


def info(asset: str) -> dict:
    """The model's name, author, licence and page, cached; refuses a
    licence the game can't use."""
    uid = _uid(asset)
    path = CACHE / uid / "info.json"
    if not path.exists():
        d = json.loads(_get(f"{API}/models/{uid}"))
        lic = d.get("license") or {}
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"name": d["name"], "author": d["user"]["displayName"],
                                    "licence": lic.get("label", "?"), "slug": lic.get("slug", "?"),
                                    "url": d["viewerUrl"]}))
    i = json.loads(path.read_text())
    if i["slug"] not in LICENCES:
        raise SystemExit(f"sketchfab: {i['name']} is {i['licence']}, which the game can't use")
    return i


def fetch(asset: str) -> Path:
    """The model's glTF, with its textures beside it, downloaded once."""
    uid = _uid(asset)
    folder = CACHE / uid
    done = folder / ".done"
    if done.exists():
        return folder / done.read_text().strip()
    info(asset)
    links = json.loads(_get(f"{API}/models/{uid}/download", auth=True))
    folder.mkdir(parents=True, exist_ok=True)
    archive = folder / "model.zip"
    archive.write_bytes(_get(links["gltf"]["url"]))
    with zipfile.ZipFile(archive) as z:
        for m in z.infolist():
            # Only files inside the folder: nothing absolute, nothing climbing out.
            target = (folder / m.filename).resolve()
            if not str(target).startswith(str(folder.resolve()) + os.sep):
                raise ValueError(f"sketchfab: {uid}'s archive has a path outside it: {m.filename}")
        z.extractall(folder)
    archive.unlink()
    gltf = next(iter(sorted(folder.rglob("*.gltf"), key=lambda p: len(p.parts))), None)
    if gltf is None:
        raise ValueError(f"sketchfab: {uid} has no glTF")
    name = str(gltf.relative_to(folder))
    done.write_text(name)
    return folder / name


def credits(assets: set[str]) -> list[str]:
    """CREDITS.txt's lines for the Sketchfab models used."""
    if not assets:
        return []
    lines = ["Vehicles and ships built on Sketchfab models (https://sketchfab.com),",
             "retrofitted for Earth Two: repainted, worn, parts added and taken off.", ""]
    for a in sorted(assets, key=lambda a: info(a)["name"].lower()):
        i = info(a)
        lines.append(f"  \"{i['name']}\" by {i['author']} ({i['licence']}): {i['url']}")
    return lines + [""]
