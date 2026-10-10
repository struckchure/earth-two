package landing

import (
	"fmt"
	"net/url"
	"strings"
)

const DefaultSiteURL = "https://earthtwo.world"

const pageTitle = "Earth Two — Free-Roam Sci-Fi Role-Playing Game"
const pageDescription = "Explore an abandoned colony in Earth Two, a free-roam sci-fi role-playing game. Take contracts, drive vehicles, and make a life on the Red."

type pageSEO struct {
	Title       string
	Description string
	Canonical   string
	Image       string
	ImageAlt    string
}

func newPageSEO(siteURL string) (pageSEO, error) {
	seo := pageSEO{Title: pageTitle, Description: pageDescription,
		ImageAlt: "Landfall's patched spaceship hull and dome frame beneath the Red's violet evening sky in Earth Two"}
	if siteURL == "" {
		return seo, nil
	}
	u, err := url.Parse(siteURL)
	if err != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || (u.Path != "" && u.Path != "/") {
		return seo, fmt.Errorf("site URL must be an HTTPS origin without a path, query, or credentials")
	}
	seo.Canonical = strings.TrimRight(siteURL, "/") + "/"
	seo.Image = seo.Canonical + "assets/landfall.jpg"
	return seo, nil
}
