//go:build !js

package main

import (
	"mime"
	"net/http"
	"os"
	"path"
	"strconv"
	"strings"
)

// Serve precompressed build artifacts when the browser accepts gzip. The
// rooted filesystem keeps requests inside dir; FileServer handles everything
// else, including redirects and missing files.
func newFileHandler(dir string) (http.Handler, error) {
	root, err := os.OpenRoot(dir)
	if err != nil {
		return nil, err
	}
	files := http.FileServer(http.FS(root.FS()))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Revalidate so rebuilding at the same URL cannot leave stale code or
		// assets in the browser. ServeContent provides Last-Modified and 304s.
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Vary", "Accept-Encoding")
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
	}), nil
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
