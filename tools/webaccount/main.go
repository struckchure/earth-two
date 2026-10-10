// Command webaccount installs the game's browser shell after illusion's build.
package main

import (
	"bytes"
	"crypto/sha256"
	_ "embed"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
)

//go:embed shell.html
var shell []byte

//go:embed pointer.js
var pointer []byte

//go:embed touch.js
var touch []byte

//go:embed service-worker.js
var serviceWorker []byte

//go:embed cache.js
var cacheScript []byte

func main() {
	if len(os.Args) != 2 {
		fail(fmt.Errorf("usage: webaccount index.html"))
	}
	path := os.Args[1]
	data, err := os.ReadFile(path)
	if err != nil {
		fail(err)
	}
	data, err = buildShell(data, filepath.Dir(path))
	if err != nil {
		fail(err)
	}
	if err := os.WriteFile(filepath.Join(filepath.Dir(path), "service-worker.js"), serviceWorker, 0644); err != nil {
		fail(err)
	}
	if err := os.WriteFile(path, data, 0644); err != nil {
		fail(err)
	}
}

func buildShell(source []byte, dir string) ([]byte, error) {
	match := regexp.MustCompile(`const modules = (\[[^;]+\]);`).FindSubmatch(source)
	if match == nil {
		return nil, fmt.Errorf("browser module list was not found")
	}
	var modules []string
	if err := json.Unmarshal(match[1], &modules); err != nil {
		return nil, err
	}
	// Use uncompressed sizes: fetch streams contain decoded bytes even when
	// the server sends gzip, whose Content-Length describes compressed bytes.
	sizes := map[string]int64{}
	versions := map[string]string{}
	names := []string{"raylib.data", "game.wasm", "account.js", "wasm_exec.js", "fs.js"}
	for _, module := range modules {
		names = append(names, module+".js", module+".wasm")
	}
	for _, name := range names {
		file, err := os.Open(filepath.Join(dir, name))
		if err != nil {
			return nil, err
		}
		hash := sha256.New()
		size, err := io.Copy(hash, file)
		file.Close()
		if err != nil {
			return nil, err
		}
		versions[name] = fmt.Sprintf("%x", hash.Sum(nil))
		if name == "raylib.data" || name == "game.wasm" {
			sizes[name] = size
		}
	}
	manifest, err := json.Marshal(sizes)
	if err != nil {
		return nil, err
	}
	result := bytes.ReplaceAll(shell, []byte("__MODULES__"), match[1])
	result = bytes.ReplaceAll(result, []byte("__POINTER__"), pointer)
	result = bytes.ReplaceAll(result, []byte("__TOUCH__"), touch)
	result = bytes.ReplaceAll(result, []byte("__CACHE__"), cacheScript)
	versionJSON, err := json.Marshal(versions)
	if err != nil {
		return nil, err
	}
	result = bytes.ReplaceAll(result, []byte("__VERSIONS__"), versionJSON)
	return bytes.ReplaceAll(result, []byte("__SIZES__"), manifest), nil
}

func fail(err error) { fmt.Fprintln(os.Stderr, err); os.Exit(1) }
