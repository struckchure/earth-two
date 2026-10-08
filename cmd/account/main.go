// Command account manages a player's encrypted key and signed account actions.
package main

import (
	"bufio"
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/struckchure/earth-two/identity"
	"github.com/struckchure/earth-two/internals/account"
	"golang.org/x/term"
)

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		return errors.New("usage: account create|import|export|status|email|name [options]")
	}
	command := args[0]
	fs := flag.NewFlagSet("account "+command, flag.ContinueOnError)
	config, err := os.UserHomeDir()
	if err != nil {
		return err
	}
	defaultKey := os.Getenv("EARTH_TWO_IDENTITY_PATH")
	if defaultKey == "" {
		defaultKey = filepath.Join(config, "earth-two", "pk")
	}
	keyPath := fs.String("key", defaultKey, "encrypted active private key file")
	source := fs.String("in", "", "encrypted backup to import")
	target := fs.String("out", "", "encrypted backup to export")
	passwordFile := fs.String("passphrase-file", "", "read passphrase from a file instead of prompting")
	host := fs.String("host", "http://localhost:3000", "SpacetimeDB host")
	database := fs.String("database", "earth-two", "SpacetimeDB database name or identity")
	email := fs.String("email", "", "optional contact address; empty removes it")
	displayName := fs.String("name", "", "display name; names do not have to be unique")
	if err := fs.Parse(args[1:]); err != nil {
		return err
	}
	if fs.NArg() != 0 {
		return errors.New("unexpected positional arguments")
	}
	switch command {
	case "create", "import", "export", "status", "email", "name":
	default:
		return errors.New("unknown account command")
	}
	if command == "import" && *source == "" {
		return errors.New("import requires -in")
	}
	if command == "export" && *target == "" {
		return errors.New("export requires -out")
	}
	password, err := passphrase(*passwordFile)
	if err != nil {
		return err
	}
	if command == "create" {
		key, err := identity.Generate()
		if err != nil {
			return err
		}
		data, err := key.Export(password)
		if err != nil {
			return err
		}
		if err := identity.WriteBackup(*keyPath, data); err != nil {
			return err
		}
		if err := identity.WritePublicKey(filepath.Join(filepath.Dir(*keyPath), "pub"), key.PublicKey()); err != nil {
			return err
		}
		fmt.Println(key.ID())
		return nil
	}
	path := *keyPath
	if command == "import" {
		path = *source
	}
	data, err := identity.ReadBackup(path)
	if err != nil {
		return err
	}
	key, err := identity.Import(data, password)
	if err != nil {
		return err
	}
	if command == "import" || command == "export" {
		// Re-encrypt each transfer with fresh salt and nonce. The account ID and
		// signing key are preserved, and no original file is overwritten.
		data, err = key.Export(password)
		if err != nil {
			return err
		}
		destination := *keyPath
		if command == "export" {
			destination = *target
		}
		if err := identity.WriteBackup(destination, data); err != nil {
			return err
		}
		if command == "import" {
			if err := identity.WritePublicKey(filepath.Join(filepath.Dir(*keyPath), "pub"), key.PublicKey()); err != nil {
				return err
			}
		}
		fmt.Println(key.ID())
		return nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	session, err := account.Connect(ctx, *host, *database, key)
	if err != nil {
		return err
	}
	defer session.Close()
	if command == "email" {
		if err := session.SetEmail(ctx, *email); err != nil {
			return err
		}
	}
	if command == "name" {
		if err := session.SetDisplayName(ctx, *displayName); err != nil {
			return err
		}
	}
	details, err := session.Details()
	if err != nil {
		return err
	}
	fmt.Printf("Account: %s\nName: %s\nEmail: %s\n", key.ID(), details.DisplayName, details.Email)
	return nil
}

func passphrase(path string) (string, error) {
	var data []byte
	if path != "" {
		f, err := os.Open(path)
		if err != nil {
			return "", err
		}
		defer f.Close()
		data, err = io.ReadAll(io.LimitReader(f, 4097))
		if err != nil {
			return "", err
		}
	} else if term.IsTerminal(int(os.Stdin.Fd())) {
		fmt.Fprint(os.Stderr, "Key passphrase: ")
		var err error
		data, err = term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Fprintln(os.Stderr)
		if err != nil {
			return "", err
		}
	} else {
		var err error
		data, err = bufio.NewReader(io.LimitReader(os.Stdin, 4097)).ReadBytes('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
	}
	defer clear(data)
	password := strings.TrimSuffix(strings.TrimSuffix(string(data), "\n"), "\r")
	if password == "" || len(data) > 4096 || !utf8.ValidString(password) {
		return "", errors.New("a nonempty UTF-8 passphrase is required (maximum 4096 bytes)")
	}
	return password, nil
}
