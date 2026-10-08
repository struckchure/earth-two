package account

// Details is shared by the native and browser adapters, without importing a
// native-only networking SDK into the browser game.
type Details struct {
	ID          string
	PublicKey   []byte
	Revision    uint64
	Email       string
	DisplayName string
}

// Build-time public defaults, set by tools/buildenv through Go's linker.
var DefaultHost = "http://127.0.0.1:3001"
var DefaultDatabase = "earth-two"
