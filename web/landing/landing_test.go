package landing

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPageAndEmbeddedAssets(t *testing.T) {
	h, err := Handler("")
	if err != nil {
		t.Fatal(err)
	}
	for _, tt := range []struct {
		url, contentType string
	}{
		{"/", "text/html"},
		{"/styles.css", "text/css"},
		{"/assets/landfall.jpg", "image/jpeg"},
		{"/assets/Inter-Black.ttf", "font/ttf"},
		{"/assets/favicon.svg", "image/svg+xml"},
	} {
		w := httptest.NewRecorder()
		h.ServeHTTP(w, httptest.NewRequest(http.MethodGet, tt.url, nil))
		if w.Code != http.StatusOK || !strings.HasPrefix(w.Header().Get("Content-Type"), tt.contentType) || w.Body.Len() == 0 {
			t.Errorf("%s: status %d, content type %q, bytes %d", tt.url, w.Code, w.Header().Get("Content-Type"), w.Body.Len())
		}
		if tt.url == "/" && (strings.Contains(w.Body.String(), "<video") || strings.Contains(w.Body.String(), "<script")) {
			t.Fatal("page without a trailer must have a fallback and no scripts")
		}
	}
	for _, url := range []string{"/assets/", "/assets/missing.jpg", "/trailer.mp4", "/README.md", "/.env", "/index.html"} {
		w := httptest.NewRecorder()
		h.ServeHTTP(w, httptest.NewRequest(http.MethodGet, url, nil))
		if w.Code != http.StatusNotFound {
			t.Errorf("%s: got %d, want 404", url, w.Code)
		}
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, httptest.NewRequest(http.MethodHead, "/", nil))
	if w.Code != http.StatusOK || w.Body.Len() != 0 {
		t.Fatalf("HEAD: status %d, bytes %d", w.Code, w.Body.Len())
	}
}

func TestTrailerByteRanges(t *testing.T) {
	file := filepath.Join(t.TempDir(), "trailer.mp4")
	if err := os.WriteFile(file, []byte("0123456789"), 0600); err != nil {
		t.Fatal(err)
	}
	h, err := Handler(file)
	if err != nil {
		t.Fatal(err)
	}
	page := httptest.NewRecorder()
	h.ServeHTTP(page, httptest.NewRequest(http.MethodGet, "/", nil))
	if !strings.Contains(page.Body.String(), `<source src="/trailer.mp4"`) {
		t.Fatal("available trailer is missing from page")
	}
	r := httptest.NewRequest(http.MethodGet, "/trailer.mp4", nil)
	r.Header.Set("Range", "bytes=2-5")
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusPartialContent || w.Body.String() != "2345" || w.Header().Get("Content-Range") != "bytes 2-5/10" {
		t.Fatalf("range response: status %d, headers %v, body %q", w.Code, w.Header(), w.Body.String())
	}
}

func TestMissingTrailerFallsBack(t *testing.T) {
	h, err := Handler(filepath.Join(t.TempDir(), "not-created.mp4"))
	if err != nil {
		t.Fatal(err)
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, httptest.NewRequest(http.MethodGet, "/", nil))
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "The next dispatch is on its way") {
		t.Fatal("missing video must leave a working landing page")
	}
}
