package main

import (
	"context"
	"flag"
	"os"
	"os/signal"
	"syscall"

	"github.com/struckchure/earth-two/internals/server"
)

func main() {
	cfg := server.Config{}
	host, database := os.Getenv("SPACETIMEDB_SERVER"), os.Getenv("SPACETIMEDB_DATABASE")
	if host == "" {
		host = "http://localhost:3000"
	}
	if database == "" {
		database = "earth-two"
	}
	flag.StringVar(&cfg.Addr, "addr", ":8081", "HTTP address for health checks")
	flag.StringVar(&cfg.Host, "host", host, "SpacetimeDB HTTP origin")
	flag.StringVar(&cfg.Database, "database", database, "SpacetimeDB database name or identity")
	flag.Parse()
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	logger := server.NewLogger(os.Stdout)
	if err := server.Run(ctx, cfg, logger); err != nil {
		logger.Error("server failed", "error", err)
		os.Exit(1)
	}
}
