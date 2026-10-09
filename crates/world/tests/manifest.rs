//! The real manifests and models, checked as `world/world_test.go` and
//! `game/terrain_test.go` check them: the same specifications, ported.

use std::{fs, path::PathBuf};

use bevy_math::Vec3;
use earth_two_world::{
    kit::{Kit, Layout, Piece},
    terrain,
};
use serde::Deserialize;

fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
}

fn kit() -> Kit {
    Kit::load(assets(), "world/world.json").unwrap()
}

#[derive(Deserialize)]
struct Gltf {
    meshes: Vec<Mesh>,
    accessors: Vec<Accessor>,
}

#[derive(Deserialize)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    attributes: std::collections::HashMap<String, usize>,
    indices: Option<usize>,
}

#[derive(Deserialize)]
struct Accessor {
    count: usize,
    min: Option<Vec<f32>>,
    max: Option<Vec<f32>>,
}

/// The JSON chunk of a .glb.
fn gltf(model: &str) -> Gltf {
    let bytes = fs::read(assets().join(model)).unwrap();
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[20..20 + n]).unwrap()
}

/// The box round every POSITION accessor: the pieces are one mesh at the
/// origin, so that's the model's bounds.
fn mesh_bounds(model: &str) -> (Vec3, Vec3) {
    let doc = gltf(model);
    let mut lo = Vec3::INFINITY;
    let mut hi = Vec3::NEG_INFINITY;
    for mesh in &doc.meshes {
        for primitive in &mesh.primitives {
            let accessor = &doc.accessors[primitive.attributes["POSITION"]];
            let (min, max) = (
                accessor.min.as_ref().unwrap(),
                accessor.max.as_ref().unwrap(),
            );
            lo = lo.min(Vec3::new(min[0], min[1], min[2]));
            hi = hi.max(Vec3::new(max[0], max[1], max[2]));
        }
    }
    (lo, hi)
}

fn triangles(model: &str) -> usize {
    let doc = gltf(model);
    doc.meshes
        .iter()
        .flat_map(|m| &m.primitives)
        .map(|p| doc.accessors[p.indices.unwrap()].count / 3)
        .sum()
}

fn inside(point: Vec3, lo: Vec3, hi: Vec3, slack: f32) -> bool {
    point.cmpge(lo - slack).all() && point.cmple(hi + slack).all()
}

#[test]
fn triangle_budgets() {
    for (name, p) in &kit().pieces {
        assert!(
            p.budget > 0 && p.budget <= 150_000,
            "{name}: a budget of {} triangles",
            p.budget
        );
        let n = triangles(&p.model);
        assert!(
            n <= p.budget as usize,
            "{name}: {n} triangles, over its budget of {}",
            p.budget
        );
    }
}

#[test]
fn meshes_fit_sixteen_bit_indices() {
    for (name, p) in &kit().pieces {
        let doc = gltf(&p.model);
        for mesh in &doc.meshes {
            for primitive in &mesh.primitives {
                let count = doc.accessors[primitive.attributes["POSITION"]].count;
                assert!(count <= 65535, "{name}: a mesh of {count} vertices");
            }
        }
    }
}

#[test]
fn colliders_inside_models() {
    const SLACK: f32 = 0.25;
    for (name, p) in &kit().pieces {
        let (lo, hi) = mesh_bounds(&p.model);
        assert!(hi.y - lo.y > 0.0, "{name}: an empty model");
        for (i, c) in p.colliders.iter().enumerate() {
            let (clo, chi) = c.bounds();
            assert!(
                inside(clo, lo, hi, SLACK) && inside(chi, lo, hi, SLACK),
                "{name}: collider {i} ({clo} to {chi}) outside the model ({lo} to {hi})"
            );
        }
    }
}

#[test]
fn traversal_sizes() {
    let kit = kit();
    let top = |name: &str| kit.pieces[name].colliders[0].bounds().1.y;
    for name in ["crate", "drum", "railing"] {
        let h = top(name);
        assert!(
            (0.35..=1.0).contains(&h),
            "{name} is {h} m high; vaults are 0.35 to 1 m"
        );
    }
    let h = top("crate_tall");
    assert!(
        h > 1.0 && h <= 1.8,
        "crate_tall is {h} m high; mantles are 1 to 1.8 m"
    );
    let (lo, _) = kit.pieces["low_duct"].colliders[0].bounds();
    assert!(
        lo.y > 0.9 && lo.y < 1.8,
        "low_duct's underside is {} m up",
        lo.y
    );
    let ladders = &kit.pieces["ladder"].ladders;
    assert!(
        ladders.len() == 1 && ladders[0].width == 0.64,
        "ladder's ladders: {ladders:?}"
    );
    let (_, hi) = kit.pieces["catwalk"].colliders[0].bounds();
    assert!(
        (hi.y - ladders[0].top[1] + 0.04).abs() <= 0.01,
        "the ladder's top isn't 4 cm over the catwalk"
    );
}

#[test]
fn kinds_and_categories() {
    for (name, p) in &kit().pieces {
        assert!(
            Piece::KINDS.contains(&p.kind.as_str()),
            "{name}: kind {:?}",
            p.kind
        );
        assert!(!p.category.is_empty(), "{name}: no category");
        assert!(
            !(p.kind == "item" && !p.colliders.is_empty()),
            "{name}: an item with colliders"
        );
        assert!(
            !(p.kind == "vehicle" && p.colliders.is_empty()),
            "{name}: a vehicle with no colliders"
        );
        for (i, c) in p.colliders.iter().enumerate() {
            assert!(
                c.size.iter().all(|s| *s > 0.0),
                "{name}: collider {i} is {:?}",
                c.size
            );
        }
    }
}

#[test]
fn layouts_name_pieces() {
    let kit = kit();
    for layout in ["world/hull_block.json", "world/landfall.json"] {
        let layout = Layout::load(assets(), layout).unwrap();
        assert!(!layout.pieces.is_empty(), "an empty layout");
        for p in &layout.pieces {
            assert!(
                kit.pieces.contains_key(&p.piece),
                "the layout places {:?}, which isn't a piece",
                p.piece
            );
        }
    }
}

#[test]
fn lamps_give_light() {
    let kit = kit();
    for name in [
        "pendant_lamp",
        "work_lamp",
        "street_lamp",
        "floodlight_tower",
        "pad_beacon",
        "status_light",
    ] {
        assert!(!kit.pieces[name].lights.is_empty(), "{name} gives no light");
    }
    const SLACK: f32 = 0.5;
    for (name, p) in &kit.pieces {
        if p.lights.is_empty() {
            continue;
        }
        let (lo, hi) = mesh_bounds(&p.model);
        for (i, l) in p.lights.iter().enumerate() {
            assert!(
                l.range > 0.0 && l.intensity > 0.0,
                "{name}: light {i} reaches {} m at {}",
                l.range,
                l.intensity
            );
            assert!(
                inside(Vec3::from(l.at), lo, hi, SLACK),
                "{name}: light {i} at {:?}, outside the model",
                l.at
            );
        }
    }
}

#[test]
fn drivable_vehicles() {
    let kit = kit();
    let mut drivable = 0;
    for (name, p) in &kit.pieces {
        let Some(v) = &p.vehicle else { continue };
        drivable += 1;
        assert!(
            !v.handling.is_empty()
                && !v.wheels.is_empty()
                && !v.seats.is_empty()
                && !v.chassis.is_empty(),
            "{name}: drives with handling {:?}, {} wheels, {} seats, {} chassis boxes",
            v.handling,
            v.wheels.len(),
            v.seats.len(),
            v.chassis.len()
        );
        for (i, w) in v.wheels.iter().enumerate() {
            assert!(
                kit.pieces
                    .get(&w.piece)
                    .is_some_and(|wp| wp.kind == "wheel"),
                "{name}: wheel {i} is drawn with {:?}, which isn't a wheel",
                w.piece
            );
            assert!(
                w.radius > 0.0 && (w.at[1] - w.radius).abs() <= 0.08,
                "{name}: wheel {i} of radius {} has its centre {} m up",
                w.radius,
                w.at[1]
            );
        }
        for (i, s) in v.seats.iter().enumerate() {
            assert!(
                !s.exits.is_empty(),
                "{name}: seat {i} has nowhere to get out"
            );
        }
    }
    assert!(drivable >= 5, "{drivable} drivable vehicles");
}

/// The ground at fixed points as the Go client computes it
/// (`game/terrain_fixture_test.go`).
#[derive(Deserialize)]
struct TerrainPoint {
    at: [f32; 2],
    ground: f32,
    drawn: f32,
    walk: f32,
    colour: [u8; 3],
    noise: f32,
}

#[test]
fn terrain_matches_the_go_fixture() {
    let data = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/terrain.json"
    ))
    .unwrap();
    let points: Vec<TerrainPoint> = serde_json::from_slice(&data).unwrap();
    assert!(points.len() > 100);
    // Go fuses multiply-adds on arm64 and Rust doesn't, so the two can
    // differ in the last bits of each octave; a millimetre covers that over
    // the mountains, and a channel step covers it in the paint.
    const HEIGHT: f32 = 1e-3;
    let mut worst = 0.0f32;
    for p in &points {
        let [x, z] = p.at;
        let noise = terrain::fbm(x / 45.0, z / 45.0);
        assert!(
            (noise - p.noise).abs() <= 1e-6,
            "noise at {x},{z}: {noise}, Go has {}",
            p.noise
        );
        for (what, got, want) in [
            ("ground", terrain::ground_height(x, z), p.ground),
            ("drawn", terrain::drawn_height(x, z), p.drawn),
            ("walk", terrain::walk_height(x, z), p.walk),
        ] {
            worst = worst.max((got - want).abs());
            assert!(
                (got - want).abs() <= HEIGHT,
                "{what} at {x},{z}: {got}, Go has {want}"
            );
        }
        let colour = terrain::ground_colour(x, z);
        for c in 0..3 {
            assert!(
                colour[c].abs_diff(p.colour[c]) <= 1,
                "colour at {x},{z}: {colour:?}, Go has {:?}",
                p.colour
            );
        }
    }
    eprintln!("worst height difference from Go: {worst} m");
}
