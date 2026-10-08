import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from game_changes import game_changed, game_input

DETECTOR = Path(__file__).with_name("game_changes.py").resolve()


class ReleaseInputsTest(unittest.TestCase):
    def test_landing_and_editorial_changes_do_not_release(self):
        for path in (
            "web/landing/index.html", "web/landing/styles.css",
            "web/landing/landing.go", "web/landing/assets/landfall.jpg",
            "cmd/landing/main.go", "railpack.landing-page.json",
            "README.md", "docs/story.md", "tools/desktop/README.md",
            "tools/trailer/assemble.py", "tools/trailer/README.txt",
        ):
            with self.subTest(path=path):
                self.assertFalse(game_input(path))

    def test_game_and_release_dependencies_remain_covered(self):
        for path in (
            "game/game.go", "character/ragdoll.go", "assets/world/dome.glb",
            "assets/world/CREDITS.txt", "game/fonts/CourierPrime-OFL.txt",
            "cmd/desktop/main.go", "cmd/web/game_js.go", "web/spacetime/bridge.ts",
            "internals/account/session.go", "identity/key.go", "scenes/landfall.json",
            "shading/toon.go", "vehicle/handling.go", "world/merge.go", "assetref/index.go",
            "tools/assetpack/manifest.json", "tools/deps/main.go", "tools/desktop/package.py",
            "tools/buildenv/main.go", "tools/ota/release.py", "tools/ci/game_changes.py",
            "go.mod", "go.sum", "Makefile", "railpack.json", "infra/ota-storage.json",
            ".github/workflows/ci.yml",
        ):
            with self.subTest(path=path):
                self.assertTrue(game_input(path))


class GitChangesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        previous = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, previous)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "CI filter test")
        self.git("config", "user.email", "ci-test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.write("game/main.go", "initial game\n")
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(["git", *args], stderr=subprocess.PIPE).decode().strip()

    def write(self, name, content):
        file = Path(name)
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(content)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-q", "-m", "test change")
        return self.git("rev-parse", "HEAD")

    def test_landing_followup_does_not_supersede_game_release(self):
        self.write("web/landing/index.html", "landing update\n")
        head = self.commit()
        self.assertFalse(game_changed(self.base, head))
        self.write("game/main.go", "new gameplay\n")
        self.assertTrue(game_changed(self.base, self.commit()))

    def test_mixed_commit_still_releases(self):
        self.write("web/landing/styles.css", "new website style\n")
        self.write("game/main.go", "new gameplay\n")
        self.assertTrue(game_changed(self.base, self.commit()))

    def test_move_out_of_game_is_a_game_deletion(self):
        Path("web/landing").mkdir(parents=True)
        self.git("mv", "game/main.go", "web/landing/example.go")
        self.assertTrue(game_changed(self.base, self.commit()))

    def test_pr_ignores_unrelated_base_branch_changes(self):
        self.git("checkout", "-q", "-b", "website")
        self.write("web/landing/index.html", "website branch\n")
        head = self.commit()
        self.git("checkout", "-q", "main")
        self.write("game/main.go", "unrelated base change\n")
        base = self.commit()
        self.assertTrue(game_changed(base, head))
        self.assertFalse(game_changed(base, head, merge_base=True))

    def test_first_push_checks_the_tracked_tree(self):
        self.assertTrue(game_changed("0" * 40, self.base))

    def test_cli_writes_actions_output(self):
        self.write("web/landing/index.html", "website update\n")
        head = self.commit()
        output = Path("actions-output")
        output.write_text("existing=value\n")
        result = subprocess.check_output([
            sys.executable, str(DETECTOR), "--base", self.base,
            "--head", head, "--github-output", str(output),
        ], text=True)
        self.assertEqual(result, "false\n")
        self.assertEqual(output.read_text(), "existing=value\ngame=false\n")

    def test_invalid_git_range_fails_instead_of_silently_skipping(self):
        with self.assertRaises(subprocess.CalledProcessError):
            game_changed("nonexistent-ref", self.base)


if __name__ == "__main__":
    unittest.main()
