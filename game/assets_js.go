package game

import "os"

// make web stages the shipped asset graph under build/assets. Direct engine
// builds can still preload the source assets directory.
func assetRoot() string {
	if info, err := os.Stat("build/assets"); err == nil && info.IsDir() {
		return "build/assets"
	}
	return "assets"
}
