package identity

import (
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"time"
)

const (
	ProofSize        = ed25519.PublicKeySize + 8 + 8 + ed25519.SignatureSize
	MaxProofLifetime = 2 * time.Minute
)

// Context binds a signature to one database and live connection. IDs are the
// raw little-endian bytes from the SpacetimeDB SDK, not their display strings.
type Context struct {
	Database   [32]byte
	Sender     [32]byte
	Connection [16]byte
}

type Authorization struct {
	AccountID string
	PublicKey []byte
	Sequence  uint64
}

func message(ctx Context, header []byte, action string, payload []byte) []byte {
	b := append([]byte("earth-two/account-action/v1\x00"), ctx.Database[:]...)
	b = append(b, ctx.Sender[:]...)
	b = append(b, ctx.Connection[:]...)
	b = append(b, header...)
	b = binary.LittleEndian.AppendUint16(b, uint16(len(action)))
	b = append(b, action...)
	digest := sha256.Sum256(payload)
	return append(b, digest[:]...)
}

// Sign returns a compact proof, never the private key. Sequence must match the
// account's persisted revision; the reducer advances it in the same transaction.
func (k *Key) Sign(ctx Context, action string, payload []byte, sequence uint64, expires time.Time) ([]byte, error) {
	if len(action) == 0 || len(action) > 255 || expires.UnixMicro() <= 0 {
		return nil, errors.New("identity: invalid signing request")
	}
	b := append([]byte(nil), k.PublicKey()...)
	b = binary.LittleEndian.AppendUint64(b, sequence)
	b = binary.LittleEndian.AppendUint64(b, uint64(expires.UnixMicro()))
	return append(b, ed25519.Sign(k.private, message(ctx, b, action, payload))...), nil
}

// Verify checks scope, action, payload and expiry. The caller must also check
// Sequence against durable account state before performing a mutation.
func Verify(ctx Context, action string, payload, proof []byte, now time.Time) (Authorization, error) {
	if len(proof) != ProofSize || len(action) == 0 || len(action) > 255 {
		return Authorization{}, errors.New("identity: invalid proof")
	}
	expires := binary.LittleEndian.Uint64(proof[40:48])
	if now.UnixMicro() <= 0 || expires > uint64(^uint64(0)>>1) ||
		expires <= uint64(now.UnixMicro()) || expires-uint64(now.UnixMicro()) > uint64(MaxProofLifetime.Microseconds()) {
		return Authorization{}, errors.New("identity: expired or excessive proof lifetime")
	}
	if !ed25519.Verify(ed25519.PublicKey(proof[:32]), message(ctx, proof[:48], action, payload), proof[48:]) {
		return Authorization{}, errors.New("identity: invalid signature")
	}
	public := append([]byte(nil), proof[:32]...)
	id, _ := AccountID(public)
	return Authorization{AccountID: id, PublicKey: public, Sequence: binary.LittleEndian.Uint64(proof[32:40])}, nil
}
