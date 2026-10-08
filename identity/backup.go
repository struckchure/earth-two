package identity

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/pbkdf2"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"errors"
	"io"
)

// BackupIterations is fixed by format v1 so imports cannot request unbounded
// work. WebCrypto and Go implement the same PBKDF2-SHA256/AES-256-GCM format.
const BackupIterations = 600_000
const MaxBackupSize = 4096

type backup struct {
	Format     string `json:"format"`
	Version    int    `json:"version"`
	AccountID  string `json:"account_id"`
	PublicKey  []byte `json:"public_key"`
	KDF        string `json:"kdf"`
	Iterations int    `json:"iterations"`
	Salt       []byte `json:"salt"`
	Cipher     string `json:"cipher"`
	Nonce      []byte `json:"nonce"`
	Ciphertext []byte `json:"ciphertext"`
}

func (b backup) aad() []byte {
	out := []byte("earth-two/key-backup/v1\x00")
	out = append(out, b.PublicKey...)
	out = binary.LittleEndian.AppendUint32(out, uint32(b.Iterations))
	return append(out, b.Salt...)
}

func backupCipher(password string, salt []byte) (cipher.AEAD, error) {
	if password == "" {
		return nil, errors.New("identity: a backup passphrase is required")
	}
	key, err := pbkdf2.Key(sha256.New, password, salt, BackupIterations, 32)
	if err != nil {
		return nil, err
	}
	defer clear(key)
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, err
	}
	return cipher.NewGCM(block)
}

// Export creates a portable, authenticated encrypted backup. The server never
// receives this document or its passphrase.
func (k *Key) Export(password string) ([]byte, error) {
	b := backup{Format: "earth-two-key", Version: 1, AccountID: k.ID(), PublicKey: k.PublicKey(),
		KDF: "pbkdf2-sha256", Iterations: BackupIterations, Cipher: "aes-256-gcm",
		Salt: make([]byte, 16), Nonce: make([]byte, 12)}
	if _, err := rand.Read(b.Salt); err != nil {
		return nil, err
	}
	if _, err := rand.Read(b.Nonce); err != nil {
		return nil, err
	}
	aead, err := backupCipher(password, b.Salt)
	if err != nil {
		return nil, err
	}
	seed := k.private.Seed()
	defer clear(seed)
	b.Ciphertext = aead.Seal(nil, b.Nonce, seed, b.aad())
	return json.MarshalIndent(b, "", "  ")
}

func Import(data []byte, password string) (*Key, error) {
	if len(data) > MaxBackupSize {
		return nil, errors.New("identity: backup is too large")
	}
	var b backup
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if err := d.Decode(&b); err != nil {
		return nil, errors.New("identity: invalid backup document")
	}
	if err := d.Decode(new(any)); err != io.EOF {
		return nil, errors.New("identity: trailing backup data")
	}
	if b.Format != "earth-two-key" || b.Version != 1 || b.KDF != "pbkdf2-sha256" ||
		b.Iterations != BackupIterations || b.Cipher != "aes-256-gcm" || len(b.PublicKey) != 32 ||
		len(b.Salt) != 16 || len(b.Nonce) != 12 || len(b.Ciphertext) != 48 {
		return nil, errors.New("identity: unsupported or invalid backup format")
	}
	aead, err := backupCipher(password, b.Salt)
	if err != nil {
		return nil, err
	}
	seed, err := aead.Open(nil, b.Nonce, b.Ciphertext, b.aad())
	if err != nil {
		return nil, errors.New("identity: incorrect passphrase or damaged backup")
	}
	defer clear(seed)
	k, err := FromSeed(seed)
	if err != nil {
		return nil, err
	}
	if k.ID() != b.AccountID || !bytes.Equal(k.PublicKey(), b.PublicKey) {
		return nil, errors.New("identity: backup public identity does not match its private key")
	}
	return k, nil
}
