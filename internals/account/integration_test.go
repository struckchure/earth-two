//go:build !js

package account

import (
	"context"
	"os"
	"testing"
	"time"

	"github.com/struckchure/earth-two/identity"
	"github.com/struckchure/earth-two/internals/spacetime/bindings"
)

// Runs against an isolated local database published with this repository's
// module. It creates fresh synthetic keys, never reads a real player's key.
func TestKeyOwnedAccountIntegration(t *testing.T) {
	host := os.Getenv("EARTH_TWO_TEST_STDB_HOST")
	if host == "" {
		t.Skip("set EARTH_TWO_TEST_STDB_HOST and EARTH_TWO_TEST_STDB_DATABASE for integration tests")
	}
	database := os.Getenv("EARTH_TWO_TEST_STDB_DATABASE")
	if database == "" {
		t.Fatal("EARTH_TWO_TEST_STDB_DATABASE is required")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	key, err := identity.Generate()
	if err != nil {
		t.Fatal(err)
	}
	first, err := Connect(ctx, host, database, key)
	if err != nil {
		t.Fatal(err)
	}
	defer first.Close()
	if latency, err := first.Ping(ctx); err != nil || latency <= 0 {
		t.Fatalf("latency probe: %v %v", latency, err)
	}
	details, err := first.Details()
	if err != nil || details.ID != key.ID() || details.Email != "" {
		t.Fatalf("new account: %+v %v", details, err)
	}
	if err := first.SetEmail(ctx, "player@example.com"); err != nil {
		t.Fatal(err)
	}
	const displayName = "Any name! 🦊 / 二"
	if err := first.SetDisplayName(ctx, displayName); err != nil {
		t.Fatal(err)
	}
	if err := first.SetEmail(ctx, ""); err != nil {
		t.Fatal(err)
	}
	backup, err := key.Export("integration backup passphrase")
	if err != nil {
		t.Fatal(err)
	}
	restored, err := identity.Import(backup, "integration backup passphrase")
	if err != nil {
		t.Fatal(err)
	}
	second, err := Connect(ctx, host, database, restored)
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close()
	if first.Context().Sender == second.Context().Sender {
		t.Fatal("expected a new transport identity")
	}
	if details, err := second.Details(); err != nil || details.ID != key.ID() {
		t.Fatalf("restored account: %+v %v", details, err)
	}
	if details, err := second.Details(); err != nil || details.DisplayName != displayName {
		t.Fatalf("display name lost on import: %+v %v", details, err)
	}
	otherKey, _ := identity.Generate()
	other, err := Connect(ctx, host, database, otherKey)
	if err != nil {
		t.Fatal(err)
	}
	defer other.Close()
	if err := other.SetDisplayName(ctx, displayName); err != nil {
		t.Fatal("duplicate display name was rejected:", err)
	}
	if err := second.SetEmail(ctx, "restored@example.com"); err != nil {
		t.Fatal(err)
	}
	revision := second.Revision()
	proof, err := restored.Sign(second.Context(), "account.set_email", []byte("signed@example.com"), revision, time.Now().Add(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	reject := func(name string, email string, bytes []byte) {
		t.Helper()
		if err := bindings.CallSetAccountEmail(second.conn, email, bytes); err != nil {
			t.Fatal(err)
		}
		select {
		case <-ctx.Done():
			t.Fatal(name + ": " + ctx.Err().Error())
		case err := <-second.errors:
			if err == nil {
				t.Fatal(name + ": no reducer error")
			}
		}
		if second.Revision() != revision {
			t.Fatal(name + ": invalid action consumed a sequence")
		}
	}
	reject("transport token alone", "signed@example.com", nil)
	reject("altered payload", "tampered@example.com", proof)
	wrongConnection, _ := restored.Sign(first.Context(), "account.set_email", []byte("signed@example.com"), revision, time.Now().Add(time.Minute))
	reject("another connection", "signed@example.com", wrongConnection)
	wrongScope := second.Context()
	wrongScope.Database[0]++
	wrongDatabase, _ := restored.Sign(wrongScope, "account.set_email", []byte("signed@example.com"), revision, time.Now().Add(time.Minute))
	reject("another database", "signed@example.com", wrongDatabase)
	if err := bindings.CallSetAccountEmail(second.conn, "signed@example.com", proof); err != nil {
		t.Fatal(err)
	}
	want := "signed@example.com"
	if err := second.committed(ctx, revision+1, &want); err != nil {
		t.Fatal(err)
	}
	revision++
	reject("replay", "signed@example.com", proof)
	if _, err := second.conn.OneOffQuery("SELECT * FROM account_email"); err == nil {
		t.Fatal("private email table was readable")
	}
	if err := second.SetEmail(ctx, "bad email"); err == nil {
		t.Fatal("invalid email accepted")
	}
	if second.Revision() != revision {
		t.Fatal("failed action consumed a sequence")
	}
}
