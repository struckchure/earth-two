//go:build !js

package account

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

func TestNetworkDefaultsIgnoreDotEnvAndAllowExportedOverrides(t *testing.T) {
	t.Chdir(t.TempDir())
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "")
	if host, db := NetworkDefaults(); host != DefaultHost || db != DefaultDatabase {
		t.Fatalf("defaults: %s %s", host, db)
	}
	if err := os.WriteFile(".env", []byte("AWS_SECRET_ACCESS_KEY=synthetic-secret\nSPACETIMEDB_SERVER=\"http://127.0.0.1:3999\"\nSPACETIMEDB_DATABASE=custom-game\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if host, db := NetworkDefaults(); host != DefaultHost || db != DefaultDatabase {
		t.Fatalf("runtime read build-only file: %s %s", host, db)
	}
	t.Setenv("SPACETIMEDB_SERVER", "http://127.0.0.1:4000")
	if host, _ := NetworkDefaults(); host != "http://127.0.0.1:4000" {
		t.Fatal("exported setting was overridden")
	}
	if os.Getenv("AWS_SECRET_ACCESS_KEY") == "synthetic-secret" {
		t.Fatal("game imported AWS credentials into its environment")
	}
}

func TestDatabaseLookupDistinguishesWrongServiceAndMissingModule(t *testing.T) {
	for _, gameServer := range []bool{false, true} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if gameServer && r.URL.Path == "/v1/ping" {
				w.WriteHeader(200)
				return
			}
			http.NotFound(w, r)
		}))
		_, err := DatabaseIdentity(context.Background(), server.URL, "missing-game")
		server.Close()
		if err == nil {
			t.Fatal("missing database accepted")
		}
		want := "not the game database server"
		if gameServer {
			want = "start it with make server"
		}
		if !strings.Contains(err.Error(), want) {
			t.Fatalf("diagnostic: %v", err)
		}
	}
}
