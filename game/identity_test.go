//go:build !js

package game

import (
	"context"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/struckchure/earth-two/identity"
)

func waitIdentity(t *testing.T, p *identityPanel) {
	t.Helper()
	until := time.Now().Add(10 * time.Second)
	for p.busy && time.Now().Before(until) {
		p.poll()
		time.Sleep(time.Millisecond)
	}
	if p.busy || p.failed {
		t.Fatalf("identity operation: %s", p.message)
	}
}

func TestInGameIdentityCreateLockExportImport(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "active.json")
	t.Setenv("EARTH_TWO_IDENTITY_PATH", path)
	p := newIdentityPanel()
	defer p.close()
	const password = "synthetic in-game identity passphrase"
	p.fields[0] = password
	p.act(actIdentityCreate)
	waitIdentity(t, p)
	id := p.key.ID()
	publicFile, err := os.ReadFile(filepath.Join(dir, "pub"))
	if err != nil || string(publicFile) != hex.EncodeToString(p.key.PublicKey())+"\n" {
		t.Fatalf("public key companion: %v", err)
	}
	if identityBackupID(p.backup) != id || p.fields[0] != "" {
		t.Fatal("created key was not saved or passphrase remained visible")
	}
	p.act(actIdentityLock)
	if p.key != nil || !p.enabled(actIdentityExport) {
		t.Fatal("locked identity cannot export its backup")
	}
	backupPath := filepath.Join(dir, "transfer.json")
	p.fields[1] = backupPath
	p.act(actIdentityExport)
	waitIdentity(t, p)
	data, err := identity.ReadBackup(backupPath)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := identity.Import(data, password)
	if err != nil || restored.ID() != id {
		t.Fatalf("export: %v", err)
	}
	t.Setenv("EARTH_TWO_IDENTITY_PATH", filepath.Join(dir, "other-device.json"))
	other := newIdentityPanel()
	defer other.close()
	other.fields[0], other.fields[1] = password, backupPath
	other.act(actIdentityImport)
	waitIdentity(t, other)
	if other.key.ID() != id || identityBackupID(other.backup) != id {
		t.Fatal("import changed account identity")
	}
	if other.enabled(actIdentityEmail) {
		t.Fatal("disconnected identity allowed an email mutation")
	}
}

func TestIdentityMenuAndLayout(t *testing.T) {
	t.Setenv("EARTH_TWO_IDENTITY_PATH", filepath.Join(t.TempDir(), "active.json"))
	m := newMenu()
	m.do(actIdentity)
	defer m.identity.close()
	if m.screen() != identityScreen || m.choices() != identityFields+len(items(identityScreen)) {
		t.Fatal("identity is not a native menu screen")
	}
	m.do(actBack)
	if m.screen() != title {
		t.Fatal("identity did not return to its parent")
	}
	m.stack = nil
	m.open(paused)
	m.do(actIdentity)
	m.do(actBack)
	if m.screen() != paused {
		t.Fatal("identity did not return to pause")
	}
	l := layoutFor(identityScreen, 600, 400, 2)
	for _, r := range append(l.rows, l.buttons...) {
		if r.X < 0 || r.Y < 0 || r.X+r.Width > 600 || r.Y+r.Height > 400 {
			t.Fatal("identity form exceeds a small game window")
		}
	}
}

func TestInGameAccountConnectionSurvivesOperationCompletion(t *testing.T) {
	host := os.Getenv("EARTH_TWO_TEST_STDB_HOST")
	if host == "" {
		t.Skip("requires the local identity module")
	}
	database := os.Getenv("EARTH_TWO_TEST_STDB_DATABASE")
	if database == "" {
		t.Fatal("test database is required")
	}
	t.Setenv("SPACETIMEDB_SERVER", host)
	t.Setenv("SPACETIMEDB_DATABASE", database)
	t.Setenv("EARTH_TWO_IDENTITY_PATH", filepath.Join(t.TempDir(), "identity.json"))
	p := newIdentityPanel()
	defer p.close()
	p.fields[0] = "synthetic live menu passphrase"
	p.act(actIdentityCreate)
	waitIdentity(t, p)
	p.act(actIdentityConnect)
	waitIdentity(t, p)
	if p.session == nil || !p.session.IsActive() {
		t.Fatal("account disconnected when its menu operation completed")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := p.session.SetEmail(ctx, "menu@example.com"); err != nil {
		t.Fatal(err)
	}
	p.fields[2] = ""
	p.act(actIdentityEmail)
	waitIdentity(t, p)
}
