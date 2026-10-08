// Package server connects the native companion
// to SpacetimeDB. Account authorization lives in module/, inside the database.
package server

import (
	"context"
	"errors"
	"log/slog"
	"net"
	"net/http"
	"net/url"
	"regexp"
	"sync/atomic"
	"time"

	"go.digitalxero.dev/spacetimedb-client/client"
	"go.digitalxero.dev/spacetimedb-client/types"
)

type Config struct {
	Addr     string
	Host     string
	Database string
}

func Run(ctx context.Context, cfg Config, logger *slog.Logger) error {
	u, err := url.Parse(cfg.Host)
	if err != nil || u.User != nil || u.Host == "" || u.RawQuery != "" || u.Fragment != "" || (u.Path != "" && u.Path != "/") ||
		(u.Scheme != "http" && u.Scheme != "https") ||
		(net.ParseIP(u.Hostname()) == nil && !regexp.MustCompile(`^[a-zA-Z0-9.-]+$`).MatchString(u.Hostname())) {
		return errors.New("server: host must be a plain http(s) SpacetimeDB origin")
	}
	runCtx, stop := context.WithCancel(ctx)
	defer stop()
	var connected atomic.Bool
	conn, err := client.NewDbConnection().WithUri(cfg.Host).WithDatabaseName(cfg.Database).
		OnConnect(func(_ client.DbConnection, _ types.Identity, _ string) { connected.Store(true) }).
		OnDisconnect(func(_ client.DbConnection, _ error) { connected.Store(false) }).Build(runCtx)
	if err != nil {
		return err
	}
	defer conn.Disconnect()
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		if !connected.Load() {
			http.Error(w, "database disconnected", http.StatusServiceUnavailable)
			return
		}
		w.WriteHeader(http.StatusOK)
	})
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Referrer-Policy", "no-referrer")
		mux.ServeHTTP(w, r)
	})
	httpServer := &http.Server{Addr: cfg.Addr, Handler: handler, ReadHeaderTimeout: 5 * time.Second, IdleTimeout: time.Minute}
	errCh := make(chan error, 2)
	go func() { errCh <- conn.Run(runCtx) }()
	go func() { errCh <- httpServer.ListenAndServe() }()
	logger.Info("server started", "addr", cfg.Addr, "database", cfg.Database)
	select {
	case <-ctx.Done():
		err = nil
	case err = <-errCh:
	}
	stop()
	shutdown, finish := context.WithTimeout(context.Background(), 5*time.Second)
	defer finish()
	if closeErr := httpServer.Shutdown(shutdown); closeErr != nil && err == nil {
		err = closeErr
	}
	if errors.Is(err, http.ErrServerClosed) || errors.Is(err, context.Canceled) {
		err = nil
	}
	logger.Info("server stopped")
	return err
}
