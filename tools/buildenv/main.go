// Command buildenv runs a build with public defaults from BUILD_DOTENV or .env.
// It never sources the file or exports its private settings to child processes.
package main

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"os/exec"
	"regexp"
	"strings"
)

const accountPackage = "github.com/struckchure/earth-two/internals/account"

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: go run ./tools/buildenv command [args...]")
		os.Exit(2)
	}
	flags, err := buildFlags(".env", os.Getenv("GOFLAGS"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "build configuration:", err)
		os.Exit(1)
	}
	cmd := exec.Command(os.Args[1], os.Args[2:]...)
	cmd.Env = childEnvironment(flags)
	cmd.Stdin, cmd.Stdout, cmd.Stderr = os.Stdin, os.Stdout, os.Stderr
	if err := cmd.Run(); err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			os.Exit(exit.ExitCode())
		}
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func buildFlags(path, existing string) (string, error) {
	values := map[string]string{}
	read := func(reader io.Reader) error {
		scan := bufio.NewScanner(reader)
		scan.Buffer(make([]byte, 4096), 1024*1024)
		for scan.Scan() {
			line := strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(scan.Text()), "export "))
			key, value, found := strings.Cut(line, "=")
			key = strings.TrimSpace(key)
			if !found || (key != "SPACETIMEDB_SERVER" && key != "SPACETIMEDB_DATABASE") {
				continue
			}
			value = strings.TrimSpace(value)
			if len(value) >= 2 && ((value[0] == '"' && value[len(value)-1] == '"') || (value[0] == '\'' && value[len(value)-1] == '\'')) {
				value = value[1 : len(value)-1]
			}
			values[key] = value
		}
		if err := scan.Err(); err != nil {
			return errors.New("could not read build settings")
		}
		return nil
	}
	if dotenv, found := os.LookupEnv("BUILD_DOTENV"); found {
		// CI supplies the secret directly. Even an empty secret means no file.
		if err := read(strings.NewReader(dotenv)); err != nil {
			return "", err
		}
	} else if file, err := os.Open(path); err == nil {
		defer file.Close()
		if err := read(file); err != nil {
			return "", err
		}
	} else if !os.IsNotExist(err) {
		return "", err
	}
	for _, key := range []string{"SPACETIMEDB_SERVER", "SPACETIMEDB_DATABASE"} {
		if value := os.Getenv(key); value != "" {
			values[key] = value
		}
	}
	var overrides []string
	if host := values["SPACETIMEDB_SERVER"]; host != "" {
		u, err := url.Parse(host)
		if err != nil || u.Hostname() == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" ||
			(u.Scheme != "http" && u.Scheme != "https") || (u.Path != "" && u.Path != "/") ||
			strings.ContainsAny(host, " \t\r\n\"'\\") {
			return "", errors.New("SPACETIMEDB_SERVER must be a public HTTP(S) origin without credentials")
		}
		overrides = append(overrides, "-X", accountPackage+".DefaultHost="+host)
	}
	if database := values["SPACETIMEDB_DATABASE"]; database != "" {
		if !regexp.MustCompile(`^[a-z0-9]+(?:-[a-z0-9]+)*$`).MatchString(database) {
			return "", errors.New("SPACETIMEDB_DATABASE must be a lowercase database name or identity")
		}
		overrides = append(overrides, "-X", accountPackage+".DefaultDatabase="+database)
	}
	if len(overrides) == 0 {
		return existing, nil
	}
	// GOFLAGS uses Go's quoted-word syntax, not shell expansion. Preserve other
	// flags and merge any existing linker options into the single -ldflags flag.
	words, err := splitFlags(existing)
	if err != nil {
		return "", err
	}
	var flags, linker []string
	for _, word := range words {
		if strings.HasPrefix(word, "-ldflags=") || strings.HasPrefix(word, "--ldflags=") {
			_, value, _ := strings.Cut(word, "=")
			linker = []string{value} // Match Go's last-flag-wins behavior.
		} else {
			flags = append(flags, word)
		}
	}
	linker = append(linker, overrides...)
	flags = append(flags, "-ldflags="+strings.Join(linker, " "))
	for i, flag := range flags {
		if strings.ContainsAny(flag, " \t\r\n") {
			quote := "'"
			if strings.Contains(flag, quote) {
				quote = "\""
				if strings.Contains(flag, quote) {
					return "", errors.New("GOFLAGS contains an unsupported combination of quotes")
				}
			}
			flags[i] = quote + flag + quote
		}
	}
	return strings.Join(flags, " "), nil
}

func childEnvironment(flags string) []string {
	var environment []string
	for _, entry := range os.Environ() {
		key, _, _ := strings.Cut(entry, "=")
		if !strings.EqualFold(key, "BUILD_DOTENV") && !strings.EqualFold(key, "GOFLAGS") {
			environment = append(environment, entry)
		}
	}
	return append(environment, "GOFLAGS="+flags)
}

func splitFlags(value string) ([]string, error) {
	var result []string
	for value = strings.TrimSpace(value); value != ""; value = strings.TrimSpace(value) {
		end := strings.IndexAny(value, " \t\r\n")
		if value[0] == '\'' || value[0] == '"' {
			end = strings.IndexByte(value[1:], value[0])
			if end < 0 {
				return nil, errors.New("GOFLAGS has an unterminated quoted flag")
			}
			result = append(result, value[1:end+1])
			value = value[end+2:]
			continue
		}
		if end < 0 {
			end = len(value)
		}
		result = append(result, value[:end])
		value = value[end:]
	}
	return result, nil
}
