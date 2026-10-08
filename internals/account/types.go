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
