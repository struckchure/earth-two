// Command webcompress writes gzip sidecars for browser build artifacts.
// Compression happens at build time, never on the server's request path.
package main

import (
	"compress/gzip"
	"fmt"
	"io"
	"io/fs"
	"log"
	"os"
	"path/filepath"
)

func main() {
	if len(os.Args) != 2 {
		log.Fatal("usage: webcompress <browser-build-directory>")
	}
	var raw, packed int64
	err := filepath.WalkDir(os.Args[1], func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		switch filepath.Ext(path) {
		case ".wasm", ".js", ".data":
		default:
			return nil
		}
		before, after, err := compress(path)
		raw += before
		packed += after
		return err
	})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("browser downloads: %.1f MiB → %.1f MiB with gzip\n", float64(raw)/(1<<20), float64(packed)/(1<<20))
}

func compress(path string) (int64, int64, error) {
	in, err := os.Open(path)
	if err != nil {
		return 0, 0, err
	}
	defer in.Close()
	out, err := os.CreateTemp(filepath.Dir(path), ".gzip-*")
	if err != nil {
		return 0, 0, err
	}
	defer os.Remove(out.Name())
	defer out.Close()
	if err := out.Chmod(0644); err != nil {
		return 0, 0, err
	}
	zip, err := gzip.NewWriterLevel(out, gzip.BestCompression)
	if err != nil {
		return 0, 0, err
	}
	n, err := io.Copy(zip, in)
	if err != nil {
		return 0, 0, err
	}
	if err := zip.Close(); err != nil {
		return 0, 0, err
	}
	info, err := out.Stat()
	if err != nil {
		return 0, 0, err
	}
	if err := out.Close(); err != nil {
		return 0, 0, err
	}
	if err := os.Rename(out.Name(), path+".gz"); err != nil {
		return 0, 0, err
	}
	return n, info.Size(), nil
}
