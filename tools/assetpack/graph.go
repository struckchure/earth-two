package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

type manifest struct {
	World   string   `json:"world"`
	Layouts []string `json:"layouts"`
	Roots   []string `json:"roots"`
	Keep    []string `json:"keep"`
	Code    []string `json:"code"`
}

type graph struct {
	files     map[string]bool
	documents map[string]any
	pieces    []string
}

func readJSON(path string, v any) error {
	b, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	if err := json.Unmarshal(b, v); err != nil {
		return fmt.Errorf("%s: %w", path, err)
	}
	return nil
}

// Runtime literals conservatively retain named world pieces, direct paths,
// and sound families. Tests and comments are deliberately excluded. Dynamic
// paths not named by runtime literals belong in the manifest's explicit roots.
func codeStrings(roots []string) (map[string]bool, error) {
	values := map[string]bool{}
	for _, root := range roots {
		err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() || filepath.Ext(path) != ".go" || strings.HasSuffix(path, "_test.go") {
				return nil
			}
			f, err := parser.ParseFile(token.NewFileSet(), path, nil, 0)
			if err != nil {
				return err
			}
			ast.Inspect(f, func(n ast.Node) bool {
				if s, ok := n.(*ast.BasicLit); ok && s.Kind == token.STRING {
					value, _ := strconv.Unquote(s.Value)
					values[value] = true
				}
				return true
			})
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	return values, nil
}

func assetPath(root, name string) (string, error) {
	if !fs.ValidPath(name) || strings.Contains(name, "\\") {
		return "", fmt.Errorf("invalid asset path %q", name)
	}
	path := filepath.Join(root, filepath.FromSlash(name))
	info, err := os.Lstat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", fmt.Errorf("asset %s is not a regular file", name)
	}
	// Also reject symlinked parents, so a pack cannot read outside its source.
	for dir := filepath.Dir(path); dir != filepath.Clean(root); dir = filepath.Dir(dir) {
		info, err := os.Lstat(dir)
		if err != nil {
			return "", err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", fmt.Errorf("asset %s has a symlinked parent", name)
		}
	}
	return path, nil
}

func selectAssets(root string, m manifest) (*graph, error) {
	literals, err := codeStrings(m.Code)
	if err != nil {
		return nil, err
	}
	g := &graph{files: map[string]bool{}, documents: map[string]any{}}
	var visit func(string) error
	visit = func(name string) error {
		if g.files[name] {
			return nil
		}
		path, err := assetPath(root, name)
		if err != nil {
			return fmt.Errorf("required asset %s: %w", name, err)
		}
		g.files[name] = true
		if filepath.Ext(name) == ".json" && name != m.World {
			var doc any
			if err := readJSON(path, &doc); err != nil {
				return err
			}
			g.documents[name] = doc
			return pathReferences(doc, visit)
		}
		return nil
	}
	var catalog map[string]any
	worldPath, err := assetPath(root, m.World)
	if err != nil {
		return nil, err
	}
	if err := readJSON(worldPath, &catalog); err != nil {
		return nil, err
	}
	pieces, ok := catalog["pieces"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s: no piece catalogue", m.World)
	}
	used := map[string]any{}
	var keepPiece func(string) error
	keepPiece = func(name string) error {
		if _, ok := used[name]; ok {
			return nil
		}
		piece, ok := pieces[name]
		if !ok {
			return fmt.Errorf("required world piece %q is absent", name)
		}
		used[name] = piece
		// Vehicles refer to wheel pieces, which need not appear in the layout.
		return walk(piece, func(key string, value any) error {
			if key == "piece" {
				if s, ok := value.(string); ok {
					return keepPiece(s)
				}
			}
			return nil
		})
	}
	for _, layout := range m.Layouts {
		if err := visit(layout); err != nil {
			return nil, err
		}
		if err := walk(g.documents[layout], func(key string, v any) error {
			if key == "piece" {
				if s, ok := v.(string); ok {
					return keepPiece(s)
				}
			}
			return nil
		}); err != nil {
			return nil, err
		}
	}
	for name := range pieces {
		if literals[name] {
			if err := keepPiece(name); err != nil {
				return nil, err
			}
		}
	}
	catalog["pieces"] = used
	g.files[m.World] = true
	g.documents[m.World] = catalog
	if err := pathReferences(catalog, visit); err != nil {
		return nil, err
	}
	for _, name := range append(m.Roots, m.Keep...) {
		if err := visit(name); err != nil {
			return nil, err
		}
	}
	for name := range literals {
		if strings.HasPrefix(name, "world/") || strings.HasPrefix(name, "characters/") || strings.HasPrefix(name, "sounds/") {
			if filepath.Ext(name) != "" {
				if err := visit(name); err != nil {
					return nil, err
				}
			}
		}
	}
	err = filepath.WalkDir(filepath.Join(root, "sounds"), func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		ext := filepath.Ext(path)
		if ext != ".wav" && ext != ".ogg" {
			return nil
		}
		stem := strings.TrimSuffix(d.Name(), ext)
		if at := strings.LastIndex(stem, "_"); at >= 0 {
			if _, err := strconv.Atoi(stem[at+1:]); err == nil {
				stem = stem[:at]
			}
		}
		if literals[stem] {
			return visit("sounds/" + d.Name())
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	for name := range used {
		g.pieces = append(g.pieces, name)
	}
	sort.Strings(g.pieces)
	return g, nil
}

func pathReferences(doc any, visit func(string) error) error {
	return walk(doc, func(key string, value any) error {
		if key == "model" || key == "path" {
			if s, ok := value.(string); ok {
				return visit(s)
			}
		}
		return nil
	})
}

func walk(doc any, fn func(string, any) error) error {
	switch v := doc.(type) {
	case map[string]any:
		// Stable traversal also makes reports and generated output reproducible.
		keys := make([]string, 0, len(v))
		for key := range v {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			if err := fn(key, v[key]); err != nil {
				return err
			}
			if err := walk(v[key], fn); err != nil {
				return err
			}
		}
	case []any:
		for _, item := range v {
			if err := walk(item, fn); err != nil {
				return err
			}
		}
	}
	return nil
}
