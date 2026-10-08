#!/usr/bin/env python3
"""Detect changes to game and release inputs without rebuilding for the website."""
import argparse
from pathlib import Path
import subprocess


# Keep game runtime, shared dependencies, asset generation and release tooling
# covered. Website code, website hosting, narrative docs and trailer editing
# live outside these paths. Register new game packages here when adding them.
GAME_FILES = {
    ".github/workflows/ci.yml",
    "go.mod",
    "go.sum",
    "Makefile",
    "railpack.json",
    "infra/ota-storage.json",
}
GAME_PREFIXES = (
    "assetref/", "assets/", "character/", "game/", "identity/",
    "internals/", "scenes/", "shading/", "vehicle/", "world/",
    "cmd/account/", "cmd/desktop/", "cmd/server/", "cmd/web/",
    "web/spacetime/",
    "tools/assetpack/", "tools/bindpose/", "tools/buildenv/", "tools/ci/",
    "tools/deps/", "tools/desktop/", "tools/internal/", "tools/makehuman/",
    "tools/ota/", "tools/paint/", "tools/server/", "tools/webaccount/",
    "tools/webcompress/", "tools/world/",
)


def game_input(name):
    # Tool README changes do not alter release artifacts. Runtime credit and
    # licence files in assets/ remain inputs because they ship with the game.
    if name.endswith(".md") or Path(name).name == "README.txt":
        return False
    return name in GAME_FILES or name.startswith(GAME_PREFIXES)


def changed_paths(base, head, merge_base=False):
    if not base or set(base) == {"0"}:
        # A first push has no predecessor. Consider the complete tracked tree.
        command = ["git", "ls-tree", "-r", "--name-only", "-z", head]
    else:
        comparison = [f"{base}...{head}"] if merge_base else [base, head]
        # Report both sides of renames, including moves out of game folders.
        command = ["git", "diff", "--name-only", "--no-renames", "-z", *comparison, "--"]
    result = subprocess.check_output(command)
    return [p.decode("utf-8", errors="surrogateescape") for p in result.split(b"\0") if p]


def game_changed(base, head, merge_base=False):
    return any(game_input(name) for name in changed_paths(base, head, merge_base))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--merge-base", action="store_true", help="compare only the PR branch's changes")
    parser.add_argument("--github-output", type=Path, help="append the game output to this step's output file")
    args = parser.parse_args()
    value = "true" if game_changed(args.base, args.head, args.merge_base) else "false"
    if args.github_output is not None:
        with args.github_output.open("a", encoding="utf-8") as output:
            output.write(f"game={value}\n")
    print(value)


if __name__ == "__main__":
    main()
