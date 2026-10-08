package identity

import (
	"crypto/ed25519"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
)

// WritePublicKey stores printable Ed25519 public bytes atomically. It is a
// convenience file; authorization derives the key from the unlocked seed.
func WritePublicKey(path string, public []byte) error {
	if len(public) != ed25519.PublicKeySize {
		return errors.New("identity: invalid public key")
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	f, err := os.CreateTemp(dir, ".public-key-*")
	if err != nil {
		return err
	}
	temp := f.Name()
	defer os.Remove(temp)
	if err := f.Chmod(0644); err != nil {
		f.Close()
		return err
	}
	if _, err := f.WriteString(hex.EncodeToString(public) + "\n"); err != nil {
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
	return os.Rename(temp, path)
}
