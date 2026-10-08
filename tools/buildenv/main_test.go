package main

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestPublicSettingsOnlyAndExportedOverrides(t *testing.T) {
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "")
	path := filepath.Join(t.TempDir(), ".env")
	data := "AWS_S3_SECRET_ACCESS_KEY=synthetic-private-value\nGOFLAGS=-X secret\nSPACETIMEDB_PUBLISH_TOKEN=private-token\nexport SPACETIMEDB_SERVER=\"https://game.example.org\"\nSPACETIMEDB_DATABASE='test-world'\n"
	if err := os.WriteFile(path, []byte(data), 0600); err != nil {
		t.Fatal(err)
	}
	flags, err := buildFlags(path, "-trimpath '-ldflags=-s -w'")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"-trimpath", "-s -w", ".DefaultHost=https://game.example.org", ".DefaultDatabase=test-world"} {
		if !strings.Contains(flags, want) {
			t.Fatalf("missing public build setting %q", want)
		}
	}
	for _, secret := range []string{"synthetic-private-value", "private-token", "AWS_", "GOFLAGS=-X secret"} {
		if strings.Contains(flags, secret) {
			t.Fatal("private setting entered linker flags")
		}
	}
	t.Setenv("SPACETIMEDB_SERVER", "https://override.example.org")
	flags, err = buildFlags(path, "")
	if err != nil || !strings.Contains(flags, ".DefaultHost=https://override.example.org") {
		t.Fatalf("exported override was ignored: %v", err)
	}
}

func TestMissingFileAndInvalidPublicValues(t *testing.T) {
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "")
	path := filepath.Join(t.TempDir(), "missing")
	if got, err := buildFlags(path, "-trimpath"); err != nil || got != "-trimpath" {
		t.Fatalf("missing file should preserve compiled defaults: %q %v", got, err)
	}
	for _, host := range []string{"https://user:private@example.org", "https://example.org?token=private", "https://example.org/#private", "https://example.org -X secret", "$(touch secret)"} {
		t.Setenv("SPACETIMEDB_SERVER", host)
		if _, err := buildFlags(path, ""); err == nil || strings.Contains(err.Error(), "private") {
			t.Fatal("invalid origin was accepted or printed its value")
		}
	}
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "world -X secret")
	if _, err := buildFlags(path, ""); err == nil {
		t.Fatal("linker injection was accepted")
	}
}

func TestDotenvSecretNeedsNoFileAndDoesNotReachBuildProcesses(t *testing.T) {
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "")
	t.Setenv("BUILD_DOTENV", "SPACETIMEDB_SERVER=https://secret-config.example.org\nSPACETIMEDB_DATABASE=secret-world\nAWS_S3_SECRET_ACCESS_KEY=synthetic-private-value\n")
	flags, err := buildFlags(filepath.Join(t.TempDir(), "absent"), "")
	if err != nil || !strings.Contains(flags, ".DefaultHost=https://secret-config.example.org") || !strings.Contains(flags, ".DefaultDatabase=secret-world") {
		t.Fatalf("secret public settings were not compiled: %v", err)
	}
	for _, entry := range childEnvironment(flags) {
		if strings.HasPrefix(entry, "BUILD_DOTENV=") || strings.Contains(entry, "synthetic-private-value") {
			t.Fatal("full secret forwarded to build subprocess")
		}
	}
	path := filepath.Join(t.TempDir(), ".env")
	if err := os.WriteFile(path, []byte("SPACETIMEDB_SERVER=https://local.example.org\n"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BUILD_DOTENV", "")
	if got, err := buildFlags(path, "-trimpath"); err != nil || got != "-trimpath" {
		t.Fatalf("empty CI secret should ignore local file: %q %v", got, err)
	}
}

func TestNativeAndWasmEmbedOnlyPublicDefaults(t *testing.T) {
	t.Setenv("SPACETIMEDB_SERVER", "")
	t.Setenv("SPACETIMEDB_DATABASE", "")
	root := t.TempDir()
	files := map[string]string{
		"go.mod":                     "module github.com/struckchure/earth-two\n\ngo 1.25\n",
		"internals/account/types.go": "package account\nvar DefaultHost = \"local\"\nvar DefaultDatabase = \"local\"\n",
		"main.go":                    "package main\nimport (\"fmt\"; \"github.com/struckchure/earth-two/internals/account\")\nfunc main() { fmt.Print(account.DefaultHost, \"/\", account.DefaultDatabase) }\n",
		".env":                       "SPACETIMEDB_SERVER=https://compiled.example.org\nSPACETIMEDB_DATABASE=compiled-world\nAWS_S3_SECRET_ACCESS_KEY=synthetic-secret-never-in-client\nSPACETIMEDB_PUBLISH_TOKEN=synthetic-publisher-never-in-client\n",
	}
	for name, data := range files {
		path := filepath.Join(root, name)
		if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(data), 0600); err != nil {
			t.Fatal(err)
		}
	}
	flags, err := buildFlags(filepath.Join(root, ".env"), "-trimpath '-ldflags=-s -w'")
	if err != nil {
		t.Fatal(err)
	}
	for _, target := range []struct{ os, arch, name string }{{runtime.GOOS, runtime.GOARCH, "native"}, {"js", "wasm", "web.wasm"}} {
		output := filepath.Join(root, target.name)
		if target.os == "windows" {
			output += ".exe"
		}
		cmd := exec.Command("go", "build", "-o", output, ".")
		cmd.Dir = root
		cmd.Env = append(os.Environ(), "GOWORK=off", "CGO_ENABLED=0", "GOOS="+target.os, "GOARCH="+target.arch, "GOFLAGS="+flags)
		if out, err := cmd.CombinedOutput(); err != nil {
			t.Fatalf("build %s: %v\n%s", target.name, err, out)
		}
		data, err := os.ReadFile(output)
		if err != nil {
			t.Fatal(err)
		}
		for _, public := range []string{"https://compiled.example.org", "compiled-world"} {
			if !bytes.Contains(data, []byte(public)) {
				t.Fatalf("%s does not contain public defaults", target.name)
			}
		}
		for _, secret := range []string{"synthetic-secret-never-in-client", "synthetic-publisher-never-in-client"} {
			if bytes.Contains(data, []byte(secret)) {
				t.Fatalf("%s contains private build configuration", target.name)
			}
		}
		if target.os != "js" {
			if err := os.Remove(filepath.Join(root, ".env")); err != nil {
				t.Fatal(err)
			}
			out, err := exec.Command(output).CombinedOutput()
			if err != nil || string(out) != "https://compiled.example.org/compiled-world" {
				t.Fatalf("compiled defaults require no runtime file: %s %v", out, err)
			}
		}
	}
}
