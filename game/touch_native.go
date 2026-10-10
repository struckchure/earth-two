//go:build !js

package game

import "github.com/struckchure/illusion"

type touchPlugin struct{}

func (touchPlugin) Build(*illusion.App) {}

func syncTouchInteract(bool, string) {}

func syncTouchContext(bool, bool, bool, bool, bool) {}

func syncTouchHUD(float32, float32, float32, float32, float32) {}
