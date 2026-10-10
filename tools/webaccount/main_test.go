package main

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestShellBuild(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{"raylib.data", "game.wasm", "account.js", "wasm_exec.js", "fs.js", "raylib.js", "raylib.wasm", "jolt.js", "jolt.wasm"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("12345"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	source := []byte(`const modules = ["raylib","jolt"];`)
	result, err := buildShell(source, dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{`const modules = ["raylib","jolt"];`, `"game.wasm":5`, `"raylib.data":5`, `id="loading"`, `earthTwoReady`} {
		if !bytes.Contains(result, []byte(want)) {
			t.Errorf("missing %s", want)
		}
	}
	again, err := buildShell(result, dir)
	if err != nil || !bytes.Equal(result, again) {
		t.Fatal("shell build is not idempotent", err)
	}
	if _, err := buildShell([]byte("unexpected upstream shell"), dir); err == nil {
		t.Fatal("expected error for missing module list")
	}
	// Same size and timestamp must still invalidate a cached script.
	path := filepath.Join(dir, "raylib.js")
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("54321"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(path, info.ModTime(), info.ModTime()); err != nil {
		t.Fatal(err)
	}
	changed, err := buildShell(source, dir)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(result, changed) {
		t.Fatal("changed script did not change its version")
	}
}
