// deps prepares private, source-patched dependencies for native Make builds.
// It never changes the shared Go module cache or the project's go.mod.
package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
)

type patch struct {
	module, file, old, fixed string
}

var patches = []patch{
	{
		module: "github.com/gen2brain/raylib-go/raylib",
		file:   "external/stb_vorbis.c",
		old:    "if (f->stream_start + loc >= f->stream_end || f->stream_start + loc < f->stream_start) {",
		fixed:  "if (loc >= (unsigned int) (f->stream_end - f->stream_start)) {",
	},
	{
		module: "github.com/struckchure/illusion",
		file:   "internal/jolt/jolt.go",
		old:    "#cgo darwin LDFLAGS: -lc++\n",
		fixed:  "// Go uses the C++ linker on macOS, which already supplies libc++.\n",
	},
}

func main() {
	if err := prepare(); err != nil {
		fmt.Fprintln(os.Stderr, "prepare dependencies:", err)
		os.Exit(1)
	}
}

func prepare() error {
	root, err := os.Getwd()
	if err != nil {
		return err
	}
	dir := filepath.Join(root, "build", "deps")
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	work := "go " + strings.TrimPrefix(runtime.Version(), "go") + "\n\nuse " + strconv.Quote(root) + "\n\n"
	for _, p := range patches {
		src, err := moduleDir(p.module)
		if err != nil {
			return err
		}
		dst, err := patchedCopy(dir, src, p)
		if err != nil {
			return err
		}
		work += "replace " + p.module + " => " + strconv.Quote(dst) + "\n"
	}
	path := filepath.Join(dir, "native.work")
	if old, err := os.ReadFile(path); err == nil && string(old) == work {
		return nil
	}
	return os.WriteFile(path, []byte(work), 0644)
}

func moduleDir(module string) (string, error) {
	var info struct{ Dir string }
	cmd := exec.Command("go", "list", "-m", "-json", module)
	cmd.Stderr = os.Stderr
	data, err := cmd.Output()
	if err != nil {
		return "", err
	}
	if err := json.Unmarshal(data, &info); err != nil {
		return "", err
	}
	if info.Dir == "" {
		cmd := exec.Command("go", "mod", "download", module)
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			return "", err
		}
		return moduleDir(module)
	}
	return info.Dir, nil
}

func apply(data []byte, p patch) ([]byte, error) {
	if bytes.Count(data, []byte(p.old)) == 1 {
		return bytes.Replace(data, []byte(p.old), []byte(p.fixed), 1), nil
	}
	if bytes.Count(data, []byte(p.fixed)) == 1 && !bytes.Contains(data, []byte(p.old)) {
		return data, nil // The local checkout already contains this fix.
	}
	return nil, fmt.Errorf("%s: %s changed upstream; review the source patch", p.module, p.file)
}

func patchedCopy(dir, src string, p patch) (string, error) {
	data, err := os.ReadFile(filepath.Join(src, p.file))
	if err != nil {
		return "", err
	}
	fixed, err := apply(data, p)
	if err != nil {
		return "", err
	}
	// Hash all source files so a changed local checkout also refreshes the copy.
	hash := sha256.New()
	hash.Write(fixed)
	err = walkSource(src, func(path, rel string, entry fs.DirEntry) error {
		fmt.Fprintln(hash, rel)
		if entry.IsDir() {
			return nil
		}
		data, err := os.ReadFile(path)
		if err == nil {
			hash.Write(data)
		}
		return err
	})
	if err != nil {
		return "", err
	}
	dst := filepath.Join(dir, fmt.Sprintf("%s-%x", filepath.Base(p.module), hash.Sum(nil)[:12]))
	if _, err := os.Stat(dst); err == nil {
		return dst, nil
	}
	stage, err := os.MkdirTemp(dir, ".patch-")
	if err != nil {
		return "", err
	}
	defer os.RemoveAll(stage)
	err = walkSource(src, func(path, rel string, entry fs.DirEntry) error {
		target := filepath.Join(stage, rel)
		if entry.IsDir() {
			return os.MkdirAll(target, 0755)
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		in, err := os.Open(path)
		if err != nil {
			return err
		}
		defer in.Close()
		out, err := os.OpenFile(target, os.O_CREATE|os.O_WRONLY, info.Mode().Perm()|0200)
		if err != nil {
			return err
		}
		_, err = io.Copy(out, in)
		closeErr := out.Close()
		if err != nil {
			return err
		}
		return closeErr
	})
	if err != nil {
		return "", err
	}
	if err := os.WriteFile(filepath.Join(stage, p.file), fixed, 0644); err != nil {
		return "", err
	}
	if err := os.Rename(stage, dst); err != nil {
		return "", err
	}
	return dst, nil
}

func walkSource(src string, visit func(string, string, fs.DirEntry) error) error {
	return filepath.WalkDir(src, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		if entry.IsDir() && (entry.Name() == ".git" || entry.Name() == "build") {
			return filepath.SkipDir
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("unsupported source symlink: %s", path)
		}
		return visit(path, rel, entry)
	})
}
