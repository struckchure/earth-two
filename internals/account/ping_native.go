//go:build !js

package account

import (
	"context"
	"encoding/binary"
	"errors"
	"go.digitalxero.dev/spacetimedb-client/bsatn"
	"time"
)

type pingArgs struct{ nonce uint64 }

func (a pingArgs) WriteBsatn(w bsatn.Writer) { w.PutU64(a.nonce) }
func (s *Session) Ping(ctx context.Context) (time.Duration, error) {
	if !s.IsActive() {
		return 0, errors.New("account disconnected")
	}
	started := time.Now()
	nonce := uint64(started.UnixNano())
	data, err := s.conn.CallProcedure(ctx, "connection_ping", pingArgs{nonce: nonce})
	if err != nil {
		return 0, err
	}
	if len(data) != 8 || binary.LittleEndian.Uint64(data) != nonce {
		return 0, errors.New("invalid latency probe response")
	}
	return time.Since(started), nil
}
