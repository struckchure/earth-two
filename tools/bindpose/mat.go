package main

import "math"

// mat4 is column-major, like glTF.
type mat4 [16]float64

func identity() mat4 { return mat4{0: 1, 5: 1, 10: 1, 15: 1} }

// local is a node's transform relative to its parent.
func local(n obj) mat4 {
	if m := floats(n["matrix"]); len(m) == 16 {
		return mat4(m)
	}
	t := floats(n["translation"], 0, 0, 0)
	r := floats(n["rotation"], 0, 0, 0, 1)
	s := floats(n["scale"], 1, 1, 1)
	x, y, z, w := r[0], r[1], r[2], r[3]
	m := identity()
	m[0] = (1 - 2*(y*y+z*z)) * s[0]
	m[1] = 2 * (x*y + z*w) * s[0]
	m[2] = 2 * (x*z - y*w) * s[0]
	m[4] = 2 * (x*y - z*w) * s[1]
	m[5] = (1 - 2*(x*x+z*z)) * s[1]
	m[6] = 2 * (y*z + x*w) * s[1]
	m[8] = 2 * (x*z + y*w) * s[2]
	m[9] = 2 * (y*z - x*w) * s[2]
	m[10] = (1 - 2*(x*x+y*y)) * s[2]
	m[12], m[13], m[14] = t[0], t[1], t[2]
	return m
}

func (a mat4) mul(b mat4) mat4 {
	var m mat4
	for c := range 4 {
		for r := range 4 {
			for k := range 4 {
				m[c*4+r] += a[k*4+r] * b[c*4+k]
			}
		}
	}
	return m
}

func (a mat4) point(v [3]float64) [3]float64 {
	return [3]float64{
		a[0]*v[0] + a[4]*v[1] + a[8]*v[2] + a[12],
		a[1]*v[0] + a[5]*v[1] + a[9]*v[2] + a[13],
		a[2]*v[0] + a[6]*v[1] + a[10]*v[2] + a[14],
	}
}

func (a mat4) vector(v [3]float64) [3]float64 {
	return [3]float64{
		a[0]*v[0] + a[4]*v[1] + a[8]*v[2],
		a[1]*v[0] + a[5]*v[1] + a[9]*v[2],
		a[2]*v[0] + a[6]*v[1] + a[10]*v[2],
	}
}

// inverse inverts an affine transform.
func (a mat4) inverse() mat4 {
	c00 := a[5]*a[10] - a[6]*a[9]
	c01 := a[2]*a[9] - a[1]*a[10]
	c02 := a[1]*a[6] - a[2]*a[5]
	det := a[0]*c00 + a[4]*c01 + a[8]*c02
	d := 1 / det
	m := identity()
	m[0], m[1], m[2] = c00*d, c01*d, c02*d
	m[4] = (a[6]*a[8] - a[4]*a[10]) * d
	m[5] = (a[0]*a[10] - a[2]*a[8]) * d
	m[6] = (a[2]*a[4] - a[0]*a[6]) * d
	m[8] = (a[4]*a[9] - a[5]*a[8]) * d
	m[9] = (a[1]*a[8] - a[0]*a[9]) * d
	m[10] = (a[0]*a[5] - a[1]*a[4]) * d
	t := m.vector([3]float64{a[12], a[13], a[14]})
	m[12], m[13], m[14] = -t[0], -t[1], -t[2]
	return m
}

// transpose3 transposes the rotation-scale part and drops the translation.
func (a mat4) transpose3() mat4 {
	m := identity()
	for r := range 3 {
		for c := range 3 {
			m[c*4+r] = a[r*4+c]
		}
	}
	return m
}

func normalize(v [3]float64) [3]float64 {
	l := math.Sqrt(v[0]*v[0] + v[1]*v[1] + v[2]*v[2])
	if l == 0 {
		return v
	}
	return [3]float64{v[0] / l, v[1] / l, v[2] / l}
}

// decompose splits an affine transform without shear into a translation, a
// rotation quaternion (x, y, z, w) and a scale.
func (a mat4) decompose() (t [3]float64, r [4]float64, s [3]float64) {
	t = [3]float64{a[12], a[13], a[14]}
	for c := range 3 {
		s[c] = math.Sqrt(a[c*4]*a[c*4] + a[c*4+1]*a[c*4+1] + a[c*4+2]*a[c*4+2])
	}
	if a.det3() < 0 {
		s[0] = -s[0]
	}
	// m is the pure rotation, m[r][c].
	var m [3][3]float64
	for c := range 3 {
		for row := range 3 {
			m[row][c] = a[c*4+row] / s[c]
		}
	}
	switch tr := m[0][0] + m[1][1] + m[2][2]; {
	case tr > 0:
		q := math.Sqrt(tr+1) * 2
		r = [4]float64{(m[2][1] - m[1][2]) / q, (m[0][2] - m[2][0]) / q, (m[1][0] - m[0][1]) / q, q / 4}
	case m[0][0] > m[1][1] && m[0][0] > m[2][2]:
		q := math.Sqrt(1+m[0][0]-m[1][1]-m[2][2]) * 2
		r = [4]float64{q / 4, (m[0][1] + m[1][0]) / q, (m[0][2] + m[2][0]) / q, (m[2][1] - m[1][2]) / q}
	case m[1][1] > m[2][2]:
		q := math.Sqrt(1+m[1][1]-m[0][0]-m[2][2]) * 2
		r = [4]float64{(m[0][1] + m[1][0]) / q, q / 4, (m[1][2] + m[2][1]) / q, (m[0][2] - m[2][0]) / q}
	default:
		q := math.Sqrt(1+m[2][2]-m[0][0]-m[1][1]) * 2
		r = [4]float64{(m[0][2] + m[2][0]) / q, (m[1][2] + m[2][1]) / q, q / 4, (m[1][0] - m[0][1]) / q}
	}
	n := math.Sqrt(r[0]*r[0] + r[1]*r[1] + r[2]*r[2] + r[3]*r[3])
	for i := range r {
		r[i] /= n
	}
	return t, r, s
}

func (a mat4) det3() float64 {
	return a[0]*(a[5]*a[10]-a[6]*a[9]) - a[4]*(a[1]*a[10]-a[2]*a[9]) + a[8]*(a[1]*a[6]-a[2]*a[5])
}
