//go:build !js

// Package account connects player-owned keys to SpacetimeDB transport identities.
// Connection tokens authorize reads; signed reducer arguments authorize changes.
package account

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/struckchure/earth-two/identity"
	"github.com/struckchure/earth-two/internals/spacetime/bindings"
	"go.digitalxero.dev/spacetimedb-client/client"
	"go.digitalxero.dev/spacetimedb-client/types"
)

type Session struct {
	key      *identity.Key
	database [32]byte
	conn     client.DbConnection
	db       *bindings.ModuleBindings
	cancel   context.CancelFunc
	events   chan struct{}
	errors   chan error
	mu       sync.Mutex // serialize signed operations on this connection
}

func Connect(ctx context.Context, host, database string, key *identity.Key) (*Session, error) {
	if key == nil {
		return nil, errors.New("account: a player key is required")
	}
	waitCtx, finish := context.WithTimeout(ctx, 20*time.Second)
	defer finish()
	scope, err := DatabaseIdentity(waitCtx, host, database)
	if err != nil {
		return nil, err
	}
	runCtx, cancel := context.WithCancel(ctx)
	s := &Session{key: key, database: scope, cancel: cancel, events: make(chan struct{}, 1), errors: make(chan error, 1)}
	ready := make(chan struct{}, 1)
	report := func(err error) {
		if err != nil {
			select {
			case s.errors <- err:
			default:
			}
		}
	}
	conn, err := client.NewDbConnection().WithUri(host).WithDatabaseName(database).
		OnConnect(func(_ client.DbConnection, _ types.Identity, _ string) { ready <- struct{}{} }).
		OnConnectError(report).OnDisconnect(func(_ client.DbConnection, err error) { report(err) }).
		OnReducerError(report).Build(waitCtx)
	if err != nil {
		cancel()
		return nil, err
	}
	s.conn = conn
	s.db = bindings.NewModuleBindings(conn)
	s.db.Account.OnInsert(func(_ *bindings.Account) { s.notify() })
	s.db.Account.OnUpdate(func(_, _ *bindings.Account) { s.notify() })
	s.db.MyAccount.OnInsert(func(_ *bindings.MyAccount) { s.notify() })
	s.db.MyAccount.OnDelete(func(_ *bindings.MyAccount) { s.notify() })
	go func() { report(conn.Run(runCtx)) }()
	if err := s.wait(waitCtx, ready); err != nil {
		s.Close()
		return nil, err
	}
	applied := make(chan struct{}, 1)
	_, err = conn.Subscribe("SELECT * FROM account WHERE id = '"+key.ID()+"'", "SELECT * FROM my_account").
		OnApplied(func() { applied <- struct{}{} }).OnError(report).Build()
	if err == nil {
		err = s.wait(waitCtx, applied)
	}
	if err == nil {
		err = s.link(waitCtx)
	}
	if err != nil {
		s.Close()
		return nil, err
	}
	return s, nil
}

func (s *Session) notify() {
	select {
	case s.events <- struct{}{}:
	default:
	}
}
func (s *Session) wait(ctx context.Context, ready <-chan struct{}) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	case err := <-s.errors:
		return err
	case <-ready:
		return nil
	}
}

func (s *Session) Close()         { s.cancel(); s.conn.Disconnect() }
func (s *Session) IsActive() bool { return s.conn.IsActive() }

func (s *Session) Context() identity.Context {
	return identity.Context{Database: s.database, Sender: s.conn.Identity().Bytes(), Connection: s.conn.ConnectionId().Bytes()}
}

func (s *Session) Revision() uint64 {
	var rev uint64
	s.db.Account.Iter(func(a *bindings.Account) bool { rev = a.Revision; return false })
	return rev
}

func (s *Session) Details() (*Details, error) {
	var details *Details
	s.db.MyAccount.Iter(func(a *bindings.MyAccount) bool {
		details = &Details{ID: a.ID, PublicKey: append([]byte(nil), a.PublicKey...), Revision: a.Revision, Email: a.Email, DisplayName: a.DisplayName}
		return false
	})
	if details == nil {
		return nil, errors.New("account: connection is not linked")
	}
	return details, nil
}

func (s *Session) link(ctx context.Context) error {
	revision := s.Revision()
	action := "account.link"
	call := bindings.CallLinkAccount
	if revision == 0 {
		action, call = "account.register", bindings.CallRegisterAccount
	}
	proof, err := s.key.Sign(s.Context(), action, nil, revision, time.Now().Add(time.Minute))
	if err != nil {
		return err
	}
	if err := call(s.conn, proof); err != nil {
		return err
	}
	return s.committed(ctx, revision+1, nil)
}

func (s *Session) committed(ctx context.Context, revision uint64, email *string) error {
	for {
		details, err := s.Details()
		if err == nil && details.Revision == revision && (email == nil || details.Email == *email) {
			return nil
		}
		if err := s.wait(ctx, s.events); err != nil {
			return err
		}
	}
}

func (s *Session) SetEmail(ctx context.Context, email string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	revision := s.Revision()
	proof, err := s.key.Sign(s.Context(), "account.set_email", []byte(email), revision, time.Now().Add(time.Minute))
	if err != nil {
		return err
	}
	if err := bindings.CallSetAccountEmail(s.conn, email, proof); err != nil {
		return err
	}
	return s.committed(ctx, revision+1, &email)
}

func (s *Session) SetDisplayName(ctx context.Context, name string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	revision := s.Revision()
	proof, err := s.key.Sign(s.Context(), "account.set_display_name", []byte(name), revision, time.Now().Add(time.Minute))
	if err != nil {
		return err
	}
	if err := bindings.CallSetAccountDisplayName(s.conn, name, proof); err != nil {
		return err
	}
	if err := s.committed(ctx, revision+1, nil); err != nil {
		return err
	}
	details, err := s.Details()
	if err != nil {
		return err
	}
	if details.DisplayName != name {
		return errors.New("display name has not synchronized")
	}
	return nil
}

// DatabaseIdentity obtains the immutable database identity used as the proof's
// audience. A rename does not change it; another database cannot replay proofs.
func DatabaseIdentity(ctx context.Context, host, database string) ([32]byte, error) {
	u, err := url.Parse(host)
	if err != nil || u.Host == "" || u.User != nil {
		return [32]byte{}, errors.New("account: invalid SpacetimeDB host")
	}
	switch u.Scheme {
	case "ws":
		u.Scheme = "http"
	case "wss":
		u.Scheme = "https"
	case "http", "https":
	default:
		return [32]byte{}, errors.New("account: invalid host scheme")
	}
	u.Path = strings.TrimRight(u.Path, "/") + "/v1/database/" + url.PathEscape(database) + "/identity"
	u.RawQuery, u.Fragment = "", ""
	req, err := http.NewRequestWithContext(ctx, "GET", u.String(), nil)
	if err != nil {
		return [32]byte{}, err
	}
	r, err := http.DefaultClient.Do(req)
	if err != nil {
		return [32]byte{}, err
	}
	defer r.Body.Close()
	if r.StatusCode == http.StatusNotFound {
		pingURL := *u
		pingURL.Path = "/v1/ping"
		ping, err := http.NewRequestWithContext(ctx, "GET", pingURL.String(), nil)
		if err == nil {
			if response, err := http.DefaultClient.Do(ping); err == nil {
				response.Body.Close()
				if response.StatusCode != http.StatusOK {
					return [32]byte{}, fmt.Errorf("%s is not the game database server", host)
				}
			}
		}
		return [32]byte{}, fmt.Errorf("game database %q is not published; start it with make server", database)
	}
	if r.StatusCode != http.StatusOK {
		return [32]byte{}, fmt.Errorf("game database %q at %s returned %d", database, host, r.StatusCode)
	}
	b, err := io.ReadAll(io.LimitReader(r.Body, 257))
	if err != nil {
		return [32]byte{}, err
	}
	id := strings.TrimSpace(string(b))
	if strings.HasPrefix(id, "\"") {
		if err := json.Unmarshal(b, &id); err != nil {
			return [32]byte{}, err
		}
	}
	raw, err := hex.DecodeString(id)
	if err != nil || len(raw) != 32 {
		return [32]byte{}, errors.New("account: invalid database identity")
	}
	var out [32]byte
	for i := range raw {
		out[31-i] = raw[i]
	}
	return out, nil
}
