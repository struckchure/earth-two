//go:build !js

package game

import "github.com/struckchure/illusion"

type performancePlugin struct{}

func (performancePlugin) Build(*illusion.App) {}
