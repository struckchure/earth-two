package server

import (
	"io"
	"log/slog"
)

func NewLogger(w io.Writer) *slog.Logger {
	return slog.New(slog.NewJSONHandler(w, nil)).With("service", "earth-two-server")
}
