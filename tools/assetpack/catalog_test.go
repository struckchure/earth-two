package main

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"

	"github.com/struckchure/earth-two/assetref"
)

func TestCatalogueMatchesPackedFiles(t *testing.T) {
	source, output, config := fixture(t)
	r, err := pack(source, output, config)
	if err != nil {
		t.Fatal(err)
	}
	index, err := assetref.Load(output)
	if err != nil {
		t.Fatal(err)
	}
	for _, f := range r.Files {
		if f.Path == assetref.Filename {
			continue
		}
		id, err := index.ID(f.Path)
		if err != nil {
			t.Fatal(err)
		}
		e, err := index.Resolve(id)
		if err != nil {
			t.Fatal(err)
		}
		b, err := os.ReadFile(filepath.Join(output, filepath.FromSlash(f.Path)))
		if err != nil {
			t.Fatal(err)
		}
		hash := sha256.Sum256(b)
		if e.SHA256 != hex.EncodeToString(hash[:]) || e.Bytes != int64(len(b)) {
			t.Fatalf("incorrect content digest for %s", f.Path)
		}
	}
	if _, err := index.ID("world/unused.glb"); err == nil {
		t.Fatal("stripped asset indexed")
	}
	if _, err := index.ID(assetref.Filename); err == nil {
		t.Fatal("index indexed itself")
	}
	_, err = pack(source, output, config)
	if err != nil {
		t.Fatal(err)
	}
	other, err := assetref.Load(output)
	if err != nil {
		t.Fatal(err)
	}
	if err := index.CheckPeer(other.PackHash()); err != nil {
		t.Fatal("rebuilding changed pack fingerprint")
	}
}
