//go:build !js

package game

import (
	"os"
	"path/filepath"
	"testing"
)

func TestInstalledAssetLocations(t *testing.T) {
	for _, test := range []struct{ name, executable, assets string }{
		{"portable", "earth-two", "assets"},
		{"windows", "earth-two.exe", "assets"},
		{"macOS", "Earth Two.app/Contents/MacOS/earth-two", "Earth Two.app/Contents/Resources/assets"},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			want := filepath.Join(root, filepath.FromSlash(test.assets))
			if err := os.MkdirAll(want, 0755); err != nil {
				t.Fatal(err)
			}
			if got := assetsForExecutable(filepath.Join(root, filepath.FromSlash(test.executable))); got != want {
				t.Fatalf("assets = %q, want %q", got, want)
			}
		})
	}
	if got := assetsForExecutable(filepath.Join(t.TempDir(), "earth-two")); got != "assets" {
		t.Fatalf("development assets = %q, want assets", got)
	}
}
