//! A player's look as sent between clients: character/appearance.go, with
//! the assetref IDs it leans on.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::outfit::{Outfit, SLOT_COUNT, Slot, Wardrobe};

/// AssetId is a 64-bit path-derived identifier, encoded as 16 hex
/// characters in JSON (assetref.ID). It remains stable when an asset is
/// rebuilt. Never send process-local render handles or wardrobe array
/// indices to peers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub String);

impl AssetId {
    pub fn for_path(path: &str) -> AssetId {
        let hash = Sha256::digest(format!("earth-two:asset:v1:{path}").as_bytes());
        AssetId(hash[..8].iter().map(|b| format!("{b:02x}")).collect())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Appearance describes a player using only assets installed on each
/// client. Send it on join or wardrobe changes. Slot order is fixed by Slot;
/// a blank wear entry means nothing in that slot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    pub body: AssetId,
    pub tone: AssetId,
    pub wear: [AssetId; SLOT_COUNT],
}

impl Wardrobe {
    /// appearance converts local wardrobe indices into stable asset
    /// references.
    pub fn appearance(&self, o: &Outfit) -> Result<Appearance, String> {
        if o.body < 0 || o.body as usize >= self.bodies.len() {
            return Err(format!("invalid body index {}", o.body));
        }
        let b = &self.bodies[o.body as usize];
        if b.asset_id.is_empty()
            || o.tone < 0
            || o.tone as usize >= b.tones.len()
            || b.tones[o.tone as usize].asset_id.is_empty()
        {
            return Err("invalid or untagged body/tone".into());
        }
        let mut a = Appearance {
            body: b.asset_id.clone(),
            tone: b.tones[o.tone as usize].asset_id.clone(),
            ..Default::default()
        };
        for slot in Slot::ALL {
            if o.raw(slot) < 0 {
                return Err(format!("invalid item in {slot}"));
            }
            let Some(item) = o.item(slot) else { continue };
            match b.items(slot).get(item) {
                Some(it) if !it.asset_id.is_empty() => a.wear[slot.index()] = it.asset_id.clone(),
                _ => return Err(format!("invalid or untagged item in {slot}")),
            }
        }
        if !a.wear[Slot::OnePiece.index()].is_empty()
            && (!a.wear[Slot::Top.index()].is_empty() || !a.wear[Slot::Bottom.index()].is_empty())
        {
            return Err("outfit conflicts with top/bottom".into());
        }
        Ok(a)
    }

    /// outfit resolves a remote appearance against the loaded body's
    /// wardrobe. IDs from another body or slot are rejected, as are
    /// incompatible outfits. Array reordering between clients does not
    /// change the chosen appearance.
    pub fn outfit(&self, a: &Appearance) -> Result<Outfit, String> {
        if a.body.is_empty() || a.tone.is_empty() {
            return Err("appearance needs body and tone IDs".into());
        }
        if !a.wear[Slot::OnePiece.index()].is_empty()
            && (!a.wear[Slot::Top.index()].is_empty() || !a.wear[Slot::Bottom.index()].is_empty())
        {
            return Err("outfit conflicts with top/bottom".into());
        }
        for (body, b) in self.bodies.iter().enumerate() {
            if b.asset_id != a.body {
                continue;
            }
            let tone = b
                .tones
                .iter()
                .position(|t| t.asset_id == a.tone)
                .ok_or_else(|| format!("unknown tone ID {:?} for body", a.tone.0))?;
            let mut o = Outfit::new(body as i32, tone as i32);
            for slot in Slot::ALL {
                let id = &a.wear[slot.index()];
                if id.is_empty() {
                    continue;
                }
                let item = b
                    .items(slot)
                    .iter()
                    .position(|it| &it.asset_id == id)
                    .ok_or_else(|| format!("unknown item ID {:?} for {slot}", id.0))?;
                o.put(slot, item);
            }
            return Ok(o);
        }
        Err(format!("unknown body ID {:?}", a.body.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::outfit::{BodyWardrobe, Item, Tone};

    fn reference_wardrobe() -> Wardrobe {
        let tagged = |path: &str| Item {
            asset_id: AssetId::for_path(path),
            ..Default::default()
        };
        let mut b = BodyWardrobe {
            asset_id: AssetId::for_path("characters/man.glb"),
            tones: vec![
                Tone {
                    asset_id: AssetId::for_path("characters/skins/light.jpg"),
                    ..Default::default()
                },
                Tone {
                    asset_id: AssetId::for_path("characters/skins/dark.jpg"),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        *b.items_mut(Slot::Top) = vec![
            tagged("characters/man/top/shirt.glb"),
            tagged("characters/man/top/sweater.glb"),
        ];
        *b.items_mut(Slot::OnePiece) = vec![tagged("characters/man/outfit/suit.glb")];
        let other = BodyWardrobe {
            asset_id: AssetId::for_path("characters/woman.glb"),
            ..Default::default()
        };
        Wardrobe {
            bodies: vec![b, other],
        }
    }

    // assetref.ForPath: sha256("earth-two:asset:v1:" + path)[:8] in hex.
    #[test]
    fn asset_id_is_the_go_hash() {
        let id = AssetId::for_path("characters/man.glb");
        assert_eq!(id.0.len(), 16);
        assert!(
            id.0.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(id, AssetId::for_path("characters/woman.glb"));
        // A known vector, computed with Go's crypto/sha256.
        let digest = Sha256::digest(b"earth-two:asset:v1:characters/man.glb");
        let want: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(id.0, want);
    }

    // character/appearance_test.go TestAppearanceResolvesAcrossLocalReordering.
    #[test]
    fn resolves_across_local_reordering() {
        let sender = reference_wardrobe();
        let mut o = Outfit::new(0, 1);
        o.put(Slot::Top, 1);
        let a = sender.appearance(&o).unwrap();
        let encoded = serde_json::to_string(&a).unwrap();
        assert!(!encoded.contains("characters/") && !encoded.contains("Model"));
        let incoming: Appearance = serde_json::from_str(&encoded).unwrap();
        let mut receiver = reference_wardrobe();
        receiver.bodies.swap(0, 1);
        let b = &mut receiver.bodies[1];
        b.tones.swap(0, 1);
        b.items_mut(Slot::Top).swap(0, 1);
        let resolved = receiver.outfit(&incoming).unwrap();
        assert!(resolved.body == 1 && resolved.tone == 0 && resolved.item(Slot::Top) == Some(0));
        let roundtrip = receiver.appearance(&resolved).unwrap();
        assert_eq!(roundtrip, a);
    }

    // TestAppearanceRejectsUnknownAndIncompatibleAssets.
    #[test]
    fn rejects_unknown_and_incompatible_assets() {
        let w = reference_wardrobe();
        let mut o = Outfit::default();
        o.put(Slot::Top, 0);
        let a = w.appearance(&o).unwrap();
        let mut cases = vec![a.clone(); 5];
        cases[0].body = AssetId("missing".into());
        cases[1].tone = AssetId("missing".into());
        cases[2].wear[Slot::Top.index()] = AssetId("missing".into());
        cases[3].wear[Slot::Coat.index()] = cases[3].wear[Slot::Top.index()].clone();
        cases[4].wear[Slot::OnePiece.index()] =
            w.bodies[0].items(Slot::OnePiece)[0].asset_id.clone();
        for incoming in &cases {
            assert!(
                w.outfit(incoming).is_err(),
                "invalid appearance accepted: {incoming:?}"
            );
        }
        for outfit in [
            Outfit::new(-1, 0),
            Outfit::new(99, 0),
            Outfit::new(0, -1),
            Outfit::new(0, 99),
        ] {
            assert!(w.appearance(&outfit).is_err());
        }
        o.put(Slot::Top, 99);
        assert!(w.appearance(&o).is_err());
    }
}
