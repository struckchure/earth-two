//go:build !js

package main

import (
	"bytes"
	"compress/gzip"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestAcceptsGzip(t *testing.T) {
	for _, tt := range []struct {
		header string
		want   bool
	}{
		{"", false}, {"br", false}, {"gzip", true},
		{"br, gzip;q=0.5", true}, {"gzip;q=0", false},
		{"*;q=1, gzip;q=0", false}, {"gzip;q=0, *;q=1", false},
		{"*", true}, {"*;q=0", false}, {"GZip; q=0.2", true},
		{"gzip;q=invalid", false}, {"gzip;q=NaN", false},
		{"gzip;q=2", false}, {"gzip;q=-1", false},
	} {
		if got := acceptsGzip(tt.header); got != tt.want {
			t.Errorf("acceptsGzip(%q) = %v, want %v", tt.header, got, tt.want)
		}
	}
}

func TestCompressedFiles(t *testing.T) {
	dir := t.TempDir()
	original := []byte("\x00asm browser build contents")
	var packed bytes.Buffer
	zip := gzip.NewWriter(&packed)
	if _, err := zip.Write(original); err != nil {
		t.Fatal(err)
	}
	if err := zip.Close(); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"game.wasm", "raylib.data"} {
		if err := os.WriteFile(filepath.Join(dir, name), original, 0644); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, name+".gz"), packed.Bytes(), 0644); err != nil {
			t.Fatal(err)
		}
	}
	handler, err := newFileHandler(dir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := handler.Close(); err != nil {
			t.Errorf("close file handler: %v", err)
		}
	})
	request := func(method, name, encoding, modified string) *httptest.ResponseRecorder {
		r := httptest.NewRequest(method, name, nil)
		r.Header.Set("Accept-Encoding", encoding)
		if modified != "" {
			r.Header.Set("If-Modified-Since", modified)
		}
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, r)
		return w
	}
	for _, tt := range []struct{ name, mime string }{
		{"/game.wasm", "application/wasm"}, {"/raylib.data", "application/octet-stream"},
	} {
		w := request(http.MethodGet, tt.name, "gzip", "")
		if w.Code != 200 || w.Header().Get("Content-Encoding") != "gzip" || w.Header().Get("Content-Type") != tt.mime {
			t.Fatalf("%s: status %d, headers %v", tt.name, w.Code, w.Header())
		}
		reader, err := gzip.NewReader(w.Body)
		if err != nil {
			t.Fatal(err)
		}
		body, err := io.ReadAll(reader)
		reader.Close()
		if err != nil || !bytes.Equal(body, original) {
			t.Fatalf("decoded body %q: %v", body, err)
		}
		if w.Header().Get("Vary") != "Accept-Encoding" {
			t.Fatal("missing Vary header")
		}
		if cached := request(http.MethodGet, tt.name, "gzip", w.Header().Get("Last-Modified")); cached.Code != 304 {
			t.Fatalf("conditional request: %d", cached.Code)
		}
		if head := request(http.MethodHead, tt.name, "gzip", ""); head.Code != 200 || head.Body.Len() != 0 {
			t.Fatalf("HEAD: status %d, body size %d", head.Code, head.Body.Len())
		}
		for _, encoding := range []string{"", "gzip;q=0"} {
			identity := request(http.MethodGet, tt.name, encoding, "")
			if identity.Header().Get("Content-Encoding") != "" || !bytes.Equal(identity.Body.Bytes(), original) {
				t.Fatalf("identity request returned compressed body: %v", identity.Header())
			}
		}
	}
	old := time.Now().Add(-time.Hour)
	if err := os.Chtimes(filepath.Join(dir, "game.wasm.gz"), old, old); err != nil {
		t.Fatal(err)
	}
	if stale := request(http.MethodGet, "/game.wasm", "gzip", ""); stale.Header().Get("Content-Encoding") != "" || !bytes.Equal(stale.Body.Bytes(), original) {
		t.Fatal("served stale compressed artifact")
	}
	if missing := request(http.MethodGet, "/missing.wasm", "gzip", ""); missing.Code != 404 {
		t.Fatalf("missing file: %d", missing.Code)
	}
	// A sidecar alone must not resurrect a removed build artifact.
	if err := os.Remove(filepath.Join(dir, "raylib.data")); err != nil {
		t.Fatal(err)
	}
	if orphan := request(http.MethodGet, "/raylib.data", "gzip", ""); orphan.Code != 404 {
		t.Fatalf("orphaned sidecar: %d", orphan.Code)
	}
}

func TestFileHandlerReleasesRoot(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "site")
	if err := os.Mkdir(dir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "asset.txt"), []byte("build asset"), 0644); err != nil {
		t.Fatal(err)
	}
	handler, err := newFileHandler(dir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = handler.Close() })
	request := func() *httptest.ResponseRecorder {
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/asset.txt", nil))
		return response
	}
	if response := request(); response.Code != http.StatusOK {
		t.Fatalf("open handler: status %d", response.Code)
	}
	if err := handler.Close(); err != nil {
		t.Fatal(err)
	}
	// On Unix an open directory can still be removed. A closed handler must
	// also lose filesystem access, so the lifecycle regression is portable.
	if response := request(); response.Code == http.StatusOK {
		t.Fatal("closed handler still serves files from its root")
	}
	if err := os.RemoveAll(dir); err != nil {
		t.Fatalf("remove closed root: %v", err)
	}
}
