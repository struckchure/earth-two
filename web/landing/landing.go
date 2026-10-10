// Package landing serves Earth Two's HTML and CSS landing page.
package landing

import (
	"bytes"
	"embed"
	"errors"
	"fmt"
	"html/template"
	"io/fs"
	"mime"
	"net/http"
	"os"
	"path"
	"strings"
	"time"
)

//go:embed index.html styles.css assets
var files embed.FS

// Handler embeds the page, fonts and images. The optional video stays on disk
// so the server can stream byte ranges without embedding a large movie.
func Handler(trailer string) (http.Handler, error) {
	page, err := template.ParseFS(files, "index.html")
	if err != nil {
		return nil, err
	}
	available := false
	if trailer != "" {
		info, err := os.Stat(trailer)
		if err != nil && !errors.Is(err, fs.ErrNotExist) {
			return nil, fmt.Errorf("trailer: %w", err)
		}
		available = err == nil && info.Mode().IsRegular()
	}
	var html bytes.Buffer
	if err := page.Execute(&html, struct{ TrailerAvailable bool }{available}); err != nil {
		return nil, err
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /{$}", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache")
		http.ServeContent(w, r, "index.html", time.Time{}, bytes.NewReader(html.Bytes()))
	})
	static := func(w http.ResponseWriter, r *http.Request) {
		name := strings.TrimPrefix(r.URL.Path, "/")
		data, err := files.ReadFile(name)
		if err != nil {
			http.NotFound(w, r)
			return
		}
		ext := path.Ext(name)
		contentType := mime.TypeByExtension(ext)
		// Font mappings depend on the host's MIME database and may be absent
		// on Windows. Our embedded TrueType fonts always use the same type.
		if ext == ".ttf" {
			contentType = "font/ttf"
		}
		// An empty header would suppress ServeContent's content detection.
		if contentType != "" {
			w.Header().Set("Content-Type", contentType)
		}
		w.Header().Set("Cache-Control", "public, max-age=3600")
		if name == "styles.css" {
			w.Header().Set("Cache-Control", "no-cache")
		}
		http.ServeContent(w, r, path.Base(name), time.Time{}, bytes.NewReader(data))
	}
	mux.HandleFunc("GET /styles.css", static)
	mux.HandleFunc("GET /assets/", static)
	mux.HandleFunc("GET /trailer.mp4", func(w http.ResponseWriter, r *http.Request) {
		if !available {
			http.NotFound(w, r)
			return
		}
		f, err := os.Open(trailer)
		if err != nil {
			http.NotFound(w, r)
			return
		}
		defer f.Close()
		info, err := f.Stat()
		if err != nil {
			http.Error(w, "Trailer unavailable", http.StatusServiceUnavailable)
			return
		}
		w.Header().Set("Content-Type", "video/mp4")
		http.ServeContent(w, r, "trailer.mp4", info.ModTime(), f)
	})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Content-Security-Policy", "default-src 'self'; style-src 'self'; font-src 'self'; img-src 'self'; media-src 'self'; script-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
		mux.ServeHTTP(w, r)
	}), nil
}
