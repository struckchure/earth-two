//go:build !js

package game

import (
	"os"
	"path/filepath"
)

// Installed assets take precedence over the working directory. Finder and
// desktop launchers need not start the game in its installation directory.
func assetRoot() string {
	exe, _ := os.Executable()
	// Linux's /usr/bin launcher is a symlink into /opt/earth-two.
	if resolved, err := filepath.EvalSymlinks(exe); err == nil {
		exe = resolved
	}
	return assetsForExecutable(exe)
}

func assetsForExecutable(exe string) string {
	if exe != "" {
		dir := filepath.Dir(exe)
		if beside := filepath.Join(dir, "assets"); isDir(beside) {
			return beside
		}
		if filepath.Base(dir) == "MacOS" && filepath.Base(filepath.Dir(dir)) == "Contents" {
			if resources := filepath.Join(dir, "..", "Resources", "assets"); isDir(resources) {
				return filepath.Clean(resources)
			}
		}
	}
	// go run's temporary executable has no assets; use the source checkout.
	return "assets"
}

func isDir(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}
