//go:build js && wasm

package account

import (
	"context"
	"os"
	"syscall/js"
	"testing"
	"time"

	"github.com/struckchure/earth-two/identity"
)

func TestBrowserKeyOwnedAccount(t *testing.T) {
	host, database := os.Getenv("EARTH_TWO_TEST_STDB_HOST"), os.Getenv("EARTH_TWO_TEST_STDB_DATABASE")
	if host == "" || js.Global().Get("EarthTwoAccount").IsUndefined() {
		t.Skip("requires the networking bridge and a local test database")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	key, err := identity.Generate()
	if err != nil {
		t.Fatal(err)
	}
	first, err := Connect(ctx, host, database, key)
	if err != nil {
		t.Fatal(err)
	}
	if latency, err := first.Ping(ctx); err != nil || latency <= 0 {
		t.Fatalf("browser latency probe: %v %v", latency, err)
	}
	if err := first.SetEmail(ctx, "browser-wasm@example.com"); err != nil {
		t.Fatal(err)
	}
	if err := first.SetDisplayName(ctx, "Browser player 🦊"); err != nil {
		t.Fatal(err)
	}
	backup, err := key.Export("synthetic browser-WASM transfer passphrase")
	if err != nil {
		t.Fatal(err)
	}
	restored, err := identity.Import(backup, "synthetic browser-WASM transfer passphrase")
	if err != nil {
		t.Fatal(err)
	}
	first.Close()
	second, err := Connect(ctx, host, database, restored)
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close()
	details, err := second.Details()
	if err != nil || details.ID != key.ID() || details.Email != "browser-wasm@example.com" || details.DisplayName != "Browser player 🦊" {
		t.Fatalf("imported browser identity: %+v %v", details, err)
	}
	if err := second.SetEmail(ctx, ""); err != nil {
		t.Fatal(err)
	}
}
