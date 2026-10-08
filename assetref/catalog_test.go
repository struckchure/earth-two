package assetref

import (
	"crypto/sha256"
	"encoding/hex"
	"testing"
)

func entry(path, content string) Entry {
	hash := sha256.Sum256([]byte(content))
	return Entry{ID: ForPath(path), Path: path, SHA256: hex.EncodeToString(hash[:]), Bytes: int64(len(content))}
}

func TestIDDerivationContract(t *testing.T) {
	for path, want := range map[string]ID{
		"characters/man.glb":   "4f2067f31e5850c5",
		"characters/woman.glb": "c66dcad0bd759da8",
	} {
		if got := ForPath(path); got != want {
			t.Fatalf("%s: ID %s, want %s", path, got, want)
		}
	}
}

func TestStableIDsAndPackCompatibility(t *testing.T) {
	body := entry("characters/man.glb", "body")
	shirt := entry("characters/man/top/shirt.glb", "shirt")
	c, err := New([]Entry{shirt, body})
	if err != nil {
		t.Fatal(err)
	}
	reordered, err := New([]Entry{body, shirt})
	if err != nil {
		t.Fatal(err)
	}
	if c.PackHash != reordered.PackHash {
		t.Fatal("file ordering changes fingerprint")
	}
	index, err := Open(c)
	if err != nil {
		t.Fatal(err)
	}
	if err := index.CheckPeer(reordered.PackHash); err != nil {
		t.Fatal(err)
	}
	id, err := index.ID(body.Path)
	if err != nil {
		t.Fatal(err)
	}
	local, err := index.Resolve(id)
	if err != nil || local != body {
		t.Fatalf("resolve: %v, %v", local, err)
	}
	changed := entry(body.Path, "new body")
	if changed.ID != body.ID {
		t.Fatal("rebuilding an asset changed its logical ID")
	}
	other, err := New([]Entry{changed, shirt})
	if err != nil {
		t.Fatal(err)
	}
	if err := index.CheckPeer(other.PackHash); err == nil {
		t.Fatal("different pack accepted")
	}
	// Shared dependencies also change the handshake fingerprint.
	shared := entry("_packed/textures/abc.png", "pixels")
	dependencies, _ := New([]Entry{body, shirt, shared})
	if err := index.CheckPeer(dependencies.PackHash); err == nil {
		t.Fatal("different dependencies accepted")
	}
	if _, err := index.Resolve("../../secret"); err == nil {
		t.Fatal("unrecognized reference accepted")
	}
	if _, err := index.ID("characters/missing.glb"); err == nil {
		t.Fatal("missing path accepted")
	}
}

func TestRejectInvalidCatalogues(t *testing.T) {
	e := entry("world/crate.glb", "model")
	for _, entries := range [][]Entry{
		{e, e},
		{{ID: e.ID, Path: "../outside", SHA256: e.SHA256}},
		{{ID: "incorrect", Path: e.Path, SHA256: e.SHA256}},
		{{ID: e.ID, Path: e.Path, SHA256: "invalid"}},
		{{ID: e.ID, Path: e.Path, SHA256: e.SHA256, Bytes: -1}},
	} {
		if _, err := New(entries); err == nil {
			t.Fatalf("invalid entries accepted: %v", entries)
		}
	}
	c, _ := New([]Entry{e})
	c.Assets[0].Bytes++
	if _, err := Open(c); err == nil {
		t.Fatal("modified catalogue accepted")
	}
	c, _ = New([]Entry{e})
	c.Version++
	if _, err := Open(c); err == nil {
		t.Fatal("unknown schema accepted")
	}
}
