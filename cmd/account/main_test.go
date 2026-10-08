package main

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/struckchure/earth-two/identity"
)

func TestKeyCommandsPreserveIdentityAndExistingFiles(t *testing.T) {
	dir := t.TempDir()
	passwordPath := filepath.Join(dir, "passphrase")
	const password = "synthetic command test passphrase"
	if err := os.WriteFile(passwordPath, []byte(password), 0600); err != nil {
		t.Fatal(err)
	}
	keyPath, backupPath, restoredPath := filepath.Join(dir, "key.json"), filepath.Join(dir, "backup.json"), filepath.Join(dir, "restored.json")
	for _, args := range [][]string{
		{"create", "-key", keyPath},
		{"export", "-key", keyPath, "-out", backupPath},
		{"import", "-key", restoredPath, "-in", backupPath},
	} {
		if err := run(append(args, "-passphrase-file", passwordPath)); err != nil {
			t.Fatal(err)
		}
	}
	var want string
	for _, path := range []string{keyPath, backupPath, restoredPath} {
		data, err := identity.ReadBackup(path)
		if err != nil {
			t.Fatal(err)
		}
		key, err := identity.Import(data, password)
		if err != nil {
			t.Fatal(err)
		}
		if want == "" {
			want = key.ID()
		}
		if key.ID() != want {
			t.Fatal("transfer created a different identity")
		}
		info, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if runtime.GOOS != "windows" && info.Mode().Perm() != 0600 {
			t.Fatal("key file is not private")
		}
	}
	before, _ := os.ReadFile(keyPath)
	if err := run([]string{"create", "-key", keyPath, "-passphrase-file", passwordPath}); err == nil {
		t.Fatal("overwrote an existing key")
	}
	after, _ := os.ReadFile(keyPath)
	if !bytes.Equal(before, after) {
		t.Fatal("existing key changed")
	}
}
