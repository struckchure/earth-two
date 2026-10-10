package landing

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"time"
)

const downloadOrigin = "https://agreed-orange-partridge-23por.sevalla.storage/earth-two"
const DefaultDownloadsURL = downloadOrigin + "/installers/latest.json"

var installerTargets = map[string]string{
	"windows-amd64": "windows-amd64-setup.exe",
	"macos-arm64":   "macos-arm64.dmg",
	"macos-amd64":   "macos-amd64.dmg",
	"linux-amd64":   "linux-amd64.deb",
}
var installerName = regexp.MustCompile(`^earth-two-(\d+\.\d+\.\d+)-(.+)$`)
var releaseID = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._-]*$`)
var downloadHash = regexp.MustCompile(`^[a-f0-9]{64}$`)

func fallbackDownloads() map[string]string {
	links := make(map[string]string)
	for target, suffix := range installerTargets {
		links[target] = downloadOrigin + "/releases/installers-37821694441/installers/earth-two-0.1.0-" + suffix
	}
	return links
}

type downloadFeed struct {
	mu     sync.Mutex
	url    string
	base   string
	client *http.Client
	next   time.Time
	links  map[string]string
}

func newDownloadFeed(raw string) (*downloadFeed, error) {
	f := &downloadFeed{links: fallbackDownloads(), client: &http.Client{Timeout: 3 * time.Second,
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}}
	if raw == "" {
		return f, nil
	}
	u, err := url.Parse(raw)
	if err != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || !strings.HasSuffix(u.Path, "/installers/latest.json") {
		return nil, fmt.Errorf("downloads manifest must be an HTTPS URL ending in /installers/latest.json")
	}
	f.url, f.base = raw, strings.TrimSuffix(raw, "/installers/latest.json")
	return f, nil
}

// Refresh at most once a minute; a failed refresh preserves the last good set.
// Maps are replaced as a whole and never modified after publication.
func (f *downloadFeed) current() map[string]string {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.url == "" || time.Now().Before(f.next) {
		return f.links
	}
	links, err := f.fetch()
	f.next = time.Now().Add(time.Minute)
	if err != nil {
		log.Printf("installer manifest unavailable; keeping previous links: %v", err)
	} else {
		f.links = links
	}
	return f.links
}

func (f *downloadFeed) fetch() (map[string]string, error) {
	response, err := f.client.Get(f.url)
	if err != nil {
		return nil, fmt.Errorf("manifest request failed")
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("manifest HTTP %d", response.StatusCode)
	}
	const limit = 64 * 1024
	data, err := io.ReadAll(io.LimitReader(response.Body, limit+1))
	if err != nil || len(data) > limit {
		return nil, fmt.Errorf("manifest unreadable or too large")
	}
	var manifest struct {
		ReleaseID string `json:"release_id"`
		Downloads []struct {
			Path   string `json:"path"`
			URL    string `json:"url"`
			Size   int64  `json:"size"`
			SHA256 string `json:"sha256"`
		} `json:"downloads"`
	}
	if json.Unmarshal(data, &manifest) != nil || !releaseID.MatchString(manifest.ReleaseID) {
		return nil, fmt.Errorf("invalid installer manifest")
	}
	entries := make(map[string]string)
	for _, item := range manifest.Downloads {
		if !installerName.MatchString(item.Path) || strings.ContainsAny(item.Path, "/\\") || item.Size <= 0 || !downloadHash.MatchString(item.SHA256) || item.URL != f.base+"/releases/"+manifest.ReleaseID+"/installers/"+item.Path {
			return nil, fmt.Errorf("invalid installer entry")
		}
		if _, exists := entries[item.Path]; exists {
			return nil, fmt.Errorf("duplicate installer entry")
		}
		entries[item.Path] = item.URL
	}
	links := make(map[string]string)
	version := ""
	for name, link := range entries {
		match := installerName.FindStringSubmatch(name)
		for target, suffix := range installerTargets {
			if match[2] != suffix {
				continue
			}
			if entries[name+".sha256"] != link+".sha256" || links[target] != "" || version != "" && version != match[1] {
				return nil, fmt.Errorf("incomplete or mixed installer versions")
			}
			version = match[1]
			links[target] = link
		}
	}
	if len(links) != len(installerTargets) {
		return nil, fmt.Errorf("missing installer targets")
	}
	return links, nil
}
