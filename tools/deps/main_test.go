package main

import (
	"bytes"
	"fmt"
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
	other := patch{module: p.module, file: "other.c", old: "first", fixed: "also fixed"}
	first, err := patchedCopy(cache, src, p, other)
	if err != nil {
		t.Fatal(err)
	}
	for file, want := range map[string]string{
		filepath.Join(src, p.file): p.old, filepath.Join(first, p.file): p.fixed,
		filepath.Join(src, other.file): other.old, filepath.Join(first, other.file): other.fixed,
	} {
		data, err := os.ReadFile(file)
		if err != nil || string(data) != want {
			t.Fatalf("%s: got %q, %v; want %q", file, data, err, want)
		}
	}
	again, err := patchedCopy(cache, src, p, other)
	if err != nil || again != first {
		t.Fatalf("unchanged source was not reused: %q, %v", again, err)
	}
	write("unpatched.c", "second")
	second, err := patchedCopy(cache, src, p, other)
	if err != nil || second == first {
		t.Fatalf("changed source was not refreshed: %q, %v", second, err)
	}
	data, err := os.ReadFile(filepath.Join(second, "unpatched.c"))
	if err != nil || string(data) != "second" {
		t.Fatalf("refreshed copy: got %q, %v", data, err)
	}
}

func TestCursorStartsInsideWithoutEnterEvent(t *testing.T) {
	p := patches[1]
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
	if again, err := apply(data, p); err != nil || !bytes.Equal(again, data) {
		t.Fatalf("already patched source changed: %v", err)
	}
	// Exercise the patched initialization and upstream enter/leave callback
	// with GLFW queries stubbed, so this regression needs no graphics display.
	start := strings.Index(string(data), "static void CursorEnterCallback(GLFWwindow *window, int enter)\n{")
	if start < 0 {
		t.Fatal("cursor callback definition missing")
	}
	end := strings.Index(string(data[start:]), "\n}")
	if end < 0 {
		t.Fatal("cursor callback end missing")
	}
	callback := string(data[start : start+end+2])
	query := patches[2]
	core, err := os.ReadFile(filepath.Join(src, query.file))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := apply(core, query); err != nil {
		t.Fatal(err)
	}
	harness := fmt.Sprintf(`#include <assert.h>
#include <stdbool.h>
#define PLATFORM_DESKTOP_GLFW
typedef struct { float x, y; } Vector2;
typedef int GLFWwindow;
enum { GLFW_HOVERED = 1 };
static struct {
    struct {
        struct { bool cursorOnScreen; Vector2 currentPosition, previousPosition; } Mouse;
        struct { Vector2 position[1]; } Touch;
    } Input;
} CORE;
static GLFWwindow window;
static struct { GLFWwindow *handle; } platform = { &window };
static int hovered;
static void (*enterCallback)(GLFWwindow *, int);
static int glfwGetWindowAttrib(GLFWwindow *w, int attr) {
    assert(w == &window && attr == GLFW_HOVERED);
    return hovered;
}
static void glfwGetCursorPos(GLFWwindow *w, double *x, double *y) {
    assert(w == &window);
    *x = 123.0; *y = 234.0;
}
static void glfwSetCursorEnterCallback(GLFWwindow *w, void (*cb)(GLFWwindow *, int)) {
    assert(w == &window); enterCallback = cb;
}
%s
%s
static void initialize(void) {
%s
}
int main(void) {
    // The window is not hovered yet during initialization. It appears under
    // the stationary pointer afterwards, without a cursor-enter callback.
    hovered = 0;
    initialize();
    hovered = 1;
    assert(!CORE.Input.Mouse.cursorOnScreen);
    assert(IsCursorOnScreen());
    hovered = 0;
    assert(!IsCursorOnScreen());
    hovered = 1;
    initialize();
    assert(CORE.Input.Mouse.cursorOnScreen);
    assert(CORE.Input.Mouse.currentPosition.x == 123.0f);
    assert(CORE.Input.Mouse.currentPosition.y == 234.0f);
    assert(CORE.Input.Mouse.previousPosition.x == CORE.Input.Mouse.currentPosition.x);
    assert(CORE.Input.Mouse.previousPosition.y == CORE.Input.Mouse.currentPosition.y);
    assert(CORE.Input.Touch.position[0].x == 123.0f);
    assert(CORE.Input.Touch.position[0].y == 234.0f);
    enterCallback(&window, 0);
    assert(!CORE.Input.Mouse.cursorOnScreen);
    enterCallback(&window, 1);
    assert(CORE.Input.Mouse.cursorOnScreen);
    // Even a stale true callback flag must not draw over another window.
    hovered = 0;
    assert(!IsCursorOnScreen());
    hovered = 0;
    initialize();
    assert(!CORE.Input.Mouse.cursorOnScreen);
    return 0;
}
`, callback, query.fixed, p.fixed)
	dir := t.TempDir()
	file := filepath.Join(dir, "cursor.c")
	if err := os.WriteFile(file, []byte(harness), 0644); err != nil {
		t.Fatal(err)
	}
	cc, err := exec.Command("go", "env", "CC").Output()
	if err != nil {
		t.Fatal(err)
	}
	compiler := strings.Fields(string(cc))
	bin := filepath.Join(dir, "cursor")
	if runtime.GOOS == "windows" {
		bin += ".exe"
	}
	args := append(compiler[1:], file, "-o", bin)
	if out, err := exec.Command(compiler[0], args...).CombinedOutput(); err != nil {
		t.Fatalf("compile cursor regression: %v\n%s", err, out)
	}
	if out, err := exec.Command(bin).CombinedOutput(); err != nil {
		t.Fatalf("cursor regression: %v\n%s", err, out)
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
