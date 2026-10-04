//go:build !js

package game

import (
	"os"
	"path/filepath"
)

// assetRoot is where the game's files are: assets in the working directory,
// as when it's run from the repository, or else next to the program, as in
// a download.
func assetRoot() string {
	if _, err := os.Stat("assets"); err == nil {
		return "assets"
	}
	if exe, err := os.Executable(); err == nil {
		if beside := filepath.Join(filepath.Dir(exe), "assets"); isDir(beside) {
			return beside
		}
	}
	return "assets"
}

func isDir(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}
