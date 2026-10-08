//go:build !js

package account

import "os"

// NetworkDefaults uses the compiled public settings. Exported variables may
// override them for local development; clients never read a .env file.
func NetworkDefaults() (string, string) {
	get := func(key, fallback string) string {
		if value := os.Getenv(key); value != "" {
			return value
		}
		return fallback
	}
	return get("SPACETIMEDB_SERVER", DefaultHost), get("SPACETIMEDB_DATABASE", DefaultDatabase)
}
