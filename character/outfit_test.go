package character

import (
	"reflect"
	"testing"

	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
)

func TestOutfitStartsBare(t *testing.T) {
	var o Outfit
	for s := range SlotCount {
		if _, ok := o.Item(s); ok {
			t.Errorf("zero Outfit wears something in %v", s)
		}
	}
}

func TestOutfitOnePieceReplacesTopAndBottom(t *testing.T) {
	var o Outfit
	o.Put(Top, 1)
	o.Put(Bottom, 2)
	o.Put(Shoes, 0)
	o.Put(OnePiece, 3)
	if _, ok := o.Item(Top); ok {
		t.Error("a one-piece left the top on")
	}
	if _, ok := o.Item(Bottom); ok {
		t.Error("a one-piece left the bottom on")
	}
	if i, ok := o.Item(Shoes); !ok || i != 0 {
		t.Errorf("a one-piece took the shoes: %d %v", i, ok)
	}
	o.Put(Bottom, 1)
	if _, ok := o.Item(OnePiece); ok {
		t.Error("a bottom left the one-piece on")
	}
	if i, ok := o.Item(Bottom); !ok || i != 1 {
		t.Errorf("bottom = %d %v, want 1", i, ok)
	}
	o.Remove(Bottom)
	if _, ok := o.Item(Bottom); ok {
		t.Error("Remove left the bottom on")
	}
}

func testWardrobe() *Wardrobe {
	man := BodyWardrobe{
		Name:       "man",
		SkinMeshes: []int{0, 1, 2},
		Regions:    map[string][]int{"torso": {0}, "hips": {1}, "feet": {2}},
		Underwear:  map[string][]int{"hips": {3}},
		Tones:      []Tone{{Name: "Dark"}, {Name: "Fair"}, {Name: "Light"}},
	}
	man.Items[Top] = []Item{{Name: "T-shirt", Hides: []string{"torso"}}, {Name: "Polo", Hides: []string{"torso", "hips"}}}
	man.Items[Bottom] = []Item{{Name: "Cargo pants", Hides: []string{"hips"}, Covers: []string{"hips"}}}
	man.Items[Hair] = []Item{{Name: "Short"}}
	woman := BodyWardrobe{Name: "woman", Tones: []Tone{{Name: "Dark"}, {Name: "Fair"}}}
	woman.Items[Top] = []Item{{Name: "Tank top"}, {Name: "T-shirt"}}
	woman.Items[Hair] = []Item{{Name: "Bob"}}
	return &Wardrobe{Bodies: []BodyWardrobe{man, woman}}
}

func TestWardrobeHidden(t *testing.T) {
	w := testWardrobe()
	var o Outfit
	o.Put(Top, 1)
	o.Put(Hair, 0)
	// The polo touches the torso and hips, so it hides their skin, but it
	// doesn't cover the hips, so the underwear on them stays.
	want := map[int]bool{0: true, 1: true}
	if got := w.Bodies[0].hidden(o); !reflect.DeepEqual(got, want) {
		t.Errorf("polo: hidden = %v, want %v", got, want)
	}
	// The pants cover the hips, underwear and all; the hair hides nothing.
	o.Put(Bottom, 0)
	want = map[int]bool{0: true, 1: true, 3: true}
	if got := w.Bodies[0].hidden(o); !reflect.DeepEqual(got, want) {
		t.Errorf("polo and pants: hidden = %v, want %v", got, want)
	}
	worn := w.Bodies[0].worn(o)
	if len(worn) != 3 || worn[0].slot != Hair || worn[1].item.Name != "Polo" || worn[2].slot != Bottom {
		t.Errorf("worn = %+v", worn)
	}
}

func TestWardrobeRebodyKeepsWhatFits(t *testing.T) {
	w := testWardrobe()
	o := Outfit{Tone: 2}
	o.Put(Top, 0)    // T-shirt: the woman has one
	o.Put(Bottom, 0) // Cargo pants: she doesn't
	o.Put(Hair, 0)   // Short: she doesn't
	got := w.Rebody(o, 1)
	if got.Body != 1 {
		t.Errorf("body = %d, want 1", got.Body)
	}
	if got.Tone != 1 {
		t.Errorf("tone = %d, want 1 (the woman's last)", got.Tone)
	}
	if i, ok := got.Item(Top); !ok || i != 1 {
		t.Errorf("top = %d %v, want her T-shirt, 1", i, ok)
	}
	for _, s := range []Slot{Bottom, Hair} {
		if _, ok := got.Item(s); ok {
			t.Errorf("%v carried over though she has no such item", s)
		}
	}
}

func TestSkinned(t *testing.T) {
	if got := skinned([]int{1, 2}, asset.Handle[render.Texture]{}); len(got) != 0 {
		t.Errorf("no tone: %v, want nothing swapped", got)
	}
	var store asset.Assets[render.Texture]
	h := store.Add(render.Texture{})
	got := skinned([]int{1, 2}, h)
	if len(got) != 2 || got[1] != h || got[2] != h {
		t.Errorf("skinned = %v, want meshes 1 and 2 in the tone", got)
	}
}
