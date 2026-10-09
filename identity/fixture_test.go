package identity

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// Cross-language fixtures for the Rust port (crates/identity). The key is
// test-only data derived from a fixed string; it never belongs to a player.
//
//	EARTH_TWO_WRITE_FIXTURE=1 go test ./identity -run TestFixture
//
// writes crates/identity/fixtures/identity.json. Without the variable the test
// verifies that file and, when present, the Rust-produced identity-rust.json.

const (
	fixtureSeedLabel  = "earth-two/identity-fixture/v1: synthetic test seed, never a player key"
	fixturePassphrase = "fixture passphrase ✓ unicode 二"
	fixtureAction     = "account.set_email"
	fixturePayload    = "fixture@example.com"
	fixtureSequence   = 7
	fixtureNow        = 1_800_000_000_000_000 // UnixMicro
	fixtureExpires    = fixtureNow + 60_000_000
	fixtureDir        = "../crates/identity/fixtures"
)

type fixture struct {
	Seed       string          `json:"seed"`
	PublicKey  string          `json:"public_key"`
	AccountID  string          `json:"account_id"`
	Database   string          `json:"database"`
	Sender     string          `json:"sender"`
	Connection string          `json:"connection"`
	Action     string          `json:"action"`
	Payload    string          `json:"payload"`
	Sequence   uint64          `json:"sequence"`
	NowMicro   int64           `json:"now_unix_micro"`
	Expires    int64           `json:"expires_unix_micro"`
	Proof      string          `json:"proof"`
	Passphrase string          `json:"backup_passphrase"`
	Backup     json.RawMessage `json:"backup"`
}

// rustFixture is what the Rust tests write for Go to verify: a backup Rust
// encrypted and a proof Rust signed, for the same key and request.
type rustFixture struct {
	AccountID  string          `json:"account_id"`
	Proof      string          `json:"proof"`
	Passphrase string          `json:"backup_passphrase"`
	Backup     json.RawMessage `json:"backup"`
}

func fixtureContext() Context {
	var ctx Context
	copy(ctx.Database[:], bytes.Repeat([]byte{0xd1}, 32))
	copy(ctx.Sender[:], bytes.Repeat([]byte{0x5e}, 32))
	copy(ctx.Connection[:], bytes.Repeat([]byte{0xc0}, 16))
	ctx.Database[0], ctx.Sender[0], ctx.Connection[0] = 1, 2, 3
	return ctx
}

func fixtureKey(t *testing.T) *Key {
	t.Helper()
	seed := sha256.Sum256([]byte(fixtureSeedLabel))
	k, err := FromSeed(seed[:])
	if err != nil {
		t.Fatal(err)
	}
	return k
}

func TestFixture(t *testing.T) {
	k := fixtureKey(t)
	ctx := fixtureContext()
	path := filepath.Join(fixtureDir, "identity.json")
	if os.Getenv("EARTH_TWO_WRITE_FIXTURE") == "1" {
		proof, err := k.Sign(ctx, fixtureAction, []byte(fixturePayload), fixtureSequence, time.UnixMicro(fixtureExpires))
		if err != nil {
			t.Fatal(err)
		}
		backup, err := k.Export(fixturePassphrase)
		if err != nil {
			t.Fatal(err)
		}
		seed := sha256.Sum256([]byte(fixtureSeedLabel))
		f := fixture{
			Seed: hex.EncodeToString(seed[:]), PublicKey: hex.EncodeToString(k.PublicKey()), AccountID: k.ID(),
			Database: hex.EncodeToString(ctx.Database[:]), Sender: hex.EncodeToString(ctx.Sender[:]), Connection: hex.EncodeToString(ctx.Connection[:]),
			Action: fixtureAction, Payload: fixturePayload, Sequence: fixtureSequence, NowMicro: fixtureNow, Expires: fixtureExpires,
			Proof: hex.EncodeToString(proof), Passphrase: fixturePassphrase, Backup: backup,
		}
		data, err := json.MarshalIndent(f, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		if err := os.MkdirAll(fixtureDir, 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, append(data, '\n'), 0644); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Skip("fixture not generated:", err)
	}
	var f fixture
	if err := json.Unmarshal(data, &f); err != nil {
		t.Fatal(err)
	}
	if f.AccountID != k.ID() || f.PublicKey != hex.EncodeToString(k.PublicKey()) {
		t.Fatal("fixture identity drifted from the test seed")
	}
	proof, _ := hex.DecodeString(f.Proof)
	// Ed25519 is deterministic, so the Go signature must be reproducible.
	again, err := k.Sign(ctx, f.Action, []byte(f.Payload), f.Sequence, time.UnixMicro(f.Expires))
	if err != nil || !bytes.Equal(again, proof) {
		t.Fatalf("fixture proof is not reproducible: %v", err)
	}
	if a, err := Verify(ctx, f.Action, []byte(f.Payload), proof, time.UnixMicro(f.NowMicro)); err != nil || a.AccountID != k.ID() || a.Sequence != f.Sequence {
		t.Fatalf("fixture proof: %+v %v", a, err)
	}
	restored, err := Import(f.Backup, f.Passphrase)
	if err != nil || restored.ID() != k.ID() {
		t.Fatalf("fixture backup: %v", err)
	}
	if _, err := Import(f.Backup, "wrong passphrase"); err == nil {
		t.Fatal("fixture backup accepted a wrong passphrase")
	}

	// The Rust side's output, when its tests have produced it.
	rustPath := filepath.Join(fixtureDir, "identity-rust.json")
	data, err = os.ReadFile(rustPath)
	if errors.Is(err, os.ErrNotExist) {
		t.Log("no Rust fixture to verify")
		return
	}
	if err != nil {
		t.Fatal(err)
	}
	var r rustFixture
	if err := json.Unmarshal(data, &r); err != nil {
		t.Fatal(err)
	}
	if r.AccountID != k.ID() {
		t.Fatal("Rust fixture is for another account")
	}
	rustKey, err := Import(r.Backup, r.Passphrase)
	if err != nil || rustKey.ID() != k.ID() || !bytes.Equal(rustKey.PublicKey(), k.PublicKey()) {
		t.Fatalf("Rust-encrypted backup: %v", err)
	}
	if _, err := Import(r.Backup, "wrong passphrase"); err == nil {
		t.Fatal("Rust backup accepted a wrong passphrase")
	}
	rustProof, _ := hex.DecodeString(r.Proof)
	if !bytes.Equal(rustProof, proof) {
		t.Fatal("Rust signed different proof bytes for the same request")
	}
	if a, err := Verify(ctx, f.Action, []byte(f.Payload), rustProof, time.UnixMicro(f.NowMicro)); err != nil || a.AccountID != k.ID() {
		t.Fatalf("Rust proof: %+v %v", a, err)
	}
	// A key restored from the Rust backup signs exactly as the original.
	fromRust, err := rustKey.Sign(ctx, f.Action, []byte(f.Payload), f.Sequence, time.UnixMicro(f.Expires))
	if err != nil || !bytes.Equal(fromRust, proof) {
		t.Fatal("key restored from the Rust backup signs differently")
	}
}
