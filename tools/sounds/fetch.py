"""The game's sounds: fetched from CC0 sources, cut to size and written to
assets/sounds, with their credits in assets/sounds/CREDITS.txt.

    python3 tools/sounds/fetch.py

What's taken, and from where, is sounds.json beside this file. Each entry
is one sound the game plays, by name (game/sound.go). Two kinds of source:

- "kenney": files from one of Kenney's packs (kenney.nl, CC0), copied as
  they are. Several files make variants: name_0.ogg, name_1.ogg, ...
- "freesound": a sound from freesound.org, only ever CC0 ones. The public
  preview is fetched (no account needed), decoded with lame, folded to
  mono and levelled. Then either
    - "loop": the stretch from "start" for "length" seconds, its end
      crossfaded into its start so it loops without a seam (ambience,
      engines: streamed by the game); or
    - "slices": that many separate hits cut out of a take of several
      (footsteps, whooshes), the loudest ones; or
    - neither: the stretch from "start" for "length", as one sound.

Downloads are kept in build/sounds/, so a second run only re-cuts. It needs
curl and lame (brew install lame) on PATH, and nothing outside Python's
standard library.
"""
import array
import json
import math
import shutil
import subprocess
import sys
import wave
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
MANIFEST = Path(__file__).resolve().parent / "sounds.json"
CACHE = ROOT / "build" / "sounds"
OUT = ROOT / "assets" / "sounds"
AGENT = "earth-two sound fetch"

# Loops are levelled by loudness (RMS), hits by their peak.
LOOP_RMS = 0.12
HIT_PEAK = 0.9
# Ambience is low and broad: half the rate loses nothing anyone hears in a
# hum or the wind, and halves the files.
LOOP_RATE_DIVISOR = 2
CROSSFADE = 1.0  # seconds a loop's end is blended into its start


def fetch(url: str, to: Path) -> Path:
    if not to.exists():
        to.parent.mkdir(parents=True, exist_ok=True)
        part = to.with_suffix(to.suffix + ".part")
        subprocess.run(["curl", "-sfL", "-A", AGENT, "-o", str(part), url], check=True)
        part.rename(to)
    return to


def kenney(pack: dict, files: list[str]) -> list[Path]:
    """The pack's files, unzipped once into build/sounds/kenney/<pack>/."""
    name = pack["name"]
    z = fetch(pack["url"], CACHE / "kenney" / f"{name}.zip")
    into = CACHE / "kenney" / name
    if not into.exists():
        with zipfile.ZipFile(z) as f:
            f.extractall(into)
    found = []
    for want in files:
        hits = sorted(into.rglob(want + ".ogg"))
        if not hits:
            sys.exit(f"{name}: no {want}.ogg in the pack")
        found.append(hits[0])
    return found


def decode(mp3: Path) -> tuple[list[float], int]:
    """Mono samples in -1..1 and their rate."""
    wav = mp3.with_suffix(".wav")
    if not wav.exists():
        subprocess.run(["lame", "--quiet", "--decode", str(mp3), str(wav)], check=True)
    with wave.open(str(wav)) as w:
        ch, rate, width = w.getnchannels(), w.getframerate(), w.getsampwidth()
        if width != 2:
            sys.exit(f"{wav}: {8 * width}-bit, expected 16")
        data = array.array("h", w.readframes(w.getnframes()))
    if sys.byteorder == "big":
        data.byteswap()
    if ch == 1:
        return [v / 32768 for v in data], rate
    return [sum(data[i:i + ch]) / (32768 * ch) for i in range(0, len(data), ch)], rate


def write(path: Path, samples: list[float], rate: int):
    path.parent.mkdir(parents=True, exist_ok=True)
    data = array.array("h", (max(-32767, min(32767, int(v * 32767))) for v in samples))
    if sys.byteorder == "big":
        data.byteswap()
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(data.tobytes())


def rms(s: list[float]) -> float:
    return math.sqrt(sum(v * v for v in s) / max(1, len(s)))


def gain(s: list[float], by: float) -> list[float]:
    return [v * by for v in s]


def halve(s: list[float]) -> list[float]:
    return [(s[i] + s[i + 1]) / 2 for i in range(0, len(s) - 1, 2)]


def loop(s: list[float], rate: int) -> list[float]:
    """The stretch with its last CROSSFADE seconds blended into its first,
    so the end runs on into the start."""
    x = min(int(CROSSFADE * rate), len(s) // 3)
    body, tail = s[:len(s) - x], s[len(s) - x:]
    for i in range(x):
        f = i / x
        # Equal power, so the blend doesn't dip.
        body[i] = body[i] * math.sin(f * math.pi / 2) + tail[i] * math.cos(f * math.pi / 2)
    return body


def slices(s: list[float], rate: int, n: int) -> list[list[float]]:
    """The n loudest hits in a take: each from just before it rises out of
    the quiet to the next one (half a second at most), faded out."""
    win = rate // 100
    env = [rms(s[i:i + win]) for i in range(0, len(s) - win, win)]
    floor = sorted(env)[len(env) // 5]
    peak = max(env)
    rise = floor + (peak - floor) * .25
    onsets, quiet = [], True
    for i, e in enumerate(env):
        if quiet and e > rise:
            onsets.append(i)
            quiet = False
        elif e < floor + (peak - floor) * .08:
            quiet = True
    hits = []
    for k, o in enumerate(onsets):
        a = max(0, (o - 1) * win)
        b = min(len(s), a + rate // 2, onsets[k + 1] * win if k + 1 < len(onsets) else len(s))
        hit = s[a:b]
        fade = min(len(hit) // 4, rate // 20)
        for i in range(fade):
            hit[len(hit) - 1 - i] *= i / fade
        hits.append(hit)
    hits.sort(key=lambda h: -max(map(abs, h)))
    return hits[:n]


def peak_to(s: list[float], to: float) -> list[float]:
    return gain(s, to / max(1e-6, max(map(abs, s))))


def freesound(e: dict) -> list[tuple[str, list[float], int]]:
    sid = e["freesound"]
    mp3 = fetch(e["url"], CACHE / "freesound" / f"{sid}.mp3")
    s, rate = decode(mp3)
    a = int(e.get("start", 0) * rate)
    b = a + int(e["length"] * rate) if "length" in e else len(s)
    s = s[a:b]
    name = e["name"]
    if e.get("loop"):
        if LOOP_RATE_DIVISOR > 1 and not e.get("full_rate"):
            s, rate = halve(s), rate // LOOP_RATE_DIVISOR
        s = loop(s, rate)
        # As loud as LOOP_RMS, unless that would clip.
        by = min(LOOP_RMS / max(1e-6, rms(s)), .95 / max(1e-6, max(map(abs, s))))
        return [(name, gain(s, by), rate)]
    if "slices" in e:
        return [(f"{name}_{i}", peak_to(h, HIT_PEAK), rate) for i, h in enumerate(slices(s, rate, e["slices"]))]
    return [(name, peak_to(s, HIT_PEAK), rate)]


def main():
    m = json.loads(MANIFEST.read_text())
    packs = {p["name"]: p for p in m["kenney"]}
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    credits = {"kenney": set(), "freesound": []}
    for e in m["sounds"]:
        name = e["name"]
        if "kenney" in e:
            files = kenney(packs[e["kenney"]], e["files"])
            for i, f in enumerate(files):
                to = OUT / (f"{name}_{i}.ogg" if len(files) > 1 else f"{name}.ogg")
                shutil.copyfile(f, to)
            credits["kenney"].add(e["kenney"])
        else:
            for out, s, rate in freesound(e):
                write(OUT / f"{out}.wav", s, rate)
            credits["freesound"].append(e)
        print(name)
    lines = [
        "Sounds from Kenney (https://kenney.nl), CC0 1.0: public domain, no",
        "credit needed, but here it is.",
        "",
    ]
    lines += [f"  {packs[p]['title']}: {packs[p]['page']}" for p in sorted(credits["kenney"])]
    lines += [
        "",
        "Sounds from Freesound (https://freesound.org), all CC0 1.0: public",
        "domain, no credit needed, but here it is. Cut, levelled and looped for",
        "Earth Two by tools/sounds/fetch.py.",
        "",
    ]
    lines += [
        f"  {e['name']}: \"{e['title']}\" by {e['author']}: https://freesound.org/s/{e['freesound']}/"
        for e in credits["freesound"]
    ]
    (OUT / "CREDITS.txt").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
