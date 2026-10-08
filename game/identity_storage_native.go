//go:build !js

package game

import (
	"context"
	"encoding/json"
	rl "github.com/gen2brain/raylib-go/raylib"
	"os"
	"path/filepath"

	"github.com/struckchure/earth-two/identity"
)

func identityProfilePath() (string, error) {
	if path := os.Getenv("EARTH_TWO_IDENTITY_PATH"); path != "" {
		return path, nil
	}
	root, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(root, "earth-two", "pk"), nil
}
func loadIdentityBackup() ([]byte, error) {
	path, err := identityProfilePath()
	if err != nil {
		return nil, err
	}
	b, err := identity.ReadBackup(path)
	if os.IsNotExist(err) {
		return nil, nil
	}
	return b, err
}
func saveIdentityBackup(data []byte) error {
	path, err := identityProfilePath()
	if err != nil {
		return err
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	// Preserve an existing account before importing another identity.
	if old, err := identity.ReadBackup(path); err == nil {
		id := identityBackupID(old)
		if id != "" {
			archive := filepath.Join(dir, "identities", id+".earth-two-key.json")
			if _, err := os.Stat(archive); os.IsNotExist(err) {
				if err := identity.WriteBackup(archive, old); err != nil {
					return err
				}
			}
		}
	}
	f, err := os.CreateTemp(dir, ".identity-*")
	if err != nil {
		return err
	}
	temp := f.Name()
	defer os.Remove(temp)
	if _, err := f.Write(data); err != nil {
		f.Close()
		return err
	}
	if err := f.Sync(); err != nil {
		f.Close()
		return err
	}
	if err := f.Close(); err != nil {
		return err
	}
	if err := os.Rename(temp, path); err != nil {
		return err
	}
	var metadata struct {
		PublicKey []byte `json:"public_key"`
	}
	if err := json.Unmarshal(data, &metadata); err != nil {
		return err
	}
	return identity.WritePublicKey(filepath.Join(dir, "pub"), metadata.PublicKey)
}
func defaultIdentityTransfer() string {
	path, err := identityProfilePath()
	if err != nil {
		return "backup.earth-two-key.json"
	}
	return filepath.Join(filepath.Dir(path), "backup.earth-two-key.json")
}
func readIdentityTransfer(_ context.Context, path string) ([]byte, error) {
	return identity.ReadBackup(path)
}
func exportIdentityTransfer(data []byte, path string) error { return identity.WriteBackup(path, data) }
func identityNetworkDefaults() (string, string) {
	host, db := os.Getenv("SPACETIMEDB_SERVER"), os.Getenv("SPACETIMEDB_DATABASE")
	if host == "" {
		host = "http://localhost:3000"
	}
	if db == "" {
		db = "earth-two"
	}
	return host, db
}

func identityPasteText(pressed bool) string {
	if pressed {
		return rl.GetClipboardText()
	}
	return ""
}
func identityClipboardFocus(_ bool) {}
