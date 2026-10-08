package identity

import (
	"errors"
	"io"
	"os"
	"path/filepath"
)

func ReadBackup(path string) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	b, err := io.ReadAll(io.LimitReader(f, MaxBackupSize+1))
	if err != nil {
		return nil, err
	}
	if len(b) > MaxBackupSize {
		return nil, errors.New("identity: backup is too large")
	}
	return b, nil
}

// WriteBackup never overwrites an existing key or backup. Both the active file
// and exported files remain encrypted. An interrupted write is removed.
func WriteBackup(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return err
	}
	ok := false
	defer func() {
		f.Close()
		if !ok {
			os.Remove(path)
		}
	}()
	if _, err := f.Write(data); err != nil {
		return err
	}
	if err := f.Sync(); err != nil {
		return err
	}
	if err := f.Close(); err != nil {
		return err
	}
	ok = true
	return nil
}
