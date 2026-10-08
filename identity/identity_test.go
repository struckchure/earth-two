package identity

import (
	"bytes"
	"encoding/json"
	"testing"
	"time"
)

func testKey(t *testing.T) *Key {
	t.Helper()
	k, err := Generate()
	if err != nil {
		t.Fatal(err)
	}
	return k
}

func TestProofBindsEveryAuthorizationField(t *testing.T) {
	k := testKey(t)
	now := time.Unix(1_800_000_000, 0)
	ctx := Context{Database: [32]byte{1}, Sender: [32]byte{2}, Connection: [16]byte{3}}
	proof, err := k.Sign(ctx, "account.set_email", []byte("a@example.com"), 7, now.Add(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	a, err := Verify(ctx, "account.set_email", []byte("a@example.com"), proof, now)
	if err != nil || a.AccountID != k.ID() || a.Sequence != 7 {
		t.Fatalf("authorization: %+v %v", a, err)
	}
	for _, field := range []string{"database", "sender", "connection", "action", "payload", "sequence", "signature", "truncated", "expiry"} {
		t.Run(field, func(t *testing.T) {
			c, action, payload, p, at := ctx, "account.set_email", []byte("a@example.com"), bytes.Clone(proof), now
			switch field {
			case "database":
				c.Database[0]++
			case "sender":
				c.Sender[0]++
			case "connection":
				c.Connection[0]++
			case "action":
				action = "account.link"
			case "payload":
				payload = []byte("b@example.com")
			case "sequence":
				p[32]++
			case "signature":
				p[100]++
			case "truncated":
				p = p[:len(p)-1]
			case "expiry":
				at = now.Add(time.Minute)
			}
			if _, err := Verify(c, action, payload, p, at); err == nil {
				t.Fatal("altered proof authorized")
			}
		})
	}
	p, _ := k.Sign(ctx, "account.link", nil, 7, now.Add(MaxProofLifetime+time.Second))
	if _, err := Verify(ctx, "account.link", nil, p, now); err == nil {
		t.Fatal("excessive lifetime accepted")
	}
}

func TestBackupRestoresIdentityAndSigningAuthority(t *testing.T) {
	k := testKey(t)
	backup, err := k.Export("a long test backup passphrase")
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(backup, k.private.Seed()) {
		t.Fatal("backup leaked seed")
	}
	restored, err := Import(backup, "a long test backup passphrase")
	if err != nil || restored.ID() != k.ID() || !bytes.Equal(restored.PublicKey(), k.PublicKey()) {
		t.Fatalf("restore: %v", err)
	}
	ctx := Context{Database: [32]byte{1}}
	now := time.Now()
	proof, _ := restored.Sign(ctx, "account.link", nil, 8, now.Add(time.Minute))
	if _, err := Verify(ctx, "account.link", nil, proof, now); err != nil {
		t.Fatal(err)
	}
	if _, err := Import(backup, "wrong passphrase"); err == nil {
		t.Fatal("wrong passphrase accepted")
	}
	var document map[string]any
	if err := json.Unmarshal(backup, &document); err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{"account_id", "ciphertext", "iterations", "version"} {
		t.Run(field, func(t *testing.T) {
			copy := make(map[string]any)
			for k, v := range document {
				copy[k] = v
			}
			switch field {
			case "account_id":
				copy[field] = "another-account"
			case "ciphertext":
				copy[field] = "AAAA"
			default:
				copy[field] = 999999999
			}
			corrupt, _ := json.Marshal(copy)
			if _, err := Import(corrupt, "a long test backup passphrase"); err == nil {
				t.Fatal("damaged backup accepted")
			}
		})
	}
	if _, err := Import(append(backup, []byte(" {}")...), "a long test backup passphrase"); err == nil {
		t.Fatal("trailing data accepted")
	}
	if _, err := k.Export(""); err == nil {
		t.Fatal("empty passphrase accepted")
	}
}
