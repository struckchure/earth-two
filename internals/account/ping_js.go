//go:build js && wasm

package account

import (
	"context"
	"errors"
	"strconv"
	"time"
)

func (s *Session) Ping(ctx context.Context) (time.Duration, error) {
	if !s.IsActive() {
		return 0, errors.New("account disconnected")
	}
	b, err := bridge()
	if err != nil {
		return 0, err
	}
	started := time.Now()
	nonce := strconv.FormatInt(started.UnixNano(), 10)
	value, err := promise(ctx, b.Call("ping", s.handle, nonce))
	if err != nil {
		return 0, err
	}
	if value.String() != nonce {
		return 0, errors.New("invalid latency probe response")
	}
	return time.Since(started), nil
}
