package character

import (
	"fmt"

	"github.com/struckchure/earth-two/assetref"
)

// Appearance describes a player using only assets installed on each client.
// Send it on join or wardrobe changes. Positions/actions can then refer to
// the player alone; model bytes, textures and local render handles stay local.
// Slot order is fixed by Slot; a blank Wear entry means nothing in that slot.
type Appearance struct {
	Body assetref.ID            `json:"body"`
	Tone assetref.ID            `json:"tone"`
	Wear [SlotCount]assetref.ID `json:"wear"`
}

// Appearance converts local wardrobe indices into stable asset references.
func (w *Wardrobe) Appearance(o Outfit) (Appearance, error) {
	if o.Body < 0 || o.Body >= len(w.Bodies) {
		return Appearance{}, fmt.Errorf("invalid body index %d", o.Body)
	}
	b := &w.Bodies[o.Body]
	if b.AssetID == "" || o.Tone < 0 || o.Tone >= len(b.Tones) || b.Tones[o.Tone].AssetID == "" {
		return Appearance{}, fmt.Errorf("invalid or untagged body/tone")
	}
	a := Appearance{Body: b.AssetID, Tone: b.Tones[o.Tone].AssetID}
	for slot := Slot(0); slot < SlotCount; slot++ {
		if o.wear[slot] < 0 {
			return Appearance{}, fmt.Errorf("invalid item in %s", slot)
		}
		item, worn := o.Item(slot)
		if !worn {
			continue
		}
		if item < 0 || item >= len(b.Items[slot]) || b.Items[slot][item].AssetID == "" {
			return Appearance{}, fmt.Errorf("invalid or untagged item in %s", slot)
		}
		a.Wear[slot] = b.Items[slot][item].AssetID
	}
	if a.Wear[OnePiece] != "" && (a.Wear[Top] != "" || a.Wear[Bottom] != "") {
		return Appearance{}, fmt.Errorf("outfit conflicts with top/bottom")
	}
	return a, nil
}

// Outfit resolves a remote appearance against the loaded body's wardrobe.
// IDs from another body or slot are rejected, as are incompatible outfits.
// Array reordering between clients does not change the chosen appearance.
func (w *Wardrobe) Outfit(a Appearance) (Outfit, error) {
	if a.Body == "" || a.Tone == "" {
		return Outfit{}, fmt.Errorf("appearance needs body and tone IDs")
	}
	if a.Wear[OnePiece] != "" && (a.Wear[Top] != "" || a.Wear[Bottom] != "") {
		return Outfit{}, fmt.Errorf("outfit conflicts with top/bottom")
	}
	for body, b := range w.Bodies {
		if b.AssetID != a.Body {
			continue
		}
		o := Outfit{Body: body, Tone: -1}
		for tone, t := range b.Tones {
			if t.AssetID == a.Tone {
				o.Tone = tone
				break
			}
		}
		if o.Tone < 0 {
			return Outfit{}, fmt.Errorf("unknown tone ID %q for body", a.Tone)
		}
		for slot := Slot(0); slot < SlotCount; slot++ {
			id := a.Wear[slot]
			if id == "" {
				continue
			}
			found := false
			for item, it := range b.Items[slot] {
				if it.AssetID == id {
					o.Put(slot, item)
					found = true
					break
				}
			}
			if !found {
				return Outfit{}, fmt.Errorf("unknown item ID %q for %s", id, slot)
			}
		}
		return o, nil
	}
	return Outfit{}, fmt.Errorf("unknown body ID %q", a.Body)
}
