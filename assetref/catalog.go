// Package assetref identifies locally installed assets across game clients.
// IDs are stable logical references; content hashes identify a particular pack.
package assetref

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const Filename = "asset-index.json"
const Version = 1

// ID is a 64-bit path-derived identifier, encoded as 16 hex characters in JSON.
// It remains stable when an asset is rebuilt. A pack rejects ID collisions.
// Never send process-local render handles or wardrobe array indices to peers.
type ID string

func ForPath(path string) ID {
	hash := sha256.Sum256([]byte("earth-two:asset:v1:" + path))
	return ID(hex.EncodeToString(hash[:8]))
}

type Entry struct {
	ID     ID     `json:"id"`
	Path   string `json:"path"`
	SHA256 string `json:"sha256"`
	Bytes  int64  `json:"bytes"`
}

type Catalog struct {
	Version  int     `json:"version"`
	PackHash string  `json:"packHash"`
	Assets   []Entry `json:"assets"`
}

// Index resolves received IDs to local paths. Unknown IDs fail rather than
// becoming arbitrary file paths or requests to transfer asset bytes.
type Index struct {
	catalog Catalog
	byID    map[ID]Entry
	byPath  map[string]ID
}

func validHash(s string) bool {
	b, err := hex.DecodeString(s)
	return err == nil && len(b) == sha256.Size && s == strings.ToLower(s)
}

// New builds a deterministic catalogue from the packed files. The pack hash
// covers every entry, including shared textures/buffers and runtime manifests.
// The index and the packer's empty ownership marker are not indexed themselves.
func New(entries []Entry) (Catalog, error) {
	assets := append([]Entry(nil), entries...)
	sort.Slice(assets, func(i, j int) bool { return assets[i].Path < assets[j].Path })
	ids := map[ID]string{}
	for _, e := range assets {
		if !fs.ValidPath(e.Path) || strings.Contains(e.Path, "\\") || e.Path == Filename || e.Path == ".assetpack" {
			return Catalog{}, fmt.Errorf("invalid asset path %q", e.Path)
		}
		if e.ID != ForPath(e.Path) {
			return Catalog{}, fmt.Errorf("incorrect ID for %s", e.Path)
		}
		if previous, ok := ids[e.ID]; ok {
			return Catalog{}, fmt.Errorf("duplicate or colliding asset ID: %s and %s", previous, e.Path)
		}
		if !validHash(e.SHA256) || e.Bytes < 0 {
			return Catalog{}, fmt.Errorf("invalid content digest or size for %s", e.Path)
		}
		ids[e.ID] = e.Path
	}
	b, err := json.Marshal(struct {
		Version int     `json:"version"`
		Assets  []Entry `json:"assets"`
	}{Version, assets})
	if err != nil {
		return Catalog{}, err
	}
	hash := sha256.Sum256(b)
	return Catalog{Version: Version, PackHash: hex.EncodeToString(hash[:]), Assets: assets}, nil
}

func Open(c Catalog) (*Index, error) {
	if c.Version != Version {
		return nil, fmt.Errorf("unsupported asset catalogue version %d", c.Version)
	}
	expected, err := New(c.Assets)
	if err != nil {
		return nil, err
	}
	if c.PackHash != expected.PackHash {
		return nil, fmt.Errorf("asset catalogue fingerprint mismatch")
	}
	index := &Index{catalog: expected, byID: map[ID]Entry{}, byPath: map[string]ID{}}
	for _, e := range expected.Assets {
		index.byID[e.ID] = e
		index.byPath[e.Path] = e.ID
	}
	return index, nil
}

func Load(root string) (*Index, error) {
	b, err := os.ReadFile(filepath.Join(root, Filename))
	if err != nil {
		return nil, err
	}
	var c Catalog
	if err := json.Unmarshal(b, &c); err != nil {
		return nil, err
	}
	return Open(c)
}

func (i *Index) PackHash() string { return i.catalog.PackHash }

// CheckPeer belongs in the session handshake before spawning remote players.
// A mismatch requires a compatible game build; it never transfers assets.
func (i *Index) CheckPeer(packHash string) error {
	if packHash != i.PackHash() {
		return fmt.Errorf("incompatible asset pack")
	}
	return nil
}

func (i *Index) Resolve(id ID) (Entry, error) {
	e, ok := i.byID[id]
	if !ok {
		return Entry{}, fmt.Errorf("unknown asset ID %q", id)
	}
	return e, nil
}

func (i *Index) ID(path string) (ID, error) {
	id, ok := i.byPath[path]
	if !ok {
		return "", fmt.Errorf("asset path %q is not in this pack", path)
	}
	return id, nil
}
