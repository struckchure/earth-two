//go:build !js

package game

// The GLSL version the particles' shader is written for, on the desktop.
const (
	effectsVertexHeader   = "#version 330\n"
	effectsFragmentHeader = "#version 330\n"
)
