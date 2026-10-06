package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestPatchRejectsChangedSource(t *testing.T) {
	for _, p := range patches {
		if _, err := apply([]byte("unrecognised upstream source"), p); err == nil {
			t.Fatalf("%s: accepted a source without the expected patch context", p.module)
		}
		if _, err := apply([]byte(p.old+p.old), p); err == nil {
			t.Fatalf("%s: accepted ambiguous patch context", p.module)
		}
	}
}

func TestCopyPreservesUpstreamAndRefreshesChanges(t *testing.T) {
	src, cache := t.TempDir(), t.TempDir()
	p := patch{module: "example.com/dep", file: "source.c", old: "broken", fixed: "fixed"}
	write := func(file, data string) {
		t.Helper()
		if err := os.WriteFile(filepath.Join(src, file), []byte(data), 0644); err != nil {
			t.Fatal(err)
		}
	}
	write(p.file, p.old)
	write("other.c", "first")
	first, err := patchedCopy(cache, src, p)
	if err != nil {
		t.Fatal(err)
	}
	for file, want := range map[string]string{filepath.Join(src, p.file): p.old, filepath.Join(first, p.file): p.fixed} {
		data, err := os.ReadFile(file)
		if err != nil || string(data) != want {
			t.Fatalf("%s: got %q, %v; want %q", file, data, err, want)
		}
	}
	again, err := patchedCopy(cache, src, p)
	if err != nil || again != first {
		t.Fatalf("unchanged source was not reused: %q, %v", again, err)
	}
	write("other.c", "second")
	second, err := patchedCopy(cache, src, p)
	if err != nil || second == first {
		t.Fatalf("changed source was not refreshed: %q, %v", second, err)
	}
	data, err := os.ReadFile(filepath.Join(second, "other.c"))
	if err != nil || string(data) != "second" {
		t.Fatalf("refreshed copy: got %q, %v", data, err)
	}
}

func TestVorbisOffsetBounds(t *testing.T) {
	p := patches[0]
	src, err := moduleDir(p.module)
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(src, p.file))
	if err != nil {
		t.Fatal(err)
	}
	data, err = apply(data, p)
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	file := filepath.Join(dir, "stb_vorbis.c")
	if err := os.WriteFile(file, data, 0644); err != nil {
		t.Fatal(err)
	}
	cc, err := exec.Command("go", "env", "CC").Output()
	if err != nil {
		t.Fatal(err)
	}
	compiler := strings.Fields(string(cc))
	bin := filepath.Join(dir, "offset")
	if runtime.GOOS == "windows" {
		bin += ".exe"
	}
	args := append(compiler[1:], "-O2", "-Werror=tautological-compare", "-DSTB_VORBIS_SOURCE="+`"`+filepath.ToSlash(file)+`"`, "testdata/offset.c", "-o", bin, "-lm")
	if out, err := exec.Command(compiler[0], args...).CombinedOutput(); err != nil {
		t.Fatalf("compile offset regression: %v\n%s", err, out)
	}
	if out, err := exec.Command(bin).CombinedOutput(); err != nil {
		t.Fatalf("offset regression: %v\n%s", err, out)
	}
}
