package main

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/struckchure/earth-two/assetref"
)

type fileSize struct {
	Path  string `json:"path"`
	Bytes int64  `json:"bytes"`
}

type report struct {
	SourceBytes   int64      `json:"sourceBytes"`
	SelectedBytes int64      `json:"selectedBytes"`
	PackedBytes   int64      `json:"packedBytes"`
	Pieces        []string   `json:"pieces"`
	Included      []string   `json:"included"`
	Excluded      []fileSize `json:"excluded"`
	Files         []fileSize `json:"files"`
}

func writeFile(path string, b []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	return os.WriteFile(path, b, 0644)
}

func writeJSON(path string, v any) error {
	b, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return err
	}
	return writeFile(path, append(b, '\n'))
}

func inside(parent, child string) bool {
	rel, err := filepath.Rel(parent, child)
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

func pack(source, output, config string) (*report, error) {
	source, err := filepath.Abs(source)
	if err != nil {
		return nil, err
	}
	output, err = filepath.Abs(output)
	if err != nil {
		return nil, err
	}
	if inside(source, output) || inside(output, source) {
		return nil, fmt.Errorf("source and output directories must not overlap")
	}
	// Only replace directories created by this tool, never an arbitrary folder.
	const marker = ".assetpack"
	if info, err := os.Lstat(output); err == nil {
		if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("output must be a regular directory")
		}
		if _, err := os.Stat(filepath.Join(output, marker)); err != nil {
			return nil, fmt.Errorf("refusing to replace %s without %s marker", output, marker)
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	var m manifest
	if err := readJSON(config, &m); err != nil {
		return nil, err
	}
	g, err := selectAssets(source, m)
	if err != nil {
		return nil, err
	}
	r := &report{Pieces: g.pieces}
	if err := filepath.WalkDir(source, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("non-regular source asset: %s", path)
		}
		rel, _ := filepath.Rel(source, path)
		rel = filepath.ToSlash(rel)
		r.SourceBytes += info.Size()
		if g.files[rel] {
			if strings.HasPrefix(rel, "_packed/") || rel == marker || rel == assetref.Filename {
				return fmt.Errorf("reserved asset path: %s", rel)
			}
			r.SelectedBytes += info.Size()
			r.Included = append(r.Included, rel)
		} else {
			r.Excluded = append(r.Excluded, fileSize{rel, info.Size()})
		}
		return nil
	}); err != nil {
		return nil, err
	}
	sort.Strings(r.Included)
	if err := os.MkdirAll(filepath.Dir(output), 0755); err != nil {
		return nil, err
	}
	stage, err := os.MkdirTemp(filepath.Dir(output), ".assetpack-*")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(stage)
	p := newPacker(stage)
	// Count duplicate payloads first, then externalize only those shared across
	// models. Small views stay embedded to avoid a forest of tiny files.
	for _, name := range r.Included {
		if filepath.Ext(name) == ".glb" {
			if err := p.count(filepath.Join(source, filepath.FromSlash(name))); err != nil {
				return nil, fmt.Errorf("%s: %w", name, err)
			}
		}
	}
	for _, name := range r.Included {
		b, err := os.ReadFile(filepath.Join(source, filepath.FromSlash(name)))
		if err != nil {
			return nil, err
		}
		if doc, ok := g.documents[name]; ok {
			b, err = json.Marshal(doc)
		} else if filepath.Ext(name) == ".glb" {
			b, err = p.model(name, b)
		} else if filepath.Ext(name) == ".png" {
			b, err = optimizePNG(b)
		}
		if err != nil {
			return nil, fmt.Errorf("%s: %w", name, err)
		}
		if err := writeFile(filepath.Join(stage, filepath.FromSlash(name)), b); err != nil {
			return nil, err
		}
	}
	if err := writeCatalog(stage); err != nil {
		return nil, err
	}
	if err := filepath.WalkDir(stage, func(path string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return err
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		rel, _ := filepath.Rel(stage, path)
		r.PackedBytes += info.Size()
		r.Files = append(r.Files, fileSize{filepath.ToSlash(rel), info.Size()})
		return nil
	}); err != nil {
		return nil, err
	}
	if err := writeFile(filepath.Join(stage, marker), nil); err != nil {
		return nil, err
	}
	// Retain the previous pack until the fully built replacement is ready.
	backup := stage + "-old"
	if _, err := os.Stat(output); err == nil {
		if err := os.Rename(output, backup); err != nil {
			return nil, err
		}
	}
	if err := os.Rename(stage, output); err != nil {
		_ = os.Rename(backup, output)
		return nil, err
	}
	if err := os.RemoveAll(backup); err != nil {
		return nil, err
	}
	return r, nil
}
