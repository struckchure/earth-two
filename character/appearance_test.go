package character

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/struckchure/earth-two/assetref"
)

func referenceWardrobe() *Wardrobe {
	b := BodyWardrobe{AssetID: assetref.ForPath("characters/man.glb")}
	b.Tones = []Tone{{AssetID: assetref.ForPath("characters/skins/light.jpg")}, {AssetID: assetref.ForPath("characters/skins/dark.jpg")}}
	b.Items[Top] = []Item{{AssetID: assetref.ForPath("characters/man/top/shirt.glb")}, {AssetID: assetref.ForPath("characters/man/top/sweater.glb")}}
	b.Items[OnePiece] = []Item{{AssetID: assetref.ForPath("characters/man/outfit/suit.glb")}}
	other := BodyWardrobe{AssetID: assetref.ForPath("characters/woman.glb")}
	return &Wardrobe{Bodies: []BodyWardrobe{b, other}}
}

func TestAppearanceResolvesAcrossLocalReordering(t *testing.T) {
	sender := referenceWardrobe()
	o := Outfit{Body: 0, Tone: 1}
	o.Put(Top, 1)
	a, err := sender.Appearance(o)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(a)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(encoded), "characters/") || strings.Contains(string(encoded), "Model") {
		t.Fatal("appearance sent paths or handles")
	}
	var incoming Appearance
	if err := json.Unmarshal(encoded, &incoming); err != nil {
		t.Fatal(err)
	}
	receiver := referenceWardrobe()
	receiver.Bodies[0], receiver.Bodies[1] = receiver.Bodies[1], receiver.Bodies[0]
	b := &receiver.Bodies[1]
	b.Tones[0], b.Tones[1] = b.Tones[1], b.Tones[0]
	b.Items[Top][0], b.Items[Top][1] = b.Items[Top][1], b.Items[Top][0]
	resolved, err := receiver.Outfit(incoming)
	if err != nil {
		t.Fatal(err)
	}
	item, worn := resolved.Item(Top)
	if resolved.Body != 1 || resolved.Tone != 0 || !worn || item != 0 {
		t.Fatalf("wrong local outfit: %+v", resolved)
	}
	roundtrip, err := receiver.Appearance(resolved)
	if err != nil || roundtrip != a {
		t.Fatal("resolved appearance differs from sender")
	}
}

func TestAppearanceRejectsUnknownAndIncompatibleAssets(t *testing.T) {
	w := referenceWardrobe()
	o := Outfit{}
	o.Put(Top, 0)
	a, err := w.Appearance(o)
	if err != nil {
		t.Fatal(err)
	}
	cases := []Appearance{a, a, a, a, a}
	cases[0].Body = "missing"
	cases[1].Tone = "missing"
	cases[2].Wear[Top] = "missing"
	cases[3].Wear[Coat] = cases[3].Wear[Top] // valid ID, wrong slot
	cases[4].Wear[OnePiece] = w.Bodies[0].Items[OnePiece][0].AssetID
	for _, incoming := range cases {
		if _, err := w.Outfit(incoming); err == nil {
			t.Fatalf("invalid appearance accepted: %+v", incoming)
		}
	}
	for _, outfit := range []Outfit{{Body: -1}, {Body: 99}, {Tone: -1}, {Tone: 99}} {
		if _, err := w.Appearance(outfit); err == nil {
			t.Fatal("invalid local outfit accepted")
		}
	}
	o.Put(Top, 99)
	if _, err := w.Appearance(o); err == nil {
		t.Fatal("invalid item accepted")
	}
}
