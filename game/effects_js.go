//go:build js

package game

// The GLSL version the particles' shader is written for, in the browser
// (WebGL 2), which also wants a default precision.
const (
	effectsVertexHeader   = "#version 300 es\n"
	effectsFragmentHeader = "#version 300 es\nprecision highp float;\n"
)
