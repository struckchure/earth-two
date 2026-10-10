package landing

import (
	"bytes"
	"encoding/xml"
	"image/jpeg"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestPublicSearchAndSharingMetadata(t *testing.T) {
	h, err := HandlerWithSite("", "", DefaultSiteURL)
	if err != nil {
		t.Fatal(err)
	}
	page := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "https://untrusted.example/?campaign=test", nil)
	request.Header.Set("X-Forwarded-Host", "untrusted.example")
	h.ServeHTTP(page, request)
	if page.Code != 200 {
		t.Fatal(page.Code, page.Body.String())
	}
	for _, want := range []string{
		`<title>` + pageTitle + `</title>`,
		`<link rel="canonical" href="https://earthtwo.world/">`,
		`<meta property="og:url" content="https://earthtwo.world/">`,
		`<meta property="og:image" content="https://earthtwo.world/assets/landfall.jpg">`,
		`<meta property="og:image:alt"`,
		`<meta name="twitter:card" content="summary_large_image">`,
		`<meta name="twitter:image" content="https://earthtwo.world/assets/landfall.jpg">`,
		`itemscope itemtype="https://schema.org/VideoGame"`,
	} {
		if !strings.Contains(page.Body.String(), want) {
			t.Errorf("missing %s", want)
		}
	}
	if strings.Contains(page.Body.String(), "untrusted.example") || strings.Contains(page.Body.String(), "noindex") || strings.Contains(page.Body.String(), "ZgotmplZ") {
		t.Fatal("invalid public metadata")
	}
	robots := httptest.NewRecorder()
	h.ServeHTTP(robots, httptest.NewRequest(http.MethodGet, "/robots.txt", nil))
	if robots.Code != 200 || !strings.Contains(robots.Body.String(), "Sitemap: https://earthtwo.world/sitemap.xml") {
		t.Fatal(robots.Body.String())
	}
	sitemap := httptest.NewRecorder()
	h.ServeHTTP(sitemap, httptest.NewRequest(http.MethodGet, "/sitemap.xml", nil))
	var parsed struct {
		URLs []struct {
			Location string `xml:"loc"`
		} `xml:"url"`
	}
	if err := xml.Unmarshal(sitemap.Body.Bytes(), &parsed); err != nil {
		t.Fatal(err)
	}
	if len(parsed.URLs) != 1 || parsed.URLs[0].Location != "https://earthtwo.world/" {
		t.Fatal(parsed)
	}
	img := httptest.NewRecorder()
	h.ServeHTTP(img, httptest.NewRequest(http.MethodGet, "/assets/landfall.jpg", nil))
	cfg, err := jpeg.DecodeConfig(bytes.NewReader(img.Body.Bytes()))
	if err != nil || img.Code != 200 || cfg.Width != 1600 || cfg.Height != 900 {
		t.Fatalf("share image: %v %+v", err, cfg)
	}
	for _, path := range []string{"/robots.txt", "/sitemap.xml", "/assets/landfall.jpg"} {
		head := httptest.NewRecorder()
		h.ServeHTTP(head, httptest.NewRequest(http.MethodHead, path, nil))
		if head.Code != 200 || head.Body.Len() != 0 {
			t.Errorf("HEAD %s: %d", path, head.Code)
		}
	}
}

func TestLocalSearchMetadataAndInvalidOrigins(t *testing.T) {
	h, err := Handler("")
	if err != nil {
		t.Fatal(err)
	}
	page := httptest.NewRecorder()
	h.ServeHTTP(page, httptest.NewRequest(http.MethodGet, "/", nil))
	if page.Header().Get("X-Robots-Tag") != "noindex, nofollow" || strings.Contains(page.Body.String(), `rel="canonical"`) {
		t.Fatal("local preview should not advertise a canonical host")
	}
	for _, raw := range []string{"http://example.org", "https://example.org/subpage", "https://user:password@example.org", "https://example.org/?q=1", "https://example.org/#test"} {
		if _, err := HandlerWithSite("", "", raw); err == nil {
			t.Errorf("accepted invalid origin %q", raw)
		}
	}
}
