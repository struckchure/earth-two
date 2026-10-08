// Command webaccount loads the networking bridge before the browser's Go game.
// The bridge adds no HTML controls; the game owns the entire identity interface.
package main

import (
	"bytes"
	"fmt"
	"os"
)

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: webaccount index.html")
		os.Exit(1)
	}
	path := os.Args[1]
	data, err := os.ReadFile(path)
	if err != nil {
		fail(err)
	}
	if bytes.Contains(data, []byte(`<script src="account.js"></script>`)) {
		return
	}
	marker := []byte(`<script src="wasm_exec.js"></script>`)
	if !bytes.Contains(data, marker) {
		fail(fmt.Errorf("%s: Go runtime script was not found", path))
	}
	data = bytes.Replace(data, marker, append([]byte("<script src=\"account.js\"></script>\n\t"), marker...), 1)
	if err := os.WriteFile(path, data, 0644); err != nil {
		fail(err)
	}
}
func fail(err error) { fmt.Fprintln(os.Stderr, err); os.Exit(1) }
