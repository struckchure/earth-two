// Bindpose makes skinned glTF characters play correctly in raylib.
//
// raylib reads skins differently from the glTF spec in two ways:
//
//   - It takes a skeleton's bind pose from its joints' rest transforms and
//     ignores the file's inverse bind matrices. Exporters like FBX2glTF
//     (which Quaternius's models on Poly Pizza come from) leave joints'
//     rest transforms away from the bind pose, so raylib skins those limbs
//     wrongly: arms stretch across the scene.
//   - It bakes the skinned mesh node's world transform into the vertices,
//     where glTF ignores it. Blender parents the mesh under the armature, so
//     a Mixamo rig's 0.01 scale shrinks the unposed mesh to centimetres,
//     and anything sized from its bounds goes wrong.
//
// Bindpose moves the skinned mesh to the scene root, placed where the
// skeleton's root joint says the mesh is, then rewrites every joint's rest
// transform to the pose its inverse bind matrix describes. Clips are
// untouched; a joint a clip doesn't animate now rests in the bind pose.
//
// raylib also loads only a file's first skin. Modular characters (like
// Quaternius's, with body, head, legs and feet as separate meshes) give each
// part its own skin over the same skeleton; bindpose checks the skins agree
// and merges them into one.
//
//	go run ./tools/bindpose -o assets/characters man.glb woman.glb
package main

import (
	"encoding/binary"
	"errors"
	"flag"
	"fmt"
	"log"
	"math"
	"os"
	"path/filepath"
)

func main() {
	out := flag.String("o", ".", "output directory")
	check := flag.Bool("check", false, "report each joint's distance from its bind pose instead of converting")
	flag.Parse()
	if flag.NArg() == 0 {
		log.Fatal("usage: bindpose -o dir file.glb...")
	}
	for _, in := range flag.Args() {
		if *check {
			if err := report(in); err != nil {
				log.Fatalf("%s: %v", in, err)
			}
			continue
		}
		dst := filepath.Join(*out, filepath.Base(in))
		if err := convert(in, dst); err != nil {
			log.Fatalf("%s: %v", in, err)
		}
		fmt.Println("wrote", dst)
	}
}

func convert(in, dst string) error {
	raw, err := os.ReadFile(in)
	if err != nil {
		return err
	}
	doc, bin, err := readGLB(raw)
	if err != nil {
		return err
	}
	g := &gltf{doc: doc, bin: bin}
	if err := g.restAtBind(); err != nil {
		return err
	}
	b, err := g.writeGLB()
	if err != nil {
		return err
	}
	return os.WriteFile(dst, b, 0o644)
}

// skeleton is what restAtBind and report need to know about a skin.
type skeleton struct {
	nodes  []any
	parent map[int]int
	joints []int
	ibms   []mat4       // inverse bind matrices, by joint index
	meshes []int        // the nodes with skinned meshes, all placed alike
	bind   map[int]mat4 // each joint's bind pose, in raylib's terms
}

func (s *skeleton) world(n int) mat4 {
	m := identity()
	for ; ; n = s.parent[n] {
		m = local(s.nodes[n].(obj)).mul(m)
		if _, ok := s.parent[n]; !ok {
			return m
		}
	}
}

func report(in string) error {
	raw, err := os.ReadFile(in)
	if err != nil {
		return err
	}
	doc, bin, err := readGLB(raw)
	if err != nil {
		return err
	}
	s, err := (&gltf{doc: doc, bin: bin}).skeleton()
	if err != nil {
		return err
	}
	fmt.Println(in)
	for _, j := range s.joints {
		w, b := s.world(j), s.bind[j]
		d := 0.0
		for i := range w {
			d = max(d, math.Abs(w[i]-b[i]))
		}
		name, _ := s.nodes[j].(obj)["name"].(string)
		fmt.Printf("  %-24s off by %.4g  rest at %.3f  bind at %.3f\n", name, d, w[12:15], b[12:15])
	}
	return nil
}

func (g *gltf) restAtBind() error {
	s, err := g.skeleton()
	if err != nil {
		return err
	}
	g.placeMesh(s)
	nodes, parent, joints, bind, world := s.nodes, s.parent, s.joints, s.bind, s.world

	// Parents first, so each joint's parent already has its new pose.
	isJoint := map[int]bool{}
	for _, j := range joints {
		isJoint[j] = true
	}
	done := map[int]bool{}
	var set func(j int)
	set = func(j int) {
		if done[j] {
			return
		}
		done[j] = true
		pw := identity()
		if p, ok := parent[j]; ok {
			if isJoint[p] {
				set(p)
			}
			pw = world(p)
		}
		t, r, s := pw.inverse().mul(bind[j]).decompose()
		n := nodes[j].(obj)
		delete(n, "matrix")
		n["translation"], n["rotation"], n["scale"] = anys(t[:]), anys(r[:]), anys(s[:])
	}
	for _, j := range joints {
		set(j)
	}
	return nil
}

func (g *gltf) skeleton() (*skeleton, error) {
	skins := g.list("skins")
	if len(skins) == 0 {
		return nil, errors.New("no skin")
	}
	skin := skins[0].(obj)
	joints := ints(skin["joints"])
	ibms, err := g.readMat4s(num(skin["inverseBindMatrices"]))
	if err != nil {
		return nil, err
	}
	if len(ibms) != len(joints) {
		return nil, errors.New("inverse bind matrices don't match the joints")
	}
	// Every other skin must be the same skeleton bound the same way, so its
	// meshes can use the first skin.
	for k, sk := range skins[1:] {
		other, err := g.readMat4s(num(sk.(obj)["inverseBindMatrices"]))
		if err != nil {
			return nil, err
		}
		if !equalInts(ints(sk.(obj)["joints"]), joints) || !near(other, ibms) {
			return nil, fmt.Errorf("skin %d isn't skin 0's skeleton bound the same way", k+1)
		}
	}

	nodes := g.list("nodes")
	parent := map[int]int{}
	for i, n := range nodes {
		for _, c := range ints(n.(obj)["children"]) {
			parent[c] = i
		}
	}
	s := &skeleton{nodes: nodes, parent: parent, joints: joints, ibms: ibms, bind: map[int]mat4{}}

	// raylib bakes each skinned mesh node's world transform into its
	// vertices (glTF ignores it), so each joint's bind pose is that transform
	// times the inverse of its inverse bind matrix. placeMesh puts every
	// skinned mesh at the same transform; until then, report the first's.
	for i, n := range nodes {
		if _, ok := n.(obj)["skin"]; ok {
			s.meshes = append(s.meshes, i)
		}
	}
	if len(s.meshes) == 0 {
		return nil, errors.New("no node uses the skin")
	}
	s.bindFrom(s.world(s.meshes[0]))
	return s, nil
}

func equalInts(a, b []int) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// near reports whether two lists of matrices match to within float32
// rounding, relative to their size.
func near(a, b []mat4) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		for k := range a[i] {
			if math.Abs(a[i][k]-b[i][k]) > 1e-4*max(1, math.Abs(a[i][k])) {
				return false
			}
		}
	}
	return true
}

// bindFrom sets each joint's bind pose for a mesh placed at m.
func (s *skeleton) bindFrom(m mat4) {
	for i, j := range s.joints {
		s.bind[j] = m.mul(s.ibms[i].inverse())
	}
}

// placeMesh moves the skinned mesh node to the scene root, at the transform
// that makes the root joint's bind pose its rest pose, so what raylib bakes
// into the vertices is the character as the skeleton stands.
func (g *gltf) placeMesh(s *skeleton) {
	isJoint := map[int]bool{}
	for _, j := range s.joints {
		isJoint[j] = true
	}
	root := 0 // index into joints
	for i, j := range s.joints {
		p, ok := s.parent[j]
		for ok && !isJoint[p] {
			p, ok = s.parent[p]
		}
		if !ok {
			root = i
			break
		}
	}
	at := s.world(s.joints[root]).mul(s.ibms[root])
	t, r, sc := at.decompose()

	scene := g.list("scenes")[num(g.doc["scene"])].(obj)
	for _, m := range s.meshes {
		if p, ok := s.parent[m]; ok {
			parent := s.nodes[p].(obj)
			var kept []any
			for _, c := range ints(parent["children"]) {
				if c != m {
					kept = append(kept, c)
				}
			}
			if len(kept) == 0 {
				delete(parent, "children")
			} else {
				parent["children"] = kept
			}
			delete(s.parent, m)
			scene["nodes"] = append(scene["nodes"].([]any), m)
		}
		n := s.nodes[m].(obj)
		delete(n, "matrix")
		n["translation"], n["rotation"], n["scale"] = anys(t[:]), anys(r[:]), anys(sc[:])
		n["skin"] = 0
	}
	g.doc["skins"] = g.list("skins")[:1]
	s.bindFrom(at)
}

func (g *gltf) readMat4s(i int) ([]mat4, error) {
	acc, data, stride, err := g.accessorBytes(i)
	if err != nil {
		return nil, err
	}
	if num(acc["componentType"]) != 5126 || acc["type"] != "MAT4" {
		return nil, fmt.Errorf("accessor %d isn't float MAT4", i)
	}
	if stride == 0 {
		stride = 64
	}
	out := make([]mat4, num(acc["count"]))
	for k := range out {
		for c := range 16 {
			out[k][c] = float64(math.Float32frombits(binary.LittleEndian.Uint32(data[k*stride+c*4:])))
		}
	}
	return out, nil
}

// anys stores numbers the way encoding/json decodes them, so local reads
// them back.
func anys(fs []float64) []any {
	out := make([]any, len(fs))
	for i, f := range fs {
		out[i] = f
	}
	return out
}
