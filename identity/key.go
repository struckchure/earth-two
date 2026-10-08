// Package identity implements player-owned Ed25519 accounts. A transport token
// identifies a connection; only the player's signing key authorizes changes.
package identity

import (
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
)

// Key holds a private key in memory. Its JSON and string representations never
// include the seed. Export encrypts the seed for storage and transfer.
type Key struct{ private ed25519.PrivateKey }

func Generate() (*Key, error) {
	_, private, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		return nil, err
	}
	return &Key{private: private}, nil
}

func FromSeed(seed []byte) (*Key, error) {
	if len(seed) != ed25519.SeedSize {
		return nil, errors.New("identity: invalid Ed25519 seed")
	}
	return &Key{private: ed25519.NewKeyFromSeed(seed)}, nil
}

func (k *Key) PublicKey() []byte {
	return append([]byte(nil), k.private[ed25519.SeedSize:]...)
}

// AccountID is independent of email, connection tokens and database hosts.
func AccountID(publicKey []byte) (string, error) {
	if len(publicKey) != ed25519.PublicKeySize {
		return "", errors.New("identity: invalid public key")
	}
	h := sha256.New()
	h.Write([]byte("earth-two/account-id/v1\x00"))
	h.Write(publicKey)
	return "e2_" + hex.EncodeToString(h.Sum(nil)), nil
}

func (k *Key) ID() string     { id, _ := AccountID(k.PublicKey()); return id }
func (k *Key) String() string { return k.ID() }
