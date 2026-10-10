// Command landing serves the game website without a browser-game build.
package main

import (
	"flag"
	"log"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/struckchure/earth-two/web/landing"
)

func main() {
	addr := ":8080"
	if port := os.Getenv("PORT"); port != "" {
		addr = ":" + port
	}
	listen := flag.String("addr", addr, "address to listen on")
	trailer := flag.String("trailer", "out/trailer/v2/earth-two-trailer.mp4", "optional trailer MP4 path")
	manifest := landing.DefaultDownloadsURL
	if value, ok := os.LookupEnv("INSTALLER_MANIFEST_URL"); ok {
		manifest = value
	}
	downloads := flag.String("downloads", manifest, "HTTPS latest-installer manifest URL (empty disables refresh)")
	flag.Parse()
	handler, err := landing.HandlerWithDownloads(*trailer, *downloads)
	if err != nil {
		log.Fatal(err)
	}
	server := &http.Server{Addr: *listen, Handler: handler, ReadHeaderTimeout: 5 * time.Second, IdleTimeout: 60 * time.Second}
	previewAddr := *listen
	if strings.HasPrefix(previewAddr, ":") {
		previewAddr = "localhost" + previewAddr
	}
	log.Printf("Earth Two landing page: http://%s", previewAddr)
	log.Fatal(server.ListenAndServe())
}
