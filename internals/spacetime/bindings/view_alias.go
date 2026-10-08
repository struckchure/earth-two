package bindings

import "go.digitalxero.dev/spacetimedb-client/bsatn"

// stdb-go v0.7.0 names the view cache after its accessor but emits its row
// type under the Rust return type's name. Keep this compatibility adapter
// separate from generated files so regeneration is reproducible.
type MyAccount = AccountDetails

func ReadMyAccount(r bsatn.Reader) (*MyAccount, error) { return ReadAccountDetails(r) }
