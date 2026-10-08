package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io/fs"
	"os"
	"path/filepath"

	"github.com/struckchure/earth-two/assetref"
)

func writeCatalog(root string) error {
	var entries []assetref.Entry
	err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return err
		}
		b, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		hash := sha256.Sum256(b)
		entries = append(entries, assetref.Entry{ID: assetref.ForPath(relative), Path: relative, SHA256: hex.EncodeToString(hash[:]), Bytes: int64(len(b))})
		return nil
	})
	if err != nil {
		return err
	}
	catalog, err := assetref.New(entries)
	if err != nil {
		return err
	}
	b, err := json.Marshal(catalog)
	if err != nil {
		return err
	}
	return writeFile(filepath.Join(root, assetref.Filename), b)
}
