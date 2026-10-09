//! The Go tests of `game/terrain_test.go`, `scatter_test.go`,
//! `maps_test.go` and `cull_test.go` that belong to the streaming port,
//! and `TestPiecesStandOnTheGround` and `TestRocksAreOffTheRoads` against
//! the real `world/landfall.json`. Headless: no window, no renderer.

use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    landfall::{
        Budget, LandfallPlugin, StreamCentre,
        cull::{CLIP_FAR, DRAW_DISTANCE, SIGHT_MAX, View, chunk_sphere, sight},
        ground::{Rectangle, level_distance},
        lamps::fixed_lamps,
        maps::{
            MINIMAP_RANGE, MapFrame, WorldMap, click, clip, distance, edge_point, facing_of,
            heading, looking,
        },
        scatter::{SCATTER_CLEAR, SCATTER_KINDS, no_bodies, scatter_cell_of, scatter_in},
        surface::{Footing, Soundscape},
        terrain::{
            TerrainBody, TileResink, detail_window, sink_under, spawn_terrain, stand_on,
            terrain_around, tile_triangle_vertices, write_tile_heights,
        },
        zones::{light_at, places_at},
    },
    physics::{EarthPhysicsPlugin, FIXED_HZ, headless_app, source_assets},
    world::WorldPlugin,
};
use earth_two_world::{
    kit::{Kit, Layout},
    terrain::{
        ARRIVAL, CHUNK_SIZE, GROUND_LEVEL, HAVEN_AT, HOLD_AT, PADS_AT, TILE_SIZE, chunk_centre,
        drawn_height, ground_height, road_distance, sample,
    },
};

fn reference() -> (Kit, Layout) {
    let root = source_assets();
    (
        Kit::load(&root, "world/world.json").unwrap(),
        Layout::load(&root, "world/landfall.json").unwrap(),
    )
}

// TestCachedTileMaskMatchesTerrain
#[test]
fn cached_tile_mask_matches_terrain() {
    const CELLS: usize = 64;
    let (cx, cz) = (9216.0f32, -1024.0f32);
    let mut vertices = tile_triangle_vertices(CELLS);
    let heights: Vec<f32> = sample(cx, cz, TILE_SIZE, CELLS, drawn_height)
        .iter()
        .map(|s| s.height)
        .collect();
    for window in [
        detail_window(35, -4, 3),
        detail_window(39, -8, 3),
        detail_window(0, 0, 3),
    ] {
        let sink = sink_under(window);
        write_tile_heights(&mut vertices, cx, cz, TILE_SIZE, CELLS, &heights, sink);
        for v in vertices.as_chunks::<3>().0 {
            let (x, z) = (cx + v[0], cz + v[2]);
            let want = GROUND_LEVEL + drawn_height(x, z) - sink(x, z);
            assert_eq!(v[1], want, "tile masking changed height at {x},{z}");
        }
    }
}

// TestScatterIsTheSameEachTime
#[test]
fn scatter_is_the_same_each_time() {
    for c in [(0, 0), (75, -12), (-40, 300), (-163, -162)] {
        assert_eq!(scatter_in(c.0, c.1), scatter_in(c.0, c.1), "cell {c:?}");
    }
}

// TestScatterKeepsOffTheLevel
#[test]
fn scatter_keeps_off_the_level() {
    // Along the caravan road and round Landfall, the Pads and the hold:
    // nothing on the roads or the seats' ground.
    let mut ci = -10;
    while ci <= 250 {
        let mut cj = -60;
        while cj <= 310 {
            for s in scatter_in(ci, cj) {
                let d = level_distance(s.at.x, s.at.z);
                assert!(
                    d >= SCATTER_CLEAR,
                    "{} at {}, {d} m from level ground, want at least {SCATTER_CLEAR}",
                    s.piece,
                    s.at
                );
            }
            cj += 7;
        }
        ci += 3;
    }
}

// TestScatterLeavesTheDunesClear
#[test]
fn scatter_leaves_the_dunes_clear() {
    // Round the Pads, where dunes_test.go drives: nothing to run into.
    let dunes = Vec3::new(9600.0, 0.0, -1700.0);
    let (ci, cj) = scatter_cell_of(dunes.x, dunes.z);
    for di in -8..=8 {
        for dj in -8..=8 {
            for s in scatter_in(ci + di, cj + dj) {
                for k in SCATTER_KINDS {
                    assert!(
                        !(k.piece == s.piece && k.min_s == k.max_s && no_bodies(s.at.x, s.at.z)),
                        "{} at {}, in the clear round the Pads",
                        s.piece,
                        s.at
                    );
                }
            }
        }
    }
}

// TestTheFringeIsScattered
#[test]
fn the_fringe_is_scattered() {
    // Out in the open between Landfall and the hold: the bigger part of a
    // square kilometre has something on it.
    let (ci, cj) = scatter_cell_of(-800.0, 5000.0);
    let (mut cells, mut filled) = (0, 0);
    for di in 0..25 {
        for dj in 0..25 {
            cells += 1;
            if !scatter_in(ci + di, cj + dj).is_empty() {
                filled += 1;
            }
        }
    }
    assert!(
        filled >= cells / 2,
        "{filled} of {cells} cells have anything on them, want most"
    );
}

// TestIndexedMapPreservesDrawSelection, with BenchmarkMinimapSelection's
// frames checked the same way.
#[test]
fn indexed_map_preserves_draw_selection() {
    let (kit, layout) = reference();
    let m = WorldMap::new(&kit, &layout.pieces);
    let mut frames = Vec::new();
    for at in [
        Vec2::ZERO,
        PADS_AT,
        HOLD_AT,
        HAVEN_AT,
        Vec2::new(-256.0, 256.0),
        Vec2::new(5600.0, -500.0),
        Vec2::new(16300.0, -16300.0),
    ] {
        for scale in [150.0 / MINIMAP_RANGE, 0.01, 0.5, 8.0] {
            for angle in [
                0.0f64,
                0.3,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            ] {
                frames.push(MapFrame {
                    screen: Rectangle::new(16.0, 16.0, 150.0, 150.0),
                    at,
                    scale,
                    up: Vec2::new(angle.sin() as f32, -(angle.cos() as f32)),
                });
            }
        }
    }
    for at in [PADS_AT, Vec2::ZERO, Vec2::new(4200.0, -300.0)] {
        frames.push(MapFrame {
            screen: Rectangle::new(0.0, 0.0, 150.0, 150.0),
            at,
            scale: 150.0 / MINIMAP_RANGE,
            up: Vec2::new(0.6, -0.8),
        });
    }
    for f in frames {
        let got = f.visible_marks(&f.marks(&m));
        let want = f.visible_marks(&m.marks);
        assert_eq!(
            got,
            want,
            "draw selection changed at {}, scale {}, up {}: {} marks vs {}",
            f.at,
            f.scale,
            f.up,
            got.len(),
            want.len()
        );
    }
}

// TestHeading
#[test]
fn heading_bearings() {
    for (forward, want) in [
        (Vec3::NEG_Z, 0.0),
        (Vec3::X, 90.0),
        (Vec3::Z, 180.0),
        (Vec3::NEG_X, 270.0),
    ] {
        let got = heading(forward);
        assert!(
            (got - want).abs() <= 0.01,
            "heading({forward}) = {got}, want {want}"
        );
    }
}

// TestClip
#[test]
fn clip_to_frame() {
    let frame = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    assert_eq!(
        clip(Rectangle::new(90.0, -10.0, 20.0, 30.0), frame),
        Rectangle::new(90.0, 0.0, 10.0, 20.0)
    );
    assert_eq!(clip(Rectangle::new(200.0, 0.0, 5.0, 5.0), frame).width, 0.0);
}

// TestLandfallMap
#[test]
fn landfall_map() {
    let (kit, layout) = reference();
    let m = WorldMap::new(&kit, &layout.pieces);
    let colliders: usize = layout
        .pieces
        .iter()
        .map(|p| kit.pieces[&p.piece].colliders.len())
        .sum();
    // One for each collider, and the dome's inside and the two floors.
    assert_eq!(m.marks.len(), colliders + 3);
    assert!(m.bounds.contains(Vec2::new(ARRIVAL.x, ARRIVAL.z)));
    // A dome wall on the south side, left of the gate, is on it.
    assert!(
        m.marks
            .iter()
            .any(|mk| mk.r.contains(Vec2::new(-10.0, 44.0))),
        "no mark where the dome's south wall is"
    );
}

fn near(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-3
}

// TestMapFrameNorthUp
#[test]
fn map_frame_north_up() {
    let f = MapFrame {
        screen: Rectangle::new(0.0, 0.0, 100.0, 100.0),
        at: Vec2::new(10.0, 20.0),
        scale: 2.0,
        up: Vec2::ZERO,
    };
    assert!(near(
        f.to_screen(Vec2::new(15.0, 18.0)),
        Vec2::new(60.0, 46.0)
    ));
    assert!(!f.turned());
}

// TestMapFrameTurns
#[test]
fn map_frame_turns() {
    let f = MapFrame {
        screen: Rectangle::new(0.0, 0.0, 100.0, 100.0),
        at: Vec2::new(10.0, 20.0),
        scale: 2.0,
        up: Vec2::X,
    };
    assert!(near(
        f.to_screen(Vec2::new(20.0, 20.0)),
        Vec2::new(50.0, 30.0)
    ));
    assert!(near(
        f.to_screen(Vec2::new(10.0, 10.0)),
        Vec2::new(30.0, 50.0)
    ));
    assert!(near(f.screen_dir(Vec2::Y), Vec2::X));
    let p = Vec2::new(13.0, 27.0);
    assert!(near(f.to_world(f.to_screen(p)), p));
}

// TestEdgePoint
#[test]
fn edge_points() {
    let r = Rectangle::new(0.0, 0.0, 100.0, 50.0);
    for (dir, want) in [
        (Vec2::X, Vec2::new(95.0, 25.0)),
        (Vec2::NEG_Y, Vec2::new(50.0, 5.0)),
        (Vec2::new(1.0, 1.0).normalize(), Vec2::new(70.0, 45.0)),
    ] {
        assert!(near(edge_point(r, dir, 5.0), want), "along {dir}");
    }
}

// TestClickMarksAndUnmarks
#[test]
fn click_marks_and_unmarks() {
    let mut m = WorldMap::empty(Rectangle::new(-1000.0, -1000.0, 2000.0, 2000.0));
    let mut f = MapFrame {
        screen: Rectangle::new(0.0, 0.0, 200.0, 200.0),
        at: Vec2::ZERO,
        scale: 0.5,
        up: Vec2::ZERO,
    };
    click(&mut m, f, Vec2::new(150.0, 60.0), 10.0);
    assert!(
        m.marked && near(m.dest, Vec2::new(100.0, -80.0)),
        "{:?}",
        m.dest
    );
    click(&mut m, f, Vec2::new(30.0, 30.0), 10.0);
    assert!(
        m.marked && near(m.dest, Vec2::new(-140.0, -140.0)),
        "{:?}",
        m.dest
    );
    click(&mut m, f, Vec2::new(34.0, 27.0), 10.0);
    assert!(!m.marked, "a click on the mark left it marked");
    f.at = Vec2::new(990.0, 0.0);
    click(&mut m, f, Vec2::new(190.0, 100.0), 10.0);
    assert_eq!(m.dest.x, 1000.0, "marked off the world");
}

// TestDistance
#[test]
fn distances() {
    for (m, want) in [
        (7.4, "7 m"),
        (99.0, "99 m"),
        (344.0, "340 m"),
        (996.0, "1.0 km"),
        (2460.0, "2.5 km"),
        (10250.0, "10.2 km"),
    ] {
        assert_eq!(distance(m), want);
    }
}

// TestFacingOf
#[test]
fn facing_of_body_and_root() {
    let east = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2);
    assert!(near(facing_of(Quat::IDENTITY, east), Vec2::X));
    assert!(near(facing_of(east, Quat::IDENTITY), Vec2::X));
    // A vehicle nosing down a slope still faces the way it's going.
    let down = east * Quat::from_axis_angle(Vec3::X, 0.3);
    assert!(near(facing_of(down, Quat::IDENTITY), Vec2::X));
}

// TestLookingFollowsTheDrawnCamera
#[test]
fn looking_follows_the_drawn_camera() {
    let drawn = Some((Vec3::new(1.0, 5.0, 1.0), Vec3::new(4.0, 1.0, 1.0)));
    let got = looking(drawn, Vec3::NEG_Z);
    assert!(got.x > 0.0 && got.z.abs() <= 1e-5, "{got}");
    assert_eq!(looking(None, Vec3::NEG_Z), Vec3::NEG_Z);
}

fn test_view() -> View {
    let mut v = View {
        ahead: Vec3::NEG_Z,
        up: Vec3::Y,
        right: Vec3::X,
        tan_v: 0.4142,
        aspect: 2.0,
        ..Default::default()
    };
    v.prepare();
    v
}

// TestViewSees
#[test]
fn view_sees() {
    let v = test_view();
    for (name, center, radius, want) in [
        ("straight ahead", Vec3::new(0.0, 0.0, -10.0), 1.0, true),
        ("behind", Vec3::new(0.0, 0.0, 10.0), 1.0, false),
        (
            "behind, but big enough to reach round",
            Vec3::new(0.0, 0.0, 3.0),
            5.0,
            true,
        ),
        ("off to the side", Vec3::new(30.0, 0.0, -10.0), 1.0, false),
        (
            "just in, at the side",
            Vec3::new(8.0, 0.0, -10.0),
            1.0,
            true,
        ),
        ("overhead", Vec3::new(0.0, 30.0, -10.0), 1.0, false),
        (
            "too far",
            Vec3::new(0.0, 0.0, -DRAW_DISTANCE - 5.0),
            1.0,
            false,
        ),
        (
            "far, but big enough to reach in",
            Vec3::new(0.0, 0.0, -DRAW_DISTANCE - 5.0),
            10.0,
            true,
        ),
    ] {
        assert_eq!(v.sees(center, radius, DRAW_DISTANCE), want, "{name}");
    }
}

// TestBigPiecesAreSeenFurther
#[test]
fn big_pieces_are_seen_further() {
    assert!(sight(0.8) <= DRAW_DISTANCE + 20.0);
    assert!(sight(24.0) >= 9000.0);
    assert_eq!(sight(1000.0), SIGHT_MAX);
}

// TestDetailChunkBounds
#[test]
fn detail_chunk_bounds() {
    let at = Vec3::new(9800.0, 0.0, -1800.0);
    let b = chunk_sphere(at);
    for x in [-CHUNK_SIZE / 2.0, CHUNK_SIZE / 2.0] {
        for z in [-CHUNK_SIZE / 2.0, CHUNK_SIZE / 2.0] {
            for height in [-66.0, 200.0] {
                let corner = at + Vec3::new(x, height, z);
                assert!(corner.distance(b.center) <= b.radius, "clips {corner}");
            }
        }
    }
    let v = test_view();
    for (z, visible) in [(-CHUNK_SIZE, true), (CHUNK_SIZE, false)] {
        let b = chunk_sphere(Vec3::new(0.0, 0.0, z));
        assert_eq!(
            v.sees(b.center, b.radius, CLIP_FAR),
            visible,
            "chunk at Z={z}"
        );
    }
}

// TestRocksAreOffTheRoads
#[test]
fn rocks_are_off_the_roads() {
    let (_, layout) = reference();
    for p in &layout.pieces {
        if matches!(
            p.piece.as_str(),
            "rock_small"
                | "rock_large"
                | "rock_spire"
                | "dust_mound"
                | "dry_brush"
                | "dead_tree"
                | "quiver_tree"
        ) {
            let (d, _) = road_distance(p.at[0], p.at[2]);
            assert!(d >= 9.0, "{} at {:?} is {d:.1} m off a road", p.piece, p.at);
        }
    }
}

// TestPiecesStandOnTheGround
#[test]
fn pieces_stand_on_the_ground() {
    let (kit, layout) = reference();
    let mut lifted = 0;
    for p in &layout.pieces {
        let h = stand_on(&kit, p);
        let under = ground_height(p.at[0], p.at[2]);
        assert!(
            h <= under + 1e-4,
            "{} at {:?} stands at {h}, above the ground under its middle ({under})",
            p.piece,
            p.at
        );
        if h != 0.0 {
            lifted += 1;
        }
    }
    assert!(lifted > 0, "nothing out in the Fringe stands off the level");
}

/// The lamps, the soundscape and the zones read off the real layout.
#[test]
fn landfall_data_reads() {
    let lamps = fixed_lamps(&source_assets()).unwrap();
    assert!(!lamps.is_empty());
    let (kit, layout) = reference();
    let s = Soundscape::new(&kit, &layout.pieces);
    assert!(s.bell.is_some(), "no bell");
    assert!(s.nearest("generator", Vec3::ZERO).is_some());
    assert_eq!(
        s.surface_at(Vec3::new(-16.0, 0.0, 4.0), false),
        Footing::Paving
    );
    assert_eq!(
        s.surface_at(Vec3::new(3900.0, 0.0, -120.0), false),
        Footing::Soil
    );
    assert_eq!(
        s.surface_at(Vec3::new(3900.0, 0.0, 300.0), false),
        Footing::Sand
    );
    assert_eq!(
        s.surface_at(Vec3::new(3900.0, 0.0, 300.0), true),
        Footing::Grating
    );
    let hull = places_at(Vec3::new(-16.0, 1.0, 4.0));
    assert!(hull.hull > 0.99 && hull.outside < 0.01, "{hull:?}");
    let out = places_at(Vec3::new(3000.0, 1.0, 0.0));
    assert!(out.outside > 0.99, "{out:?}");
    assert_eq!(light_at(Vec3::new(3000.0, 1.0, 0.0)), [255, 182, 128, 255]);
}

/// TestWalkingOffTheRoadOntoTheDunes needs the character controller; until
/// the character port, a dynamic capsule dropped onto `terrain_around`'s
/// colliders stands in: on the caravan road east of Landfall, and north of
/// it on the dunes, it settles on the ground as the Go test expected the
/// character's feet to.
#[test]
fn capsule_settles_on_the_terrain_colliders() {
    let mut app = headless_app(AssetPlugin::default());
    let mut world = app.world_mut();
    {
        let mut commands = world.commands();
        terrain_around(&mut commands, 1000.0, 167.0, 1);
    }
    world.flush();
    let mut probes = Vec::new();
    for (x, z) in [(1000.0f32, 167.0f32), (1000.0, 140.0)] {
        let y = ground_height(x, z) + 3.0;
        probes.push(
            world
                .spawn((
                    RigidBody::Dynamic,
                    Collider::capsule(0.3, 1.2),
                    Transform::from_xyz(x, y, z),
                    LockedAxes::ROTATION_LOCKED,
                    Friction::new(1.0),
                ))
                .id(),
        );
    }
    for _ in 0..300 {
        app.update();
    }
    world = app.world_mut();
    for e in probes {
        let at = world.get::<Position>(e).unwrap().0;
        let feet = at - Vec3::new(0.0, 0.9, 0.0);
        let want = ground_height(feet.x, feet.z) + GROUND_LEVEL;
        assert!(
            feet.is_finite() && (feet.y - want).abs() <= 0.3,
            "capsule at {feet}, want on the ground at {want}"
        );
    }
}

/// `streamTerrain`: crossing into another chunk moves the colliders at
/// once and rebuilds the chunks a few a frame, nearest first; the tiles
/// under the old and new windows are told to resink.
#[test]
fn terrain_streams_round_the_player() {
    // As `headless_app`, with the subsystem in before the app is finished.
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
        EarthPhysicsPlugin,
        WorldPlugin,
        LandfallPlugin {
            budget: Budget::BROWSER,
        },
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(1.0 / FIXED_HZ),
    ));
    app.finish();
    app.cleanup();
    let world = app.world_mut();
    let terrain = {
        let mut commands = world.commands();
        spawn_terrain(&mut commands, &Budget::BROWSER, ARRIVAL)
    };
    world.insert_resource(terrain);
    world.flush();
    let player = world
        .spawn((StreamCentre, Transform::from_translation(ARRIVAL)))
        .id();
    app.update();
    let (ci, cj) = {
        let t = app
            .world()
            .resource::<earth_two_client::landfall::terrain::Terrain>();
        assert!(t.pending.is_empty());
        (t.ci, t.cj)
    };
    // Two chunks east.
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .x += 2.0 * CHUNK_SIZE;
    app.update();
    {
        let t = app
            .world()
            .resource::<earth_two_client::landfall::terrain::Terrain>();
        assert_eq!((t.ci, t.cj), (ci + 2, cj));
        // The colliders moved at once, to the chunks round the player.
        for (s, b) in t.bodies.iter().enumerate() {
            assert_eq!(*b, t.body_at(s));
            let c = chunk_centre(b.ci, b.cj);
            let tr = app.world().get::<Transform>(t.body_entities[s]).unwrap();
            assert_eq!(tr.translation, Vec3::new(c.x, 0.0, c.y));
        }
        // Every slot shows a chunk in the window, or is queued.
        let r = t.detail_radius;
        for c in &t.chunks {
            assert!((c.ci - t.ci).abs() <= r && (c.cj - t.cj).abs() <= r);
        }
        // Ten new columns of chunks for a radius of 2, less one frame's rebuild.
        assert_eq!(
            t.pending.len(),
            2 * (2 * r as usize + 1) - Budget::BROWSER.chunks_per_frame
        );
    }
    let bodies = app
        .world_mut()
        .query::<&TerrainBody>()
        .iter(app.world())
        .count();
    assert_eq!(bodies, 9);
    let resinking = app
        .world_mut()
        .query_filtered::<Entity, With<TileResink>>()
        .iter(app.world())
        .count();
    assert!(resinking > 0, "no tiles told to resink");
    for _ in 0..20 {
        app.update();
    }
    let t = app
        .world()
        .resource::<earth_two_client::landfall::terrain::Terrain>();
    assert!(t.pending.is_empty());
    for c in &t.chunks {
        assert_eq!((c.bi, c.bj), (c.ci, c.cj), "slot shows its chunk");
    }
}
