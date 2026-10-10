package landing

import (
	"bytes"
	"encoding/json"
	"html/template"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func feedManifest(base, id, version string) map[string]any {
	var entries []map[string]any
	for _, suffix := range installerTargets {
		for _, checksum := range []string{"", ".sha256"} {
			name := "earth-two-" + version + "-" + suffix + checksum
			entries = append(entries, map[string]any{"path": name, "url": base + "/releases/" + id + "/installers/" + name, "size": 123, "sha256": strings.Repeat("a", 64)})
		}
	}
	return map[string]any{"release_id": id, "downloads": entries}
}

func TestDownloadFeedRefreshAndFallback(t *testing.T) {
	var manifest map[string]any
	requests := 0
	status := http.StatusOK
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests++
		w.WriteHeader(status)
		json.NewEncoder(w).Encode(manifest)
	}))
	defer server.Close()
	manifest = feedManifest(server.URL, "first", "0.1.0")
	feed, err := newDownloadFeed(server.URL + "/installers/latest.json")
	if err != nil {
		t.Fatal(err)
	}
	feed.client = server.Client()
	first := feed.current()
	if !strings.Contains(first["macos-arm64"], "/first/") {
		t.Fatal(first)
	}
	feed.current()
	if requests != 1 {
		t.Fatalf("cache made %d requests", requests)
	}
	manifest = feedManifest(server.URL, "second", "0.2.0")
	feed.next = time.Time{}
	latest := feed.current()
	if !strings.Contains(latest["windows-amd64"], "/second/") {
		t.Fatal(latest)
	}
	// All eight actual HTML links change as a set, including checksums.
	page := template.Must(template.ParseFS(files, "index.html"))
	var html bytes.Buffer
	if err := page.Execute(&html, struct {
		TrailerAvailable bool
		Downloads        map[string]string
		SEO              pageSEO
	}{false, latest, pageSEO{}}); err != nil {
		t.Fatal(err)
	}
	for _, link := range latest {
		if !strings.Contains(html.String(), `href="`+link+`"`) || !strings.Contains(html.String(), `href="`+link+`.sha256"`) {
			t.Fatalf("missing installer/checksum pair: %s", link)
		}
	}
	status = http.StatusServiceUnavailable
	feed.next = time.Time{}
	if feed.current()["windows-amd64"] != latest["windows-amd64"] {
		t.Fatal("failure discarded last good links")
	}
	fresh, _ := newDownloadFeed(server.URL + "/installers/latest.json")
	fresh.client = server.Client()
	if fresh.current()["windows-amd64"] != fallbackDownloads()["windows-amd64"] {
		t.Fatal("first failure must use bundled fallback")
	}
}

func TestRejectIncompleteOrUnsafeDownloadFeed(t *testing.T) {
	var payload []byte
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write(payload) }))
	defer server.Close()
	feed, _ := newDownloadFeed(server.URL + "/installers/latest.json")
	feed.client = server.Client()
	for _, kind := range []string{"missing checksum", "missing platform", "foreign URL", "duplicate", "mixed version", "bad hash", "invalid JSON", "oversized"} {
		t.Run(kind, func(t *testing.T) {
			m := feedManifest(server.URL, "good", "0.1.0")
			entries := m["downloads"].([]map[string]any)
			switch kind {
			case "missing checksum":
				entries = entries[:len(entries)-1]
			case "missing platform":
				entries = entries[:len(entries)-2]
			case "foreign URL":
				entries[0]["url"] = "https://other.example/installer.exe"
			case "duplicate":
				entries = append(entries, entries[0])
			case "mixed version":
				for _, e := range entries[:2] {
					e["path"] = strings.ReplaceAll(e["path"].(string), "0.1.0", "0.2.0")
					e["url"] = strings.ReplaceAll(e["url"].(string), "0.1.0", "0.2.0")
				}
			case "bad hash":
				entries[0]["sha256"] = "invalid"
			}
			m["downloads"] = entries
			payload, _ = json.Marshal(m)
			if kind == "invalid JSON" {
				payload = []byte("{")
			}
			if kind == "oversized" {
				payload = bytes.Repeat([]byte("x"), 65537)
			}
			if _, err := feed.fetch(); err == nil {
				t.Fatal("accepted invalid manifest")
			}
		})
	}
}

func TestDownloadManifestURL(t *testing.T) {
	for _, raw := range []string{"http://example.org/installers/latest.json", "https://user:password@example.org/installers/latest.json", "https://example.org/other.json"} {
		if _, err := newDownloadFeed(raw); err == nil {
			t.Fatalf("accepted %s", raw)
		}
	}
}
