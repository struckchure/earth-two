// Package vehicle makes the vehicles drivable: the player walks up to one,
// presses E, sits in it and drives it anywhere, and presses E to get out.
//
// A drivable vehicle is a root entity with a physics.Vehicle (Jolt's wheeled
// vehicle, from illusion), its [Drivable] and its input and state; its
// chassis model is a child, posed between physics steps from
// physics.Interpolated, and each wheel model is a child of that, posed from
// physics.VehicleState so it spins, steers and rides its suspension. Parked,
// with nobody in it, a vehicle is Static: it costs nothing, and needs no
// ground colliders under it. Getting in makes it Dynamic.
//
// How a vehicle is built (its wheels, seat and chassis) comes from its piece
// in world.json, as a [Spec] (see tools/world/vehicles.py); how it handles
// is a Go table ([Handlings]), so tuning needs no rebuild of the assets.
package vehicle

import (
	rl "github.com/gen2brain/raylib-go/raylib"
)

// Spec is how a piece drives, in the piece's own frame (front toward +Z,
// standing on y = 0): world.json's "vehicle" for it.
type Spec struct {
	// Handling names its entry in Handlings.
	Handling string `json:"handling"`
	// CenterOfMass is where its weight is, relative to its origin; zero puts
	// it at the middle of the chassis.
	CenterOfMass [3]float32 `json:"centerOfMass"`
	// Camera is how far behind the chase camera follows; 0 picks a distance
	// from its size.
	Camera float32 `json:"camera"`
	// Chassis is the boxes it collides with, which clear its wheels: the
	// body rides on the wheels' suspension, and the wheels find the ground
	// by themselves.
	Chassis []Box       `json:"chassis"`
	Wheels  []WheelSpec `json:"wheels"`
	Seats   []SeatSpec  `json:"seats"`
	// Headlamps are mounts on the bodywork. Omitted mounts are inferred
	// from the chassis for vehicles without authored lamp positions.
	Headlamps []HeadlampSpec `json:"headlamps"`
}

// HeadlampSpec is a lamp's lens centre and radius, in metres.
type HeadlampSpec struct {
	At     [3]float32 `json:"at"`
	Radius float32    `json:"radius"`
}

// Box is a box of a vehicle's chassis: its middle, its full size along its
// own axes and how it's turned (as world.Collider).
type Box struct {
	Center   [3]float32 `json:"center"`
	Size     [3]float32 `json:"size"`
	Rotation [4]float32 `json:"rotation"` // x, y, z, w
}

// WheelSpec is one of a vehicle's wheels.
type WheelSpec struct {
	// Piece is the wheel's own piece, modelled centred on its axle, the axle
	// along X, as a wheel on the vehicle's left (+X) side: its outer face
	// toward +X. Wheels on the right are drawn turned about.
	Piece string `json:"piece"`
	// At is the wheel's centre as the vehicle is modelled, parked.
	At            [3]float32 `json:"at"`
	Radius, Width float32
	Left          bool `json:"left"`      // on the +X side
	Steer         bool `json:"steer"`     // turns with the steering
	Drive         bool `json:"drive"`     // the engine turns it
	HandBrake     bool `json:"handBrake"` // the hand brake holds it
}

// SeatSpec is a seat.
type SeatSpec struct {
	// At is where the sitter's model origin (its feet, standing) goes; the
	// seat faces the vehicle's front.
	At [3]float32 `json:"at"`
	// Pose is "drive", at a wheel, "ride", astride, or "inside", at a wheel
	// shut in a cab: out of sight.
	Pose string `json:"pose"`
	// Exits are where the sitter can stand when getting out, tried in
	// order: on the ground beside the vehicle, clear of it.
	Exits [][3]float32 `json:"exits"`
	// Grips and Pegs are where a rider astride holds the bars and rests
	// their feet, left then right.
	Grips [][3]float32 `json:"grips"`
	Pegs  [][3]float32 `json:"pegs"`
}

func vec(a [3]float32) rl.Vector3 { return rl.Vector3{X: a[0], Y: a[1], Z: a[2]} }

func quat(a [4]float32) rl.Quaternion {
	q := rl.Quaternion{X: a[0], Y: a[1], Z: a[2], W: a[3]}
	if q == (rl.Quaternion{}) {
		return rl.QuaternionIdentity()
	}
	return q
}

// corners is every corner of the chassis boxes, for its convex hull.
func (s *Spec) corners() []rl.Vector3 {
	var out []rl.Vector3
	for _, b := range s.Chassis {
		c, q := vec(b.Center), quat(b.Rotation)
		for i := range 8 {
			d := rl.Vector3{X: b.Size[0] / 2, Y: b.Size[1] / 2, Z: b.Size[2] / 2}
			if i&1 != 0 {
				d.X = -d.X
			}
			if i&2 != 0 {
				d.Y = -d.Y
			}
			if i&4 != 0 {
				d.Z = -d.Z
			}
			out = append(out, rl.Vector3Add(c, rl.Vector3RotateByQuaternion(d, q)))
		}
	}
	return out
}

// bounds is the box around the chassis.
func (s *Spec) bounds() rl.BoundingBox {
	cs := s.corners()
	if len(cs) == 0 {
		return rl.BoundingBox{}
	}
	b := rl.BoundingBox{Min: cs[0], Max: cs[0]}
	for _, c := range cs[1:] {
		b.Min = rl.Vector3Min(b.Min, c)
		b.Max = rl.Vector3Max(b.Max, c)
	}
	return b
}

// chaseDistance is how far behind the camera follows.
func (s *Spec) chaseDistance() float32 {
	if s.Camera > 0 {
		return s.Camera
	}
	b := s.bounds()
	size := rl.Vector3Subtract(b.Max, b.Min)
	return max(4.5, 1.2*size.Z+1.5*size.Y)
}
