package character

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"

	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Slot is a place on the body something is worn.
type Slot uint8

const (
	// Face is a head with other features, worn in place of the body's own.
	Face Slot = iota
	Hair
	Glasses
	Top
	Bottom
	// OnePiece is a top and bottom in one (a suit, overalls): wearing one
	// takes off the Top and Bottom, and wearing either takes it off.
	OnePiece
	Shoes
	SlotCount
)

// slotKeys are the slots' names in wardrobe.json.
var slotKeys = [SlotCount]string{"face", "hair", "glasses", "top", "bottom", "outfit", "shoes"}

func (s Slot) String() string {
	switch s {
	case Face:
		return "Face"
	case Hair:
		return "Hair"
	case Glasses:
		return "Glasses"
	case Top:
		return "Top"
	case Bottom:
		return "Bottom"
	case OnePiece:
		return "Outfit"
	case Shoes:
		return "Shoes"
	}
	return "?"
}

// Outfit is what a character looks like: which body (an index into
// Roster.Skins and Wardrobe.Bodies), which skin tone, and what it wears. It
// goes on the root entity; change it and dress puts it on. The zero Outfit
// is the first body, first tone, wearing nothing but underwear.
type Outfit struct {
	Body int
	Tone int
	wear [SlotCount]int // item index + 1; 0 is nothing
}

// Item returns what's worn in slot, as an index into the body's items.
func (o Outfit) Item(slot Slot) (int, bool) { return o.wear[slot] - 1, o.wear[slot] > 0 }

// Put wears item (an index into the body's items for slot) in slot, taking
// off what it replaces: a one-piece takes off the top and bottom, and a top
// or bottom takes off a one-piece.
func (o *Outfit) Put(slot Slot, item int) {
	o.wear[slot] = item + 1
	switch slot {
	case OnePiece:
		o.wear[Top], o.wear[Bottom] = 0, 0
	case Top, Bottom:
		o.wear[OnePiece] = 0
	}
}

// Remove takes off what's worn in slot.
func (o *Outfit) Remove(slot Slot) { o.wear[slot] = 0 }

// Item is something that can be worn: a model rigged to its body's
// skeleton. It hides the skin of the body regions it touches, and brings back
// the part it doesn't cover as skin patches of its own (SkinMeshes, by mesh
// index in its model); it hides the underwear on the regions it Covers.
type Item struct {
	Name       string
	Model      asset.Handle[render.Model]
	Hides      []string
	Covers     []string
	SkinMeshes []int

	unlined map[int]bool // the meshes that get no outline
}

// Tone is a skin: the texture that replaces the body's.
type Tone struct {
	Name    string
	Texture asset.Handle[render.Texture]
}

// BodyWardrobe is what one body can wear.
type BodyWardrobe struct {
	Name string
	// SkinMeshes are the body's skin meshes, by index in its model: the ones
	// a Tone retextures.
	SkinMeshes []int
	// Regions are the body's skin meshes by region, and Underwear the
	// underwear on each region.
	Regions   map[string][]int
	Underwear map[string][]int
	// FaceMeshes are the body's eyes and eyebrows, which go with its head
	// when a Face is worn.
	FaceMeshes []int
	Tones      []Tone
	Items      [SlotCount][]Item
}

// Wardrobe is a resource: what each body in the roster can wear, by the
// same index as Roster.Skins.
type Wardrobe struct {
	Bodies []BodyWardrobe

	outline func(skip map[int]bool) render.Pass // see Plugin.Outline
}

// unlined are the body's meshes that get no outline: all but its skin and
// underwear, which leaves the eyes and eyebrows.
func (b *BodyWardrobe) unlined() map[int]bool {
	lined := map[int]bool{}
	last := -1
	mark := func(meshes []int) {
		for _, i := range meshes {
			lined[i] = true
			last = max(last, i)
		}
	}
	mark(b.SkinMeshes)
	for _, meshes := range b.Underwear {
		mark(meshes)
	}
	skip := map[int]bool{}
	for i := 0; i < last; i++ {
		if !lined[i] {
			skip[i] = true
		}
	}
	return skip
}

// Find returns the index of body's item called name in slot.
func (w *Wardrobe) Find(body int, slot Slot, name string) (int, bool) {
	for i, it := range w.Bodies[body].Items[slot] {
		if it.Name == name {
			return i, true
		}
	}
	return 0, false
}

// Rebody returns o on another body: the same tone and the items that body
// has too, by name.
func (w *Wardrobe) Rebody(o Outfit, body int) Outfit {
	next := Outfit{Body: body, Tone: min(o.Tone, max(len(w.Bodies[body].Tones)-1, 0))}
	for s := range SlotCount {
		i, ok := o.Item(s)
		if !ok || i >= len(w.Bodies[o.Body].Items[s]) {
			continue
		}
		if j, ok := w.Find(body, s, w.Bodies[o.Body].Items[s][i].Name); ok {
			next.wear[s] = j + 1
		}
	}
	return next
}

// hidden is the body meshes o hides: the skin of the regions its items
// touch, and the underwear on the regions they cover.
func (b *BodyWardrobe) hidden(o Outfit) map[int]bool {
	out := map[int]bool{}
	for _, worn := range b.worn(o) {
		// Tops replace torso underwear, including its straps: partial
		// coverage tests otherwise keep the entire bra over open necklines.
		if worn.slot == Top || worn.slot == OnePiece {
			for _, mesh := range b.Underwear["torso"] {
				out[mesh] = true
			}
		}
		if worn.slot == Face {
			for _, mesh := range b.FaceMeshes {
				out[mesh] = true
			}
		}
		for _, region := range worn.item.Hides {
			for _, mesh := range b.Regions[region] {
				out[mesh] = true
			}
		}
		for _, region := range worn.item.Covers {
			for _, mesh := range b.Underwear[region] {
				out[mesh] = true
			}
		}
	}
	return out
}

// worn is what o wears, slot by slot.
func (b *BodyWardrobe) worn(o Outfit) []wornItem {
	var out []wornItem
	for s := range SlotCount {
		if i, ok := o.Item(s); ok && i < len(b.Items[s]) {
			out = append(out, wornItem{s, b.Items[s][i]})
		}
	}
	return out
}

type wornItem struct {
	slot Slot
	item Item
}

// wardrobeFile is wardrobe.json, as tools/makehuman/wardrobe.py writes it.
type wardrobeFile struct {
	Bodies []struct {
		Name       string           `json:"name"`
		Model      string           `json:"model"`
		SkinMeshes []int            `json:"skinMeshes"`
		Regions    map[string][]int `json:"regions"`
		FaceMeshes []int            `json:"faceMeshes"`
		Underwear  map[string][]int `json:"underwear"`
		Skins      []struct {
			Name string `json:"name"`
			Path string `json:"path"`
		} `json:"skins"`
		Slots map[string][]struct {
			Name       string   `json:"name"`
			Path       string   `json:"path"`
			Hides      []string `json:"hides"`
			Covers     []string `json:"covers"`
			SkinMeshes []int    `json:"skinMeshes"`
		} `json:"slots"`
	} `json:"bodies"`
}

// loadWardrobe reads the wardrobe file at path (under the asset root) and
// loads what it lists for each model in models, matched by model path.
func loadWardrobe(
	path string,
	models []Model,
	settings *asset.Settings,
	modelLoader *asset.Loader[render.Model],
	textures *asset.Loader[render.Texture],
) (*Wardrobe, error) {
	root := "assets"
	if settings != nil && settings.Root != "" {
		root = settings.Root
	}
	data, err := os.ReadFile(filepath.Join(root, path))
	if err != nil {
		return nil, err
	}
	var f wardrobeFile
	if err := json.Unmarshal(data, &f); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	w := &Wardrobe{Bodies: make([]BodyWardrobe, len(models))}
	for i, m := range models {
		for _, b := range f.Bodies {
			if b.Model != m.Path {
				continue
			}
			bw := &w.Bodies[i]
			bw.Name, bw.SkinMeshes, bw.Regions, bw.Underwear = b.Name, b.SkinMeshes, b.Regions, b.Underwear
			bw.FaceMeshes = b.FaceMeshes
			for _, s := range b.Skins {
				bw.Tones = append(bw.Tones, Tone{Name: s.Name, Texture: textures.MustLoad(s.Path)})
			}
			for s, key := range slotKeys {
				for _, it := range b.Slots[key] {
					item := Item{
						Name: it.Name, Model: modelLoader.MustLoad(it.Path),
						Hides: it.Hides, Covers: it.Covers, SkinMeshes: it.SkinMeshes,
					}
					if Slot(s) == Face {
						// Only a face's skin is outlined, not its eyes and
						// eyebrows.
						item.unlined = map[int]bool{}
						if m := modelLoader.Get(item.Model); m != nil {
							for mesh := range int(m.MeshCount) {
								item.unlined[mesh] = !slices.Contains(it.SkinMeshes, mesh)
							}
						}
					}
					bw.Items[s] = append(bw.Items[s], item)
				}
			}
		}
	}
	return w, nil
}

// Garment marks an entity drawing something a character wears. It's a
// child of the character's Body, posed like it (see mirrorPose), and moves
// with physics where it hangs loose (see clothe).
type Garment struct {
	Slot     Slot
	skin     []int                      // its skin patches, by mesh index
	footwear asset.Handle[render.Model] // shoes underneath trouser cuffs
}

// dress puts each character's Outfit on it when the outfit changes: the
// body, the skin tone, the body regions hidden under clothes, and a Garment
// child of the Body for each thing worn.
func dress(
	cmd *illusion.Commands,
	outfits *illusion.Query1[Outfit],
	bodies *illusion.Query4Where[State, render.Model3d, render.AnimationPlayer, transform.Transform, illusion.With[Body]],
	garments *illusion.Query0Where[illusion.With[Garment]],
	hier *illusion.Hierarchy,
	roster *illusion.Res[Roster],
	wardrobe *illusion.Res[Wardrobe],
	applied *illusion.Local[map[ecs.Entity]Outfit],
) {
	w := wardrobe.Get()
	r := roster.Get()
	done := applied.Get()
	if *done == nil {
		*done = map[ecs.Entity]Outfit{}
	}
	n := 0
	outfits.Each(func(root ecs.Entity, o *Outfit) {
		n++
		if last, ok := (*done)[root]; ok && last == *o {
			return
		}
		if o.Body < 0 || o.Body >= len(w.Bodies) || o.Body >= len(r.Skins) {
			return
		}
		(*done)[root] = *o
		bw := &w.Bodies[o.Body]
		hier.EachChild(root, func(body ecs.Entity) {
			st, m, p, tr, ok := bodies.Get(body)
			if !ok {
				return
			}
			if st.skin != o.Body {
				r.Wear(st, m, p, tr, o.Body)
			}
			var tone asset.Handle[render.Texture]
			if o.Tone >= 0 && o.Tone < len(bw.Tones) {
				tone = bw.Tones[o.Tone].Texture
			}
			cmd.Entity(body).Insert(illusion.C(render.ModelParts{Hidden: bw.hidden(*o), Texture: skinned(bw.SkinMeshes, tone)}))
			if w.outline != nil {
				cmd.Entity(body).Insert(illusion.C(render.Passes{w.outline(bw.unlined())}))
			}

			hier.EachChild(body, func(child ecs.Entity) {
				if garments.Contains(child) {
					cmd.Despawn(child)
				}
			})
			var footwear asset.Handle[render.Model]
			if i, ok := o.Item(Shoes); ok && i < len(bw.Items[Shoes]) {
				footwear = bw.Items[Shoes][i].Model
			}
			for _, worn := range bw.worn(*o) {
				var underfoot asset.Handle[render.Model]
				if worn.slot == Bottom || worn.slot == OnePiece {
					underfoot = footwear
				}
				// Hair and glasses go without an outline: one around hair cards
				// or thin frames is a blob.
				var passes render.Passes
				if w.outline != nil && worn.slot != Hair && worn.slot != Glasses {
					passes = render.Passes{w.outline(worn.item.unlined)}
				}
				cmd.Spawn(
					illusion.C(passes),
					illusion.C(Garment{Slot: worn.slot, skin: worn.item.SkinMeshes, footwear: underfoot}),
					illusion.C(render.Model3d{Model: worn.item.Model}),
					// Its skin patches in the body's tone.
					illusion.C(render.ModelParts{Texture: skinned(worn.item.SkinMeshes, tone)}),
					illusion.C(render.AnimationPlayer{Animations: p.Animations}),
					illusion.C(transform.Identity()),
				).ChildOf(body)
			}
		})
	})
	if len(*done) > n {
		// Some have gone, or taken their Outfit off: forget them.
		kept := make(map[ecs.Entity]Outfit, n)
		outfits.Each(func(root ecs.Entity, _ *Outfit) {
			if o, ok := (*done)[root]; ok {
				kept[root] = o
			}
		})
		*done = kept
	}
}

// skinned gives meshes the skin tone texture.
func skinned(meshes []int, tone asset.Handle[render.Texture]) map[int]asset.Handle[render.Texture] {
	out := make(map[int]asset.Handle[render.Texture], len(meshes))
	if tone.IsZero() {
		return out
	}
	for _, mesh := range meshes {
		out[mesh] = tone
	}
	return out
}

// mirrorPose poses each garment like the body it's on: it copies the body's
// animation player after render.Animate has advanced it. The copy is paused
// so the garment isn't advanced a second time.
func mirrorPose(
	garments *illusion.Query1Where[render.AnimationPlayer, illusion.With[Garment]],
	players *illusion.Query1[render.AnimationPlayer],
	hier *illusion.Hierarchy,
) {
	garments.Each(func(e ecs.Entity, gp *render.AnimationPlayer) {
		body, ok := hier.Parent(e)
		if !ok {
			return
		}
		if bp, ok := players.Get(body); ok {
			*gp = *bp
			gp.Paused = true
		}
	})
}

// Names lists slot's item names on body, for menus.
func (w *Wardrobe) Names(body int, slot Slot) []string {
	items := w.Bodies[body].Items[slot]
	out := make([]string, len(items))
	for i, it := range items {
		out[i] = it.Name
	}
	return out
}
