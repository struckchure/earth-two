// Command webaccount installs the game's browser shell after illusion's build.
package main

import (
	"bytes"
	_ "embed"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
)

//go:embed shell.html
var shell []byte

//go:embed pointer.js
var pointer []byte

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
	for _, name := range []string{"raylib.data", "game.wasm"} {
		info, err := os.Stat(filepath.Join(dir, name))
		if err != nil {
			return nil, err
		}
		sizes[name] = info.Size()
	}
	manifest, err := json.Marshal(sizes)
	if err != nil {
		return nil, err
	}
	result := bytes.ReplaceAll(shell, []byte("__MODULES__"), match[1])
	result = bytes.ReplaceAll(result, []byte("__POINTER__"), pointer)
	return bytes.ReplaceAll(result, []byte("__SIZES__"), manifest), nil
}

func fail(err error) { fmt.Fprintln(os.Stderr, err); os.Exit(1) }
