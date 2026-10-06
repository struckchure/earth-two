package main

import (
	"bytes"
	"compress/gzip"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRoundTripAndReplace(t *testing.T) {
	path := filepath.Join(t.TempDir(), "raylib.data")
	for _, data := range [][]byte{bytes.Repeat([]byte("asset contents"), 1000), []byte("a rebuilt asset pack")} {
		if err := os.WriteFile(path, data, 0644); err != nil {
			t.Fatal(err)
		}
		raw, packed, err := compress(path)
		if err != nil {
			t.Fatal(err)
		}
		encoded, err := os.ReadFile(path + ".gz")
		if err != nil {
			t.Fatal(err)
		}
		if raw != int64(len(data)) || packed != int64(len(encoded)) {
			t.Fatalf("reported sizes %d/%d, actual %d/%d", raw, packed, len(data), len(encoded))
		}
		reader, err := gzip.NewReader(bytes.NewReader(encoded))
		if err != nil {
			t.Fatal(err)
		}
		decoded, err := io.ReadAll(reader)
		reader.Close()
		if err != nil || !bytes.Equal(decoded, data) {
			t.Fatalf("decoded %q: %v", decoded, err)
		}
		original, err := os.ReadFile(path)
		if err != nil || !bytes.Equal(original, data) {
			t.Fatal("compression changed the original")
		}
	}
	leftovers, err := filepath.Glob(filepath.Join(filepath.Dir(path), ".gzip-*"))
	if err != nil || len(leftovers) != 0 {
		t.Fatalf("temporary files left behind: %v (%v)", leftovers, err)
	}
}
