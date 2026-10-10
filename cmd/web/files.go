//go:build !js

package main

import (
	"crypto/sha256"
	"fmt"
	"io"
	"mime"
	"net/http"
	"os"
	"path"
	"strconv"
	"strings"
	"sync"
)

// Serve precompressed build artifacts when the browser accepts gzip. The
// rooted filesystem keeps requests inside dir; FileServer handles everything
// else, including redirects and missing files. The caller must Close the
// handler after it stops serving to release the root directory handle.
type fileHandler struct {
	http.Handler
	root *os.Root
}

func (h *fileHandler) Close() error { return h.root.Close() }

func newFileHandler(dir string) (*fileHandler, error) {
	root, err := os.OpenRoot(dir)
	if err != nil {
		return nil, err
	}
	files := http.FileServer(http.FS(root.FS()))
	// Cache digests per file revision, not per caller-supplied URL.
	var digestMu sync.Mutex
	type revision struct {
		info os.FileInfo
		hash string
	}
	digests := map[string]revision{}
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Revalidate so rebuilding at the same URL cannot leave stale code or
		// assets in the browser. ServeContent provides Last-Modified and 304s.
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Vary", "Accept-Encoding")
		name := strings.TrimPrefix(path.Clean("/"+r.URL.Path), "/")
		ext := path.Ext(name)
		version := r.URL.Query().Get("v")
		if (r.Method == http.MethodGet || r.Method == http.MethodHead) && version != "" && (ext == ".js" || ext == ".wasm" || ext == ".data") {
			file, err := root.Open(name)
			if err != nil {
				http.NotFound(w, r)
				return
			}
			defer file.Close()
			info, err := file.Stat()
			if err != nil || !info.Mode().IsRegular() {
				http.NotFound(w, r)
				return
			}
			digestMu.Lock()
			cached, ok := digests[name]
			if !ok || !os.SameFile(cached.info, info) || cached.info.Size() != info.Size() || !cached.info.ModTime().Equal(info.ModTime()) {
				hash := sha256.New()
				_, err = io.Copy(hash, file)
				if err == nil {
					cached = revision{info, fmt.Sprintf("%x", hash.Sum(nil))}
					digests[name] = cached
				}
			}
			digestMu.Unlock()
			if err != nil {
				http.Error(w, "Could not read build artifact", http.StatusInternalServerError)
				return
			}
			if version != cached.hash {
				// Never cache the current bytes under an old deployment's hash.
				w.Header().Set("Cache-Control", "no-store")
				http.Error(w, "Build changed. Reload the page to get the current version.", http.StatusConflict)
				return
			}
			if _, err := file.Seek(0, io.SeekStart); err != nil {
				http.Error(w, "Could not read build artifact", http.StatusInternalServerError)
				return
			}
			w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
			// Weak because gzip and uncompressed responses encode the same content.
			w.Header().Set("ETag", `W/"`+cached.hash+`"`)
			contentType := mime.TypeByExtension(ext)
			if contentType == "" {
				contentType = "application/octet-stream"
			}
			w.Header().Set("Content-Type", contentType)
			if acceptsGzip(r.Header.Get("Accept-Encoding")) {
				if compressed, err := root.Open(name + ".gz"); err == nil {
					defer compressed.Close()
					if packed, err := compressed.Stat(); err == nil && packed.Mode().IsRegular() && !packed.ModTime().Before(info.ModTime()) {
						w.Header().Set("Content-Encoding", "gzip")
						http.ServeContent(w, r, name, info.ModTime(), compressed)
						return
					}
				}
			}
			http.ServeContent(w, r, name, info.ModTime(), file)
			return
		}
		if (r.Method == http.MethodGet || r.Method == http.MethodHead) && acceptsGzip(r.Header.Get("Accept-Encoding")) && !strings.HasSuffix(r.URL.Path, "/") {
			name := strings.TrimPrefix(path.Clean("/"+r.URL.Path), "/")
			original, err := root.Stat(name)
			if err == nil && original.Mode().IsRegular() {
				compressed, err := root.Open(name + ".gz")
				if err == nil {
					defer compressed.Close()
					info, err := compressed.Stat()
					// A build interrupted before compression must never serve an
					// older sidecar over a newer original.
					if err == nil && info.Mode().IsRegular() && !info.ModTime().Before(original.ModTime()) {
						contentType := mime.TypeByExtension(path.Ext(name))
						if contentType == "" {
							contentType = "application/octet-stream"
						}
						w.Header().Set("Content-Type", contentType)
						w.Header().Set("Content-Encoding", "gzip")
						http.ServeContent(w, r, name, info.ModTime(), compressed)
						return
					}
				}
			}
		}
		files.ServeHTTP(w, r)
	})
	return &fileHandler{Handler: handler, root: root}, nil
}

func acceptsGzip(header string) bool {
	wildcard := false
	for _, item := range strings.Split(header, ",") {
		parts := strings.Split(item, ";")
		coding := strings.TrimSpace(parts[0])
		q := 1.0
		for _, parameter := range parts[1:] {
			key, value, ok := strings.Cut(strings.TrimSpace(parameter), "=")
			if ok && strings.EqualFold(key, "q") {
				parsed, err := strconv.ParseFloat(strings.TrimSpace(value), 64)
				if err != nil || !(parsed >= 0 && parsed <= 1) {
					q = 0
				} else {
					q = parsed
				}
			}
		}
		if strings.EqualFold(coding, "gzip") {
			return q > 0
		}
		if coding == "*" {
			wildcard = q > 0
		}
	}
	return wildcard
}
