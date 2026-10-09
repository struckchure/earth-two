//! The maps' data (`game/maps.go`): a minimap in the top left corner in
//! play, a compass along the top, and the full map, which M opens and
//! closes. Both maps are drawn from the layout, each piece as the
//! footprint of its colliders, coloured by the part of the world it's
//! from. The minimap turns with the camera, so what's ahead is up; the
//! full map has north (the game's -Z) up.
//!
//! A click on the full map marks a destination (another click on it takes
//! it off). It's shown on both maps, on the compass and over the world,
//! with how far it is, until the player gets there.
//!
//! This is the data and the maths: the marks, their index and the minimap's
//! selection (`marksNear`), the frame's projection, clicks, headings and
//! distances. Drawing them is the UI port's.

use std::collections::HashMap;

use bevy::prelude::*;
use earth_two_world::{
    kit::{Kit, Placement, quarter_turns},
    terrain::{FLOORED, HAVEN_AT, HOLD_AT, PADS_AT, WORLD_SIZE},
};

use super::ground::{Rectangle, footprint, hypot};

/// Sizes, in points, and how much of the world the minimap shows.
pub const MINIMAP_SIZE: f32 = 150.0;
/// Metres across.
pub const MINIMAP_RANGE: f32 = 110.0;
pub const COMPASS_WIDTH: f32 = 420.0;
/// Degrees across the compass.
pub const COMPASS_SPAN: f32 = 180.0;

/// How near the destination, in metres, the player has to get for it to
/// be reached, and taken off the map.
pub const ARRIVED: f32 = 6.0;
/// How near a click has to be to the destination's mark, in points, to
/// take it off rather than move it.
pub const MARK_REACH: f32 = 14.0;

/// Full map zoom, in pixels per metre at the UI's scale of 1.
pub const MAP_ZOOM_MIN: f32 = 0.03; // the whole region
pub const MAP_ZOOM_MAX: f32 = 16.0;
pub const MAP_ZOOM_OUT: f32 = 0.1; // where it opens: 10 km or so across

/// How far, in points, the pointer can move between the button going down
/// and up for it to be a click, not a drag.
pub const CLICK_SLOP: f32 = 6.0;

/// An 8-bit RGBA colour, as raylib's.
pub type Color = [u8; 4];

/// Something on the map: a footprint, X across and Y for the game's Z, in
/// metres, and its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub r: Rectangle,
    pub c: Color,
}

/// The tiers of a place's name: the lower, the bigger and the sooner
/// drawn, so zoomed out the seats are named, and zoomed in what's in them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// A seat: marked, and named in big letters.
    Seat,
    /// A road or way: named along it, unmarked.
    Route,
    /// A place in a seat: marked, and named small.
    Spot,
}

/// A place's name, at a point in the game's frame (X, Z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub name: &'static str,
    pub at: Vec2,
    pub tier: Tier,
}

const fn label(name: &'static str, x: f32, z: f32, tier: Tier) -> Label {
    Label {
        name,
        at: Vec2::new(x, z),
        tier,
    }
}

/// The map's colours: the Fringe's ground, the ground inside the dome, the
/// floors, and each part of the world.
pub const MAP_FRINGE: Color = [74, 42, 32, 255];
pub const MAP_INSIDE: Color = [104, 62, 46, 255];
pub const MAP_DECK: Color = [64, 64, 70, 255];
pub const MAP_PADS: Color = [92, 92, 96, 255];
pub const MAP_ROAD: Color = [148, 118, 98, 255];
pub const MAP_DEFAULT: Color = [170, 160, 150, 255];
pub const MAP_COLOURS: [(&str, Color); 9] = [
    ("The Hull", [150, 150, 158, 255]),
    ("The Hull market", [222, 142, 60, 255]),
    ("Hull decks", [196, 118, 66, 255]),
    ("The Exchange", [214, 180, 96, 255]),
    ("Charter Row", [236, 236, 240, 255]),
    ("The Pads", [122, 146, 180, 255]),
    ("Domes and gate", [160, 206, 224, 255]),
    ("The Fringe", [178, 124, 92, 255]),
    ("Vehicles", [236, 196, 92, 255]),
];
/// The destination's colour: bright against the map's reds and the
/// accent's orange.
pub const COL_DEST: Color = [120, 220, 255, 255];

/// The dome line, as tools/world/landfall.py has it (DOME_*), in the
/// game's frame.
pub const DOME_AREA: Rectangle = Rectangle::new(-54.0, -56.0, 88.0, 100.0);

/// The maps' names, at landfall.py's districts in the game's frame
/// (Blender's (x, y) is the game's (x, -y); each seat's own layout is
/// moved to its middle).
pub const PLACES: [Label; 29] = [
    label("Landfall", 0.0, 0.0, Tier::Seat),
    label("The Pads", PADS_AT.x, PADS_AT.y, Tier::Seat),
    label("The Fringers' hold", HOLD_AT.x, HOLD_AT.y, Tier::Seat),
    label("The Quiet Book's haven", HAVEN_AT.x, HAVEN_AT.y, Tier::Seat),
    label("Caravan road", 5600.0, -540.0, Tier::Route),
    label("Fringe track", -1260.0, 7600.0, Tier::Route),
    label("The wash", -5900.0, -6100.0, Tier::Route),
    // In Landfall.
    label("The Hull", -16.0, -16.0, Tier::Spot), // just north of it
    label("The Exchange", 2.0, 4.0, Tier::Spot),
    label("Lower decks", -34.0, 4.0, Tier::Spot),
    label("The Stacks", -15.0, -6.0, Tier::Spot),
    label("Hull market", -16.0, 14.0, Tier::Spot),
    label("Charter Row", -4.0, -36.0, Tier::Spot),
    label("South gate", 0.0, 46.0, Tier::Spot),
    label("Caravan stop", -6.0, 54.0, Tier::Spot),
    // At the Pads.
    label("The drifter", 9807.0, -1798.0, Tier::Spot),
    label("Drifter market", 9807.0, -1768.0, Tier::Spot),
    label("Container yard", 9784.0, -1805.0, Tier::Spot),
    label("Arrivals", 9770.0, -1770.0, Tier::Spot),
    label("Fuel depot", 9810.0, -1839.0, Tier::Spot),
    label("Vehicle hire", 9768.0, -1788.0, Tier::Spot),
    // Along the roads.
    label("Fuel stop", 5600.0, -518.0, Tier::Spot),
    label("Broken-down hauler", 7600.0, -1262.0, Tier::Spot),
    label("Scrap cairn", -1291.0, 7600.0, Tier::Spot),
    // In the hold.
    label("Farms", -1625.0, 12023.0, Tier::Spot),
    label("Salvage fields", -1375.0, 12020.0, Tier::Spot),
    label("Fringer camp", -1460.0, 12014.0, Tier::Spot),
    label("Wind farm", -1600.0, 11915.0, Tier::Spot),
    label("Terraformer wreck", -1430.0, 12042.0, Tier::Spot),
];

/// How big each tier's names are on a map, in points: seat, route, spot.
pub type MapLabels = [f32; 3];
pub const MINIMAP_LABELS: MapLabels = [12.0, 10.0, 10.0];
pub const FULL_MAP_LABELS: MapLabels = [20.0, 14.0, 14.0];

/// The map's colour for a piece's category.
pub fn map_colour(category: &str) -> Color {
    MAP_COLOURS
        .iter()
        .find(|(name, _)| *name == category)
        .map(|(_, c)| *c)
        .unwrap_or(MAP_DEFAULT)
}

/// `lighten`: each channel moved `by` of the way to white.
pub fn lighten(c: Color, by: f32) -> Color {
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * by) as u8;
    [mix(c[0]), mix(c[1]), mix(c[2]), c[3]]
}

pub const MAP_CELL_SIZE: f32 = 256.0;

pub fn map_cell(x: f32) -> i32 {
    ((x / MAP_CELL_SIZE) as f64).floor() as i32
}

/// A rectangle on the XZ plane as the Go code lays it out: X across, Y for
/// the game's Z.
#[inline]
fn in_rect(r: &Rectangle, p: Vec2) -> bool {
    r.contains(p)
}

/// `worldMap`, a resource: what the maps draw, and where the full map is
/// looking.
#[derive(Resource, Debug, Clone)]
pub struct WorldMap {
    pub marks: Vec<Mark>,
    pub bounds: Rectangle,
    /// Footprints are static. Their centres are indexed once so the
    /// minimap only visits nearby ones; candidate indices keep the original
    /// paint order.
    pub mark_cells: HashMap<(i32, i32), Vec<usize>>,
    pub mark_radius: f32,
    /// The full map's middle (in metres) and scale (pixels per metre, at
    /// the UI's scale of 1); zero until it's first opened.
    pub at: Vec2,
    pub zoom: f32,
    /// The destination marked on the map (X, Z), if marked.
    pub dest: Vec2,
    pub marked: bool,
}

impl WorldMap {
    /// A map with only its bounds (the tests' empty map).
    pub fn empty(bounds: Rectangle) -> WorldMap {
        WorldMap {
            marks: Vec::new(),
            bounds,
            mark_cells: HashMap::new(),
            mark_radius: 0.0,
            at: Vec2::ZERO,
            zoom: 0.0,
            dest: Vec2::ZERO,
            marked: false,
        }
    }

    /// `newWorldMap`: the map of the pieces in a layout.
    pub fn new(k: &Kit, placed: &[Placement]) -> WorldMap {
        let h = WORLD_SIZE / 2.0;
        let mut m = WorldMap::empty(Rectangle::new(-h, -h, 2.0 * h, 2.0 * h));
        m.marks.push(Mark {
            r: DOME_AREA,
            c: MAP_INSIDE,
        });
        for (f, c) in FLOORED.iter().zip([MAP_DECK, MAP_PADS]) {
            m.marks.push(Mark {
                r: Rectangle::new(f.min.x, f.min.y, f.max.x - f.min.x, f.max.y - f.min.y),
                c,
            });
        }
        let mut pieces: Vec<(Mark, f32)> = Vec::new();
        for p in placed {
            let Some(piece) = k.pieces.get(&p.piece) else {
                continue;
            };
            let c = map_colour(&piece.category);
            let at = p.at();
            let turn = quarter_turns(p.turns);
            for col in &piece.colliders {
                let (r, top) = footprint(col, at, turn);
                // Taller things are lighter, so walls stand out from what's
                // on the floor.
                pieces.push((
                    Mark {
                        r,
                        c: lighten(c, (top / 8.0).min(1.0) * 0.35),
                    },
                    top,
                ));
            }
        }
        // Low things first, so taller ones are drawn over them.
        pieces.sort_by(|a, b| a.1.total_cmp(&b.1));
        m.marks.extend(pieces.into_iter().map(|(mark, _)| mark));
        m.index_marks();
        m
    }

    pub fn index_marks(&mut self) {
        self.mark_cells = HashMap::new();
        self.mark_radius = 0.0;
        for (i, mk) in self.marks.iter().enumerate() {
            let half = Vec2::new(mk.r.width / 2.0, mk.r.height / 2.0);
            let centre = Vec2::new(mk.r.x + half.x, mk.r.y + half.y);
            let cell = (map_cell(centre.x), map_cell(centre.y));
            self.mark_cells.entry(cell).or_default().push(i);
            self.mark_radius = self.mark_radius.max(half.length());
        }
    }

    /// `marksNear`: a conservative subset in the same layering order as
    /// the marks. The exact screen clipping and circle test stay in the
    /// draw paths. Large map views use the linear path, avoiding a scan of
    /// empty grid cells; in dense Landfall, sorting and copying most of
    /// the marks costs more than scanning them.
    pub fn marks_near(&self, at: Vec2, reach: f32) -> Vec<Mark> {
        if self.mark_cells.is_empty() {
            return self.marks.clone();
        }
        let r = reach + self.mark_radius;
        let (x0, x1) = (map_cell(at.x - r), map_cell(at.x + r));
        let (z0, z1) = (map_cell(at.y - r), map_cell(at.y + r));
        if (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 >= self.mark_cells.len() as i64 {
            return self.marks.clone();
        }
        let mut count = 0;
        for z in z0..=z1 {
            for x in x0..=x1 {
                count += self.mark_cells.get(&(x, z)).map_or(0, Vec::len);
            }
        }
        if count >= self.marks.len() / 3 {
            return self.marks.clone();
        }
        let mut candidates = Vec::with_capacity(count);
        for z in z0..=z1 {
            for x in x0..=x1 {
                if let Some(cell) = self.mark_cells.get(&(x, z)) {
                    candidates.extend_from_slice(cell);
                }
            }
        }
        candidates.sort_unstable();
        candidates.into_iter().map(|i| self.marks[i]).collect()
    }
}

/// `north`: the way north is in the world (X, Z): the game's -Z.
pub const NORTH: Vec2 = Vec2::new(0.0, -1.0);

/// `mapFrame`: where the map is drawn: the part of the screen, the point of
/// the world in its middle, how many pixels a metre is, and the way in the
/// world (X, Z) that's up on it: north, unless it turns.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MapFrame {
    pub screen: Rectangle,
    pub at: Vec2,
    pub scale: f32,
    /// A unit vector; zero for north.
    pub up: Vec2,
}

impl MapFrame {
    /// The ways in the world that are right and up on the frame.
    pub fn axes(&self) -> (Vec2, Vec2) {
        let up = if self.up == Vec2::ZERO {
            NORTH
        } else {
            self.up
        };
        (Vec2::new(-up.y, up.x), up)
    }

    /// Whether the frame turns away from north up.
    pub fn turned(&self) -> bool {
        self.up != Vec2::ZERO && self.up != NORTH
    }

    /// The frame's middle on the screen.
    pub fn middle(&self) -> Vec2 {
        Vec2::new(
            self.screen.x + self.screen.width / 2.0,
            self.screen.y + self.screen.height / 2.0,
        )
    }

    /// Where a point of the world (X, Z) is in the frame.
    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        self.middle() + self.screen_dir(p - self.at) * self.scale
    }

    /// The point of the world (X, Z) at a point of the frame.
    pub fn to_world(&self, s: Vec2) -> Vec2 {
        let (right, up) = self.axes();
        let d = (s - self.middle()) * (1.0 / self.scale);
        self.at + right * d.x + up * -d.y
    }

    /// A way in the world (X, Z) as a way on the frame.
    pub fn screen_dir(&self, v: Vec2) -> Vec2 {
        let (right, up) = self.axes();
        Vec2::new(v.dot(right), -v.dot(up))
    }

    /// How far from the frame's middle, in metres, its corners reach.
    pub fn reach(&self) -> f32 {
        hypot(self.screen.width, self.screen.height) / 2.0 / self.scale
    }

    /// The marks `draw` and `drawTurned` would draw, in their order: north
    /// up, those whose clipped rectangle has any size; turned, those that
    /// reach within the circle round the frame. The draw selection the
    /// indexed `marks_near` must preserve.
    pub fn visible_marks(&self, marks: &[Mark]) -> Vec<Mark> {
        let reach = self.reach();
        let mut out = Vec::new();
        for mk in marks {
            if self.turned() {
                let half = Vec2::new(mk.r.width / 2.0, mk.r.height / 2.0);
                let centre = Vec2::new(mk.r.x + half.x, mk.r.y + half.y);
                if centre.distance(self.at) - half.length() > reach {
                    continue;
                }
            } else {
                let a = self.to_screen(Vec2::new(mk.r.x, mk.r.y));
                let r = clip(
                    Rectangle::new(a.x, a.y, mk.r.width * self.scale, mk.r.height * self.scale),
                    self.screen,
                );
                if r.width <= 0.0 || r.height <= 0.0 {
                    continue;
                }
            }
            out.push(*mk);
        }
        out
    }

    /// The marks the frame draws: `marksNear` round its middle, out to
    /// its corners and a pixel over.
    pub fn marks(&self, m: &WorldMap) -> Vec<Mark> {
        m.marks_near(self.at, self.reach() + 1.0 / self.scale)
    }
}

/// `clipSegment`: the part of a to b inside r, if any (Liang-Barsky).
pub fn clip_segment(a: Vec2, b: Vec2, r: Rectangle) -> Option<(Vec2, Vec2)> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    let d = b - a;
    for (p, q) in [
        (-d.x, a.x - r.x),
        (d.x, r.x + r.width - a.x),
        (-d.y, a.y - r.y),
        (d.y, r.y + r.height - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((a + d * t0, a + d * t1))
}

/// `clip`: r cut to within `to`: empty if they don't meet.
pub fn clip(r: Rectangle, to: Rectangle) -> Rectangle {
    let (x0, y0) = (r.x.max(to.x), r.y.max(to.y));
    let (x1, y1) = (
        (r.x + r.width).min(to.x + to.width),
        (r.y + r.height).min(to.y + to.height),
    );
    if x1 <= x0 || y1 <= y0 {
        return Rectangle::default();
    }
    Rectangle::new(x0, y0, x1 - x0, y1 - y0)
}

/// `facingOf`: the way (X, Z) a body turned by `body`, in a root turned by
/// `root`, faces: on foot the body turns; seated, the root turns with what
/// carries it.
pub fn facing_of(root: Quat, body: Quat) -> Vec2 {
    let f = (root * body) * Vec3::Z;
    let d = Vec2::new(f.x, f.z);
    if d.length() > 1e-4 {
        return d.normalize();
    }
    Vec2::new(0.0, 1.0)
}

/// `looking`: the way the camera looks as it's drawn (it eases after where
/// it's steered, and follows a vehicle round), or else where it's steered.
/// `drawn` is the drawn camera's position and target, if there is one.
pub fn looking(drawn: Option<(Vec3, Vec3)>, steered: Vec3) -> Vec3 {
    if let Some((position, target)) = drawn {
        let f = target - position;
        if Vec2::new(f.x, f.z).length() > 1e-4 {
            return f.normalize();
        }
    }
    steered
}

/// `edgePoint`: where a line from the middle of r along `dir` meets r's
/// edge, `pad` in from it.
pub fn edge_point(r: Rectangle, dir: Vec2, pad: f32) -> Vec2 {
    let mid = Vec2::new(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let (hw, hh) = (r.width / 2.0 - pad, r.height / 2.0 - pad);
    let mut t = f32::INFINITY;
    if dir.x != 0.0 {
        t = hw / dir.x.abs();
    }
    if dir.y != 0.0 {
        t = t.min(hh / dir.y.abs());
    }
    if t.is_infinite() {
        return mid;
    }
    mid + dir * t
}

/// `within`: whether p is in r, `pad` in from its edges.
pub fn within(r: Rectangle, p: Vec2, pad: f32) -> bool {
    p.x >= r.x + pad
        && p.x <= r.x + r.width - pad
        && p.y >= r.y + pad
        && p.y <= r.y + r.height - pad
}

/// `distance`: how far a way is, for the destination: to the 10 m under a
/// kilometre, and in tenths of a kilometre over.
pub fn distance(m: f32) -> String {
    if m < 100.0 {
        format!("{:.0} m", m)
    } else if m < 995.0 {
        format!("{:.0} m", ((m as f64) / 10.0).round() * 10.0)
    } else {
        format!("{:.1} km", m / 1000.0)
    }
}

/// `heading`: the way the camera looks, as a compass bearing in degrees: 0
/// north (the game's -Z), 90 east (+X).
pub fn heading(forward: Vec3) -> f32 {
    let mut b = ((forward.x as f64).atan2(-forward.z as f64) * 180.0 / std::f64::consts::PI) as f32;
    if b < 0.0 {
        b += 360.0;
    }
    b
}

/// `click`: marks the destination where the full map, `f`, was clicked at
/// `s`, or takes it off if the click was on its mark (within `reach`
/// pixels).
pub fn click(m: &mut WorldMap, f: MapFrame, s: Vec2, reach: f32) {
    if m.marked && f.to_screen(m.dest).distance(s) <= reach {
        m.marked = false;
        return;
    }
    let d = f.to_world(s);
    let b = m.bounds;
    m.dest = Vec2::new(
        d.x.clamp(b.x, b.x + b.width),
        d.y.clamp(b.y, b.y + b.height),
    );
    m.marked = true;
}

/// The destination's taken off the map when the player gets there.
pub fn arrive(m: &mut WorldMap, at: Vec2) {
    if m.marked && at.distance(m.dest) < ARRIVED {
        m.marked = false;
    }
}

/// Whether a point of the world is on the map.
pub fn on_map(m: &WorldMap, p: Vec2) -> bool {
    in_rect(&m.bounds, p)
}
