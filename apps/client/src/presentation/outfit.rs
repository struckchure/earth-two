//! What a character wears: character/outfit.go. The wardrobe file, the
//! outfit rules, and the dress system that puts a Garment child under the
//! Body for each thing worn.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::appearance::AssetId;
use super::player::AnimationPlayer;
use super::roster::{Model, Roster, wear};
use crate::character::{Body, State};

/// Slot is a place on the body something is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Slot {
    /// A head with other features, worn in place of the body's own.
    Face,
    Hair,
    Glasses,
    /// Worn over the face: a rebreather or a dust mask.
    Mask,
    Top,
    Bottom,
    /// A top and bottom in one (a suit, overalls): wearing one takes off the
    /// Top and Bottom, and wearing either takes it off.
    OnePiece,
    /// Worn over the top and bottom, or a one-piece.
    Coat,
    Shoes,
}

pub const SLOT_COUNT: usize = 9;

impl Slot {
    pub const ALL: [Slot; SLOT_COUNT] = [
        Slot::Face,
        Slot::Hair,
        Slot::Glasses,
        Slot::Mask,
        Slot::Top,
        Slot::Bottom,
        Slot::OnePiece,
        Slot::Coat,
        Slot::Shoes,
    ];

    /// The slots' names in wardrobe.json.
    pub const KEYS: [&'static str; SLOT_COUNT] = [
        "face", "hair", "glasses", "mask", "top", "bottom", "outfit", "coat", "shoes",
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn key(self) -> &'static str {
        Slot::KEYS[self.index()]
    }

    pub fn name(self) -> &'static str {
        match self {
            Slot::Face => "Face",
            Slot::Hair => "Hair",
            Slot::Glasses => "Glasses",
            Slot::Mask => "Mask",
            Slot::Top => "Top",
            Slot::Bottom => "Bottom",
            Slot::OnePiece => "Outfit",
            Slot::Coat => "Coat",
            Slot::Shoes => "Shoes",
        }
    }
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Outfit is what a character looks like: which body (an index into
/// Roster.skins and Wardrobe.bodies), which skin tone, and what it wears. It
/// goes on the root entity; change it and dress puts it on. The default
/// Outfit is the first body, first tone, wearing nothing but underwear.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outfit {
    pub body: i32,
    pub tone: i32,
    /// Item index + 1; 0 is nothing.
    wear: [i32; SLOT_COUNT],
}

impl Outfit {
    pub fn new(body: i32, tone: i32) -> Outfit {
        Outfit {
            body,
            tone,
            wear: [0; SLOT_COUNT],
        }
    }

    /// Item returns what's worn in slot, as an index into the body's items.
    pub fn item(&self, slot: Slot) -> Option<usize> {
        let w = self.wear[slot.index()];
        (w > 0).then(|| (w - 1) as usize)
    }

    /// raw is the slot's stored value, for the appearance checks: negative
    /// is invalid.
    pub(crate) fn raw(&self, slot: Slot) -> i32 {
        self.wear[slot.index()]
    }

    /// Put wears item (an index into the body's items for slot) in slot,
    /// taking off what it replaces: a one-piece takes off the top and
    /// bottom, and a top or bottom takes off a one-piece.
    pub fn put(&mut self, slot: Slot, item: usize) {
        self.wear[slot.index()] = item as i32 + 1;
        match slot {
            Slot::OnePiece => {
                self.wear[Slot::Top.index()] = 0;
                self.wear[Slot::Bottom.index()] = 0;
            }
            Slot::Top | Slot::Bottom => self.wear[Slot::OnePiece.index()] = 0,
            _ => {}
        }
    }

    /// Remove takes off what's worn in slot.
    pub fn remove(&mut self, slot: Slot) {
        self.wear[slot.index()] = 0;
    }
}

/// Item is something that can be worn: a model rigged to its body's
/// skeleton. It hides the skin of the body regions it touches, and brings
/// back the part it doesn't cover as skin patches of its own (skin_meshes,
/// by mesh index in its model); it hides the underwear on the regions it
/// covers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    /// Stable model reference for remote appearances.
    pub asset_id: AssetId,
    pub name: String,
    /// The model's path, relative to the asset root.
    pub model: String,
    pub hides: Vec<String>,
    pub covers: Vec<String>,
    pub skin_meshes: Vec<usize>,
    /// The meshes that get no outline.
    pub unlined: BTreeSet<usize>,
}

impl Item {
    pub fn named(name: &str) -> Item {
        Item {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// Look is a faction's whole outfit, put on at once (Wardrobe::wear): what
/// it wears in each slot it names, by item index + 1, or -1 to take off
/// what's there; 0 leaves a slot as it is (the hair, the face).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub name: String,
    wear: [i32; SLOT_COUNT],
}

impl Look {
    /// new makes a look that puts on put's items (slot -> item index) and
    /// takes off what's in off.
    pub fn new(name: &str, put: &[(Slot, usize)], off: &[Slot]) -> Look {
        let mut l = Look {
            name: name.to_string(),
            wear: [0; SLOT_COUNT],
        };
        for (s, i) in put {
            l.wear[s.index()] = *i as i32 + 1;
        }
        for s in off {
            l.wear[s.index()] = -1;
        }
        l
    }
}

/// Tone is a skin: the texture that replaces the body's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tone {
    pub asset_id: AssetId,
    pub name: String,
    /// The texture's path, relative to the asset root.
    pub texture: String,
}

impl Tone {
    pub fn named(name: &str) -> Tone {
        Tone {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// BodyWardrobe is what one body can wear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyWardrobe {
    pub asset_id: AssetId,
    pub name: String,
    /// The body's skin meshes, by index in its model: the ones a Tone
    /// retextures.
    pub skin_meshes: Vec<usize>,
    /// The body's skin meshes by region, and the underwear on each region.
    pub regions: HashMap<String, Vec<usize>>,
    pub underwear: HashMap<String, Vec<usize>>,
    /// The body's eyes and eyebrows, which go with its head when a Face is
    /// worn.
    pub face_meshes: Vec<usize>,
    pub tones: Vec<Tone>,
    pub items: [Vec<Item>; SLOT_COUNT],
    /// The factions' looks.
    pub looks: Vec<Look>,
}

impl BodyWardrobe {
    pub fn items(&self, slot: Slot) -> &Vec<Item> {
        &self.items[slot.index()]
    }

    pub fn items_mut(&mut self, slot: Slot) -> &mut Vec<Item> {
        &mut self.items[slot.index()]
    }

    /// unlined are the body's meshes that get no outline: all but its skin
    /// and underwear, which leaves the eyes and eyebrows.
    pub fn unlined(&self) -> BTreeSet<usize> {
        let mut lined = BTreeSet::new();
        let mut last: i64 = -1;
        let mut mark = |meshes: &[usize]| {
            for &i in meshes {
                lined.insert(i);
                last = last.max(i as i64);
            }
        };
        mark(&self.skin_meshes);
        for meshes in self.underwear.values() {
            mark(meshes);
        }
        (0..last.max(0) as usize)
            .filter(|i| !lined.contains(i))
            .collect()
    }

    /// hidden is the body meshes o hides: the skin of the regions its items
    /// touch, and the underwear on the regions they cover.
    pub fn hidden(&self, o: &Outfit) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for (slot, item) in self.worn(o) {
            // Tops replace torso underwear, including its straps: partial
            // coverage tests otherwise keep the entire bra over open
            // necklines.
            if slot == Slot::Top || slot == Slot::OnePiece {
                out.extend(self.underwear.get("torso").into_iter().flatten());
            }
            if slot == Slot::Face {
                out.extend(&self.face_meshes);
            }
            for region in &item.hides {
                out.extend(self.regions.get(region).into_iter().flatten());
            }
            for region in &item.covers {
                out.extend(self.underwear.get(region).into_iter().flatten());
            }
        }
        out
    }

    /// worn is what o wears, slot by slot.
    pub fn worn(&self, o: &Outfit) -> Vec<(Slot, &Item)> {
        let mut out = vec![];
        for s in Slot::ALL {
            if let Some(i) = o.item(s)
                && let Some(item) = self.items(s).get(i)
            {
                out.push((s, item));
            }
        }
        out
    }
}

/// Wardrobe is a resource: what each body in the roster can wear, by the
/// same index as Roster.skins.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Wardrobe {
    pub bodies: Vec<BodyWardrobe>,
}

impl Wardrobe {
    /// Find returns the index of body's item called name in slot.
    pub fn find(&self, body: usize, slot: Slot, name: &str) -> Option<usize> {
        self.bodies[body]
            .items(slot)
            .iter()
            .position(|it| it.name == name)
    }

    /// Wear returns o in look (an index into its body's looks).
    pub fn wear(&self, mut o: Outfit, look: usize) -> Outfit {
        let l = &self.bodies[o.body as usize].looks[look];
        // Take off first, then put on, so a one-piece and a top and bottom
        // named in one look don't take each other off.
        for s in Slot::ALL {
            if l.wear[s.index()] < 0 {
                o.remove(s);
            }
        }
        for s in Slot::ALL {
            let x = l.wear[s.index()];
            if x > 0 {
                o.put(s, (x - 1) as usize);
            }
        }
        o
    }

    /// LookOf returns which of its body's looks o is wearing: the first
    /// whose every named slot matches.
    pub fn look_of(&self, o: &Outfit) -> Option<usize> {
        self.bodies[o.body as usize].looks.iter().position(|l| {
            Slot::ALL.iter().all(|s| {
                let x = l.wear[s.index()];
                let w = o.wear[s.index()];
                x == 0 || !(x < 0 && w != 0 || x > 0 && w != x)
            })
        })
    }

    /// Rebody returns o on another body: the same tone and the items that
    /// body has too, by name.
    pub fn rebody(&self, o: &Outfit, body: usize) -> Outfit {
        let tones = self.bodies[body].tones.len() as i32;
        let mut next = Outfit::new(body as i32, o.tone.min((tones - 1).max(0)));
        for s in Slot::ALL {
            let Some(i) = o.item(s) else { continue };
            let Some(item) = self.bodies[o.body as usize].items(s).get(i) else {
                continue;
            };
            if let Some(j) = self.find(body, s, &item.name) {
                next.wear[s.index()] = j as i32 + 1;
            }
        }
        next
    }

    /// Names lists slot's item names on body, for menus.
    pub fn names(&self, body: usize, slot: Slot) -> Vec<String> {
        self.bodies[body]
            .items(slot)
            .iter()
            .map(|it| it.name.clone())
            .collect()
    }
}

/// wardrobe.json, as tools/makehuman/wardrobe.py writes it.
#[derive(Deserialize)]
struct WardrobeFile {
    bodies: Vec<WardrobeBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WardrobeBody {
    name: String,
    model: String,
    #[serde(default)]
    skin_meshes: Vec<usize>,
    #[serde(default)]
    regions: HashMap<String, Vec<usize>>,
    #[serde(default)]
    face_meshes: Vec<usize>,
    #[serde(default)]
    underwear: HashMap<String, Vec<usize>>,
    #[serde(default)]
    skins: Vec<WardrobeSkin>,
    #[serde(default)]
    slots: HashMap<String, Vec<WardrobeItem>>,
    #[serde(default)]
    looks: Vec<WardrobeLook>,
}

#[derive(Deserialize)]
struct WardrobeSkin {
    name: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WardrobeItem {
    name: String,
    path: String,
    #[serde(default)]
    hides: Vec<String>,
    #[serde(default)]
    covers: Vec<String>,
    #[serde(default)]
    skin_meshes: Vec<usize>,
}

#[derive(Deserialize)]
struct WardrobeLook {
    name: String,
    #[serde(default)]
    wear: HashMap<String, Option<String>>,
}

/// load_wardrobe reads the wardrobe file's contents and takes what it lists
/// for each model in models, matched by model path. mesh_count says how many
/// meshes a model at a path has, if it's known: a face's eyes and eyebrows
/// (all but its skin meshes) go without an outline.
pub fn load_wardrobe(
    data: &str,
    models: &[Model],
    mesh_count: impl Fn(&str) -> Option<usize>,
) -> Result<Wardrobe, String> {
    let f: WardrobeFile = serde_json::from_str(data).map_err(|e| format!("wardrobe: {e}"))?;
    let mut w = Wardrobe {
        bodies: vec![BodyWardrobe::default(); models.len()],
    };
    for (i, m) in models.iter().enumerate() {
        for b in f.bodies.iter().filter(|b| b.model == m.path) {
            let bw = &mut w.bodies[i];
            bw.asset_id = AssetId::for_path(&b.model);
            bw.name = b.name.clone();
            bw.skin_meshes = b.skin_meshes.clone();
            bw.regions = b.regions.clone();
            bw.underwear = b.underwear.clone();
            bw.face_meshes = b.face_meshes.clone();
            for s in &b.skins {
                bw.tones.push(Tone {
                    asset_id: AssetId::for_path(&s.path),
                    name: s.name.clone(),
                    texture: s.path.clone(),
                });
            }
            for slot in Slot::ALL {
                for it in b.slots.get(slot.key()).into_iter().flatten() {
                    let mut item = Item {
                        asset_id: AssetId::for_path(&it.path),
                        name: it.name.clone(),
                        model: it.path.clone(),
                        hides: it.hides.clone(),
                        covers: it.covers.clone(),
                        skin_meshes: it.skin_meshes.clone(),
                        unlined: BTreeSet::new(),
                    };
                    if slot == Slot::Face
                        && let Some(n) = mesh_count(&it.path)
                    {
                        // Only a face's skin is outlined, not its eyes and
                        // eyebrows.
                        item.unlined = (0..n)
                            .filter(|mesh| !it.skin_meshes.contains(mesh))
                            .collect();
                    }
                    bw.items_mut(slot).push(item);
                }
            }
            for l in &b.looks {
                let mut look = Look {
                    name: l.name.clone(),
                    wear: [0; SLOT_COUNT],
                };
                for slot in Slot::ALL {
                    match l.wear.get(slot.key()) {
                        None => {}
                        Some(None) => look.wear[slot.index()] = -1,
                        Some(Some(name)) => {
                            let item = w.find(i, slot, name).ok_or_else(|| {
                                format!(
                                    "wardrobe: {}'s {} look wears {name:?}, which it hasn't got",
                                    b.name, l.name
                                )
                            })?;
                            look.wear[slot.index()] = item as i32 + 1;
                        }
                    }
                }
                w.bodies[i].looks.push(look);
            }
        }
    }
    Ok(w)
}

/// Garment marks an entity drawing something a character wears. It's a
/// child of the character's Body, posed like it (see mirror_pose), and
/// moves with physics where it hangs loose (see cloth::clothe).
#[derive(Component, Clone, Debug)]
pub struct Garment {
    pub slot: Slot,
    /// Its skin patches, by mesh index.
    pub skin: Vec<usize>,
    /// Shoes underneath trouser cuffs: the footwear model's path.
    pub footwear: Option<String>,
}

/// ModelPath is the model an entity draws (illusion's Model3d), by path.
/// The viewer loads it and spawns its scene under the entity.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ModelPath(pub String);

/// ModelParts changes how an entity draws some of its model's meshes, by
/// mesh index: hidden ones aren't drawn, and texture swaps a mesh's own
/// material's base colour texture for this entity alone (the skin tone).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelParts {
    pub hidden: BTreeSet<usize>,
    pub texture: BTreeMap<usize, String>,
}

/// OutlineSkip is the meshes left out of an entity's outline pass, for the
/// shading module: bodies and clothes get an outline but for these.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct OutlineSkip(pub BTreeSet<usize>);

/// skinned gives meshes the skin tone texture.
pub fn skinned(meshes: &[usize], tone: Option<&str>) -> BTreeMap<usize, String> {
    let Some(tone) = tone else {
        return BTreeMap::new();
    };
    meshes.iter().map(|&m| (m, tone.to_string())).collect()
}

/// dress puts each character's Outfit on it when the outfit changes: the
/// body, the skin tone, the body regions hidden under clothes, and a Garment
/// child of the Body for each thing worn.
#[allow(clippy::too_many_arguments)]
pub fn dress(
    mut commands: Commands,
    outfits: Query<(Entity, &Outfit, Option<&Children>)>,
    mut bodies: Query<
        (
            &mut State,
            &mut AnimationPlayer,
            &mut Transform,
            Option<&Children>,
        ),
        With<Body>,
    >,
    garments: Query<(), With<Garment>>,
    roster: Res<Roster>,
    wardrobe: Res<Wardrobe>,
    mut applied: Local<HashMap<Entity, Outfit>>,
) {
    let mut n = 0;
    for (root, o, children) in &outfits {
        n += 1;
        if applied.get(&root) == Some(o) {
            continue;
        }
        if o.body < 0
            || o.body as usize >= wardrobe.bodies.len()
            || o.body as usize >= roster.skins.len()
        {
            continue;
        }
        applied.insert(root, *o);
        let bw = &wardrobe.bodies[o.body as usize];
        for &body in children.into_iter().flatten() {
            let Ok((mut st, mut p, mut tr, kids)) = bodies.get_mut(body) else {
                continue;
            };
            if st.skin != o.body as usize {
                wear(&roster, &mut st, &mut p, &mut tr, o.body as usize);
            }
            let tone = (o.tone >= 0)
                .then(|| bw.tones.get(o.tone as usize))
                .flatten()
                .map(|t| t.texture.as_str());
            commands.entity(body).insert((
                ModelPath(roster.skins[o.body as usize].model.clone()),
                ModelParts {
                    hidden: bw.hidden(o),
                    texture: skinned(&bw.skin_meshes, tone),
                },
                OutlineSkip(bw.unlined()),
            ));
            for &child in kids.into_iter().flatten() {
                if garments.contains(child) {
                    commands.entity(child).despawn();
                }
            }
            let footwear = o
                .item(Slot::Shoes)
                .and_then(|i| bw.items(Slot::Shoes).get(i))
                .map(|it| it.model.clone());
            for (slot, item) in bw.worn(o) {
                let underfoot = (slot == Slot::Bottom || slot == Slot::OnePiece)
                    .then(|| footwear.clone())
                    .flatten();
                // Hair and glasses go without an outline: one around hair
                // cards or thin frames is a blob.
                let mut garment = commands.spawn((
                    Garment {
                        slot,
                        skin: item.skin_meshes.clone(),
                        footwear: underfoot,
                    },
                    ModelPath(item.model.clone()),
                    // Its skin patches in the body's tone.
                    ModelParts {
                        hidden: BTreeSet::new(),
                        texture: skinned(&item.skin_meshes, tone),
                    },
                    AnimationPlayer::new(&p.model),
                    Transform::IDENTITY,
                    ChildOf(body),
                ));
                if slot != Slot::Hair && slot != Slot::Glasses {
                    garment.insert(OutlineSkip(item.unlined.clone()));
                }
            }
        }
    }
    if applied.len() > n {
        // Some have gone, or taken their Outfit off: forget them.
        let kept: HashMap<Entity, Outfit> = outfits
            .iter()
            .filter_map(|(root, _, _)| applied.get(&root).map(|o| (root, *o)))
            .collect();
        *applied = kept;
    }
}

/// mirror_pose poses each garment like the body it's on: it copies the
/// body's animation player after the clocks have advanced. The copy is
/// paused so the garment isn't advanced a second time.
pub fn mirror_pose(
    mut garments: Query<(&ChildOf, &mut AnimationPlayer), With<Garment>>,
    players: Query<&AnimationPlayer, Without<Garment>>,
) {
    for (parent, mut gp) in &mut garments {
        if let Ok(bp) = players.get(parent.parent()) {
            *gp = bp.clone();
            gp.paused = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // character/outfit_test.go TestOutfitStartsBare.
    #[test]
    fn starts_bare() {
        let o = Outfit::default();
        for s in Slot::ALL {
            assert!(o.item(s).is_none(), "zero Outfit wears something in {s}");
        }
    }

    // TestOutfitOnePieceReplacesTopAndBottom.
    #[test]
    fn one_piece_replaces_top_and_bottom() {
        let mut o = Outfit::default();
        o.put(Slot::Top, 1);
        o.put(Slot::Bottom, 2);
        o.put(Slot::Shoes, 0);
        o.put(Slot::OnePiece, 3);
        assert!(o.item(Slot::Top).is_none());
        assert!(o.item(Slot::Bottom).is_none());
        assert_eq!(o.item(Slot::Shoes), Some(0));
        o.put(Slot::Bottom, 1);
        assert!(o.item(Slot::OnePiece).is_none());
        assert_eq!(o.item(Slot::Bottom), Some(1));
        o.remove(Slot::Bottom);
        assert!(o.item(Slot::Bottom).is_none());
    }

    fn item(name: &str, hides: &[&str], covers: &[&str]) -> Item {
        Item {
            name: name.into(),
            hides: hides.iter().map(|s| s.to_string()).collect(),
            covers: covers.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn test_wardrobe() -> Wardrobe {
        let mut man = BodyWardrobe {
            name: "man".into(),
            skin_meshes: vec![0, 1, 2],
            regions: HashMap::from([
                ("torso".to_string(), vec![0]),
                ("hips".to_string(), vec![1]),
                ("feet".to_string(), vec![2]),
            ]),
            underwear: HashMap::from([("hips".to_string(), vec![3])]),
            tones: vec![
                Tone::named("Dark"),
                Tone::named("Fair"),
                Tone::named("Light"),
            ],
            ..Default::default()
        };
        *man.items_mut(Slot::Top) = vec![
            item("T-shirt", &["torso"], &[]),
            item("Polo", &["torso", "hips"], &[]),
        ];
        *man.items_mut(Slot::Bottom) = vec![item("Cargo pants", &["hips"], &["hips"])];
        *man.items_mut(Slot::Hair) = vec![Item::named("Short")];
        let mut woman = BodyWardrobe {
            name: "woman".into(),
            tones: vec![Tone::named("Dark"), Tone::named("Fair")],
            ..Default::default()
        };
        *woman.items_mut(Slot::Top) = vec![Item::named("Tank top"), Item::named("T-shirt")];
        *woman.items_mut(Slot::Hair) = vec![Item::named("Bob")];
        Wardrobe {
            bodies: vec![man, woman],
        }
    }

    // TestWardrobeHidden.
    #[test]
    fn hidden_meshes() {
        let w = test_wardrobe();
        let mut o = Outfit::default();
        o.put(Slot::Top, 1);
        o.put(Slot::Hair, 0);
        assert_eq!(w.bodies[0].hidden(&o), BTreeSet::from([0, 1]));
        o.put(Slot::Bottom, 0);
        assert_eq!(w.bodies[0].hidden(&o), BTreeSet::from([0, 1, 3]));
        let worn = w.bodies[0].worn(&o);
        assert_eq!(worn.len(), 3);
        assert_eq!(worn[0].0, Slot::Hair);
        assert_eq!(worn[1].1.name, "Polo");
        assert_eq!(worn[2].0, Slot::Bottom);
    }

    // TestWardrobeRebodyKeepsWhatFits.
    #[test]
    fn rebody_keeps_what_fits() {
        let w = test_wardrobe();
        let mut o = Outfit::new(0, 2);
        o.put(Slot::Top, 0);
        o.put(Slot::Bottom, 0);
        o.put(Slot::Hair, 0);
        let got = w.rebody(&o, 1);
        assert_eq!(got.body, 1);
        assert_eq!(got.tone, 1);
        assert_eq!(got.item(Slot::Top), Some(1));
        assert!(got.item(Slot::Bottom).is_none() && got.item(Slot::Hair).is_none());
    }

    // TestSkinned.
    #[test]
    fn skinned_tone() {
        assert!(skinned(&[1, 2], None).is_empty());
        let got = skinned(&[1, 2], Some("tone.jpg"));
        assert_eq!(got.len(), 2);
        assert_eq!(got[&1], "tone.jpg");
        assert_eq!(got[&2], "tone.jpg");
    }

    // TestTopReplacesTorsoUnderwear.
    #[test]
    fn top_replaces_torso_underwear() {
        let mut b = BodyWardrobe {
            regions: HashMap::from([("torso".to_string(), vec![0])]),
            underwear: HashMap::from([
                ("torso".to_string(), vec![1]),
                ("hips".to_string(), vec![2]),
            ]),
            ..Default::default()
        };
        *b.items_mut(Slot::Top) = vec![Item::named("Tank top")];
        *b.items_mut(Slot::OnePiece) = vec![Item::named("Dress")];
        *b.items_mut(Slot::Bottom) = vec![Item::named("Shorts")];
        let mut o = Outfit::default();
        o.put(Slot::Top, 0);
        assert_eq!(b.hidden(&o), BTreeSet::from([1]));
        o.remove(Slot::Top);
        o.put(Slot::Bottom, 0);
        assert!(b.hidden(&o).is_empty());
        o.put(Slot::OnePiece, 0);
        assert_eq!(b.hidden(&o), BTreeSet::from([1]));
    }

    // TestWearLook.
    #[test]
    fn wear_look() {
        let mut b = BodyWardrobe {
            name: "man".into(),
            ..Default::default()
        };
        *b.items_mut(Slot::Hair) = vec![Item::named("Short")];
        *b.items_mut(Slot::Mask) = vec![Item::named("Rebreather")];
        *b.items_mut(Slot::Top) = vec![Item::named("T-shirt")];
        *b.items_mut(Slot::OnePiece) =
            vec![Item::named("Crew coveralls"), Item::named("Desert poncho")];
        b.looks = vec![
            Look::new("Crew", &[(Slot::OnePiece, 0)], &[Slot::Mask]),
            Look::new("Fringer", &[(Slot::OnePiece, 1), (Slot::Mask, 0)], &[]),
        ];
        let w = Wardrobe { bodies: vec![b] };
        let mut o = Outfit::default();
        o.put(Slot::Hair, 0);
        o.put(Slot::Top, 0);
        assert!(w.look_of(&o).is_none(), "a T-shirt is in a look");
        o = w.wear(o, 1);
        assert_eq!(o.item(Slot::OnePiece), Some(1));
        assert!(o.item(Slot::Mask).is_some());
        assert!(o.item(Slot::Top).is_none());
        assert_eq!(w.look_of(&o), Some(1));
        o = w.wear(o, 0);
        assert!(o.item(Slot::Mask).is_none());
        assert!(o.item(Slot::Hair).is_some());
        assert_eq!(w.look_of(&o), Some(0));
    }

    #[test]
    fn unlined_leaves_eyes_and_eyebrows() {
        let b = BodyWardrobe {
            skin_meshes: vec![2, 3, 4, 5, 6, 7, 9, 10, 11],
            underwear: HashMap::from([("hips".to_string(), vec![8])]),
            ..Default::default()
        };
        assert_eq!(b.unlined(), BTreeSet::from([0, 1]));
    }

    #[test]
    fn loads_the_real_wardrobe() {
        let data = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/characters/wardrobe.json"
        ))
        .unwrap();
        let models = [
            Model {
                path: "characters/man.glb".into(),
                ..Default::default()
            },
            Model {
                path: "characters/woman.glb".into(),
                ..Default::default()
            },
        ];
        let w = load_wardrobe(&data, &models, |_| Some(3)).unwrap();
        assert_eq!(w.bodies.len(), 2);
        assert_eq!(w.bodies[0].name, "man");
        assert_eq!(w.bodies[0].tones.len(), 6);
        assert_eq!(w.find(0, Slot::Shoes, "Boots"), Some(5));
        assert_eq!(w.bodies[0].looks.len(), 5);
        assert_eq!(w.bodies[0].looks[3].name, "Fringer");
        // A face's eyes and eyebrows go without an outline.
        assert_eq!(
            w.bodies[0].items(Slot::Face)[0].unlined,
            BTreeSet::from([0, 1])
        );
        // The Crew look takes the mask off and puts the coveralls on.
        let mut o = Outfit::default();
        o.put(Slot::Mask, 0);
        o = w.wear(o, 0);
        assert!(o.item(Slot::Mask).is_none());
        assert_eq!(
            o.item(Slot::OnePiece),
            w.find(0, Slot::OnePiece, "Crew coveralls")
        );
        assert_eq!(w.look_of(&o), Some(0));
    }
}
