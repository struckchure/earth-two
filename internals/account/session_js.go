//go:build js && wasm

package account

import (
	"context"
	"encoding/hex"
	"errors"
	"strconv"
	"sync"
	"syscall/js"
	"time"

	"github.com/struckchure/earth-two/identity"
)

type Session struct {
	key          *identity.Key
	handle       int
	proofContext identity.Context
	mu           sync.Mutex
}

func bridge() (js.Value, error) {
	b := js.Global().Get("EarthTwoAccount")
	if b.IsUndefined() || b.IsNull() {
		return js.Value{}, errors.New("account: browser networking bridge is not loaded")
	}
	return b, nil
}

func promise(ctx context.Context, p js.Value) (js.Value, error) {
	type result struct {
		value js.Value
		err   error
	}
	out := make(chan result, 1)
	var success, failure js.Func
	success = js.FuncOf(func(_ js.Value, args []js.Value) any {
		out <- result{value: args[0]}
		success.Release()
		failure.Release()
		return nil
	})
	failure = js.FuncOf(func(_ js.Value, args []js.Value) any {
		message := "account: browser operation failed"
		if len(args) > 0 {
			if args[0].Type() == js.TypeString {
				message = args[0].String()
			} else if v := args[0].Get("message"); !v.IsUndefined() {
				message = v.String()
			}
		}
		out <- result{err: errors.New(message)}
		success.Release()
		failure.Release()
		return nil
	})
	p.Call("then", success, failure)
	select {
	case <-ctx.Done():
		return js.Value{}, ctx.Err()
	case r := <-out:
		return r.value, r.err
	}
}

func rawID(text string, target []byte) error {
	decoded, err := hex.DecodeString(text)
	if err != nil || len(decoded) != len(target) {
		return errors.New("account: invalid network identity")
	}
	for i := range decoded {
		target[len(target)-1-i] = decoded[i]
	}
	return nil
}

func Connect(ctx context.Context, host, database string, key *identity.Key) (*Session, error) {
	if key == nil {
		return nil, errors.New("account: a player key is required")
	}
	waitCtx, finish := context.WithTimeout(ctx, 45*time.Second)
	defer finish()
	b, err := bridge()
	if err != nil {
		return nil, err
	}
	v, err := promise(waitCtx, b.Call("connect", host, database, key.ID()))
	if err != nil {
		return nil, err
	}
	s := &Session{key: key, handle: v.Get("handle").Int()}
	if err = rawID(v.Get("database").String(), s.proofContext.Database[:]); err == nil {
		err = rawID(v.Get("sender").String(), s.proofContext.Sender[:])
	}
	if err == nil {
		err = rawID(v.Get("connection").String(), s.proofContext.Connection[:])
	}
	if err != nil {
		s.Close()
		return nil, err
	}
	revision := s.Revision()
	action := "account.link"
	if revision == 0 {
		action = "account.register"
	}
	if err := s.mutate(waitCtx, action, "", revision); err != nil {
		s.Close()
		return nil, err
	}
	return s, nil
}

func (s *Session) Context() identity.Context { return s.proofContext }
func (s *Session) Close() {
	if b, err := bridge(); err == nil {
		b.Call("close", s.handle)
	}
}
func (s *Session) IsActive() bool {
	b, err := bridge()
	return err == nil && b.Call("active", s.handle).Bool()
}
func (s *Session) Revision() uint64 {
	b, err := bridge()
	if err != nil {
		return 0
	}
	n, _ := strconv.ParseUint(b.Call("revision", s.handle).String(), 10, 64)
	return n
}
func (s *Session) Details() (*Details, error) {
	b, err := bridge()
	if err != nil {
		return nil, err
	}
	v := b.Call("details", s.handle)
	if v.IsNull() || v.IsUndefined() {
		return nil, errors.New("account: connection is not linked")
	}
	n, err := strconv.ParseUint(v.Get("revision").String(), 10, 64)
	if err != nil {
		return nil, err
	}
	public := make([]byte, v.Get("publicKey").Get("length").Int())
	js.CopyBytesToGo(public, v.Get("publicKey"))
	return &Details{ID: v.Get("id").String(), PublicKey: public, Revision: n, Email: v.Get("email").String(), DisplayName: v.Get("displayName").String()}, nil
}
func (s *Session) mutate(ctx context.Context, action, email string, revision uint64) error {
	var payload []byte
	if action == "account.set_email" || action == "account.set_display_name" {
		payload = []byte(email)
	}
	proof, err := s.key.Sign(s.proofContext, action, payload, revision, time.Now().Add(time.Minute))
	if err != nil {
		return err
	}
	b, err := bridge()
	if err != nil {
		return err
	}
	bytes := js.Global().Get("Uint8Array").New(len(proof))
	js.CopyBytesToJS(bytes, proof)
	_, err = promise(ctx, b.Call("mutate", s.handle, action, email, bytes))
	return err
}
func (s *Session) SetEmail(ctx context.Context, email string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.mutate(ctx, "account.set_email", email, s.Revision())
}

func (s *Session) SetDisplayName(ctx context.Context, name string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.mutate(ctx, "account.set_display_name", name, s.Revision())
}
