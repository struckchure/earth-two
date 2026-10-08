// Command assetpack stages the shipped game's asset graph without changing sources.
package main

import (
	"flag"
	"fmt"
	"log"
	"path/filepath"
)

func main() {
	source := flag.String("src", "assets", "source asset directory")
	output := flag.String("out", "build/assets", "packed asset directory (replaced on success)")
	config := flag.String("config", "tools/assetpack/manifest.json", "runtime asset roots")
	report := flag.String("report", "build/asset-report.json", "size and exclusion report, outside the asset pack")
	flag.Parse()
	sourcePath, err := filepath.Abs(*source)
	if err != nil {
		log.Fatal(err)
	}
	outputPath, err := filepath.Abs(*output)
	if err != nil {
		log.Fatal(err)
	}
	reportPath, err := filepath.Abs(*report)
	if err != nil {
		log.Fatal(err)
	}
	if inside(sourcePath, reportPath) || inside(outputPath, reportPath) {
		log.Fatal("report must be outside source and packed asset directories")
	}
	r, err := pack(*source, *output, *config)
	if err != nil {
		log.Fatal(err)
	}
	if err := writeJSON(*report, r); err != nil {
		log.Fatal(err)
	}
	fmt.Printf("assets: %.1f MiB → %.1f MiB (%.1f%% smaller); %d files excluded\nreport: %s\n", float64(r.SourceBytes)/(1<<20), float64(r.PackedBytes)/(1<<20), 100*(1-float64(r.PackedBytes)/float64(r.SourceBytes)), len(r.Excluded), filepath.Clean(*report))
}
