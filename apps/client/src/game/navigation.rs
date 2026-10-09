//! Map input and development teleporting from game/maps.go and teleport.go.
use super::{
    GameSet, MenuAction, Screen, Session,
    camera::{GameCamera, Orbit},
    menu::Menu,
};
use crate::{
    character::{CharacterController, CharacterPhysics, ControllerState, Intent, Traversal},
    landfall::{ground::Rectangle, maps::*},
    presentation::MotionSamples,
    vehicle::{Driving, VehicleSystems},
};
use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use earth_two_world::terrain::{GROUND_LEVEL, ground_height, walk_height};

pub struct NavigationPlugin;
impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapPointer>()
            .init_resource::<MapGesture>()
            .init_resource::<PendingTeleport>()
            .add_systems(Update, input.after(GameSet::Menu).before(GameSet::Input))
            .add_systems(
                Update,
                teleport.after(VehicleSystems::Drive).before(GameSet::Seats),
            );
        #[cfg(feature = "viewer")]
        app.add_systems(Update, pointer.before(GameSet::Menu));
    }
}
/// Logical window coordinates; independent of a window so the real input flow is testable headlessly.
#[derive(Resource, Debug)]
pub struct MapPointer {
    pub size: Vec2,
    pub at: Option<Vec2>,
    pub delta: Vec2,
    pub wheel: f32,
    pub left: bool,
    pub right: bool,
    pub pressed: bool,
    pub released: bool,
}
impl Default for MapPointer {
    fn default() -> Self {
        Self {
            size: Vec2::new(1280., 720.),
            at: None,
            delta: Vec2::ZERO,
            wheel: 0.,
            left: false,
            right: false,
            pressed: false,
            released: false,
        }
    }
}
#[derive(Resource, Default)]
pub struct MapGesture {
    opened: bool,
    pressing: bool,
    moved: f32,
}
#[derive(Resource, Default)]
struct PendingTeleport(Option<Vec2>);
pub fn scale(size: Vec2) -> f32 {
    (size.y / 760.).min(size.x / 1150.).clamp(0.7, 1.6)
}
pub fn full_frame(map: &WorldMap, size: Vec2) -> MapFrame {
    let sc = scale(size);
    let pad = 48. * sc;
    MapFrame {
        screen: Rectangle::new(
            pad,
            pad,
            (size.x - 2. * pad).max(1.),
            (size.y - 2. * pad).max(1.),
        ),
        at: map.at,
        scale: map.zoom * sc,
        up: Vec2::ZERO,
    }
}
fn clamp(map: &WorldMap, at: Vec2) -> Vec2 {
    if map.bounds.width <= 0. {
        return at;
    }
    at.clamp(
        Vec2::new(map.bounds.x, map.bounds.y),
        Vec2::new(
            map.bounds.x + map.bounds.width,
            map.bounds.y + map.bounds.height,
        ),
    )
}
/// One frame of Go's pointer gesture: left click marks; either button pans; wheel anchors the point under the cursor.
pub fn gesture(map: &mut WorldMap, gesture: &mut MapGesture, pointer: &MapPointer) {
    let Some(at) = pointer.at else {
        if pointer.released {
            gesture.pressing = false;
        }
        return;
    };
    let sc = scale(pointer.size);
    if pointer.pressed {
        gesture.pressing = true;
        gesture.moved = 0.;
    } else if gesture.pressing {
        gesture.moved += pointer.delta.length();
    }
    if pointer.left || pointer.right {
        map.at -= pointer.delta / (map.zoom * sc);
    }
    if pointer.released && gesture.pressing {
        gesture.pressing = false;
        let f = full_frame(map, pointer.size);
        if gesture.moved <= CLICK_SLOP * sc && f.screen.contains(at) {
            click(map, f, at, MARK_REACH * sc);
        }
    }
    if pointer.wheel != 0. {
        let f = full_frame(map, pointer.size);
        let under = f.to_world(at);
        map.zoom = (map.zoom * 1.15f32.powf(pointer.wheel)).clamp(MAP_ZOOM_MIN, MAP_ZOOM_MAX);
        map.at = under - (at - f.middle()) / (map.zoom * sc);
    }
    map.at = clamp(map, map.at);
}
#[allow(clippy::too_many_arguments)]
fn input(
    mut map: Option<ResMut<WorldMap>>,
    mut g: ResMut<MapGesture>,
    pointer: Res<MapPointer>,
    menu: Res<Menu>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    players: Query<&Transform>,
    mut pending: ResMut<PendingTeleport>,
    mut actions: MessageWriter<MenuAction>,
) {
    let Some(map) = map.as_mut() else { return };
    let Some(player) = session.player.and_then(|e| players.get(e).ok()) else {
        return;
    };
    let at = player.translation.xz();
    arrive(map, at);
    if menu.screen() == Screen::Mapping {
        if !g.opened {
            map.at = at;
            map.zoom = MAP_ZOOM_OUT;
            g.opened = true;
        }
        gesture(map, &mut g, &pointer);
    } else {
        g.opened = false;
        g.pressing = false;
    }
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    let to = match menu.screen() {
        Screen::Mapping => {
            let Some(at) = pointer.at else { return };
            let f = full_frame(map, pointer.size);
            if !f.screen.contains(at) {
                return;
            }
            actions.write(MenuAction::Back);
            f.to_world(at)
        }
        Screen::Playing if map.marked => map.dest,
        _ => return,
    };
    pending.0 = Some(clamp(map, to));
}
#[cfg(feature = "viewer")]
fn pointer(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    mut previous: Local<Option<Vec2>>,
    mut pointer: ResMut<MapPointer>,
) {
    let Ok(w) = windows.single() else { return };
    let at = w.cursor_position();
    // Go pans by window-coordinate motion, not raw device motion (notably on Retina).
    let delta = at.zip(*previous).map_or(Vec2::ZERO, |(at, old)| at - old);
    *previous = at;
    // raylib/GLFW keeps the most recent wheel event in a frame.
    let wheel = scroll.read().last().map_or(0., |event| {
        let pixels = event.unit == bevy::input::mouse::MouseScrollUnit::Pixel;
        if cfg!(target_arch = "wasm32") {
            // Emscripten GLFW: 100 CSS pixels or 3 lines per notch, at least one.
            let d = event.y / if pixels { 100. * w.scale_factor() } else { 3. };
            if d == 0. {
                0.
            } else {
                d.signum() * d.abs().max(1.)
            }
        } else if pixels {
            // GLFW Cocoa scales precise logical scroll deltas by 0.1.
            event.y / w.scale_factor() / if cfg!(target_os = "macos") { 10. } else { 40. }
        } else {
            event.y
        }
    });
    *pointer = MapPointer {
        size: Vec2::new(w.width(), w.height()),
        at,
        delta,
        wheel,
        left: buttons.pressed(MouseButton::Left),
        right: buttons.pressed(MouseButton::Right),
        pressed: buttons.just_pressed(MouseButton::Left),
        released: buttons.just_released(MouseButton::Left),
    };
}
/// The ground is analytic beyond the streamed collider ring. A roof wins only when there isn't standing room underneath it.
pub fn landing(physics: &CharacterPhysics, at: Vec2, exclude: Entity) -> Vec3 {
    let ground = Vec3::new(at.x, walk_height(at.x, at.y), at.y);
    let Some(hit) =
        physics.cast_ray_excluding(Vec3::new(at.x, 3000., at.y), Vec3::NEG_Y, 6000., exclude)
    else {
        return ground;
    };
    if hit.point.y <= ground.y + 0.2 {
        return ground;
    }
    if !physics.overlap_capsule_excluding(ground + Vec3::Y * 0.95, 0.3, 1.8, exclude)
        && physics
            .cast_ray_excluding(ground + Vec3::Y * 0.1, Vec3::Y, 1.9, exclude)
            .is_none_or(|h| h.distance >= 1.9)
    {
        return ground;
    }
    hit.point
}
fn teleport(world: &mut World) {
    let Some(to) = world.resource_mut::<PendingTeleport>().0.take() else {
        return;
    };
    let car = world.resource::<Driving>().vehicle;
    let Some(entity) = car.or(world.resource::<Session>().player) else {
        return;
    };
    let Some(mut tr) = world.get::<Transform>(entity).copied() else {
        return;
    };
    let height = world.get::<CharacterController>(entity).map(|cc| cc.height);
    if car.is_none() && height.is_none() {
        return;
    }
    let feet = {
        let mut state = SystemState::<CharacterPhysics>::new(world);
        landing(&state.get(world).expect("physics initialized"), to, entity)
    };
    let from = tr.translation;
    tr.translation = feet + Vec3::Y * height.map_or(0.4, |h| h / 2. + 0.05);
    if car.is_some() {
        let f = tr.rotation * Vec3::Z;
        tr.rotation = Quat::from_rotation_y(f.x.atan2(f.z));
    }
    world.entity_mut(entity).insert(tr);
    if let Some(mut p) = world.get_mut::<Position>(entity) {
        p.0 = tr.translation;
    }
    if let Some(mut r) = world.get_mut::<Rotation>(entity) {
        r.0 = tr.rotation;
    }
    if let Some(mut v) = world.get_mut::<LinearVelocity>(entity) {
        v.0 = Vec3::ZERO;
    }
    if let Some(mut v) = world.get_mut::<AngularVelocity>(entity) {
        v.0 = Vec3::ZERO;
    }
    if let Some(mut cc) = world.get_mut::<CharacterController>(entity) {
        cc.velocity = Vec3::ZERO;
        cc.walk = Vec3::ZERO;
        cc.grounded = false;
        cc.controlled = false;
    }
    if let Some(mut s) = world.get_mut::<ControllerState>(entity) {
        *s = default();
    }
    if let Some(mut s) = world.get_mut::<Traversal>(entity) {
        *s = default();
    }
    if let Some(mut i) = world.get_mut::<Intent>(entity) {
        *i = default();
    }
    if let Some(mut s) = world.get_mut::<MotionSamples>(entity) {
        *s = MotionSamples::at(tr.translation, height.unwrap_or(1.8));
    }
    if car.is_some() {
        let mut d = world.resource_mut::<Driving>();
        d.pose = (tr.translation, tr.rotation);
        d.speed = 0.;
    }
    let delta = Vec3::new(to.x - from.x, 0., to.y - from.z);
    for mut cam in world
        .query_filtered::<&mut Transform, With<GameCamera>>()
        .iter_mut(world)
    {
        cam.translation += delta;
        cam.translation.y = cam
            .translation
            .y
            .max(ground_height(cam.translation.x, cam.translation.z) + GROUND_LEVEL + 1.2);
    }
    let mut orbit = world.resource_mut::<Orbit>();
    orbit.still = 0.;
    if let Some(target) = orbit.target.as_mut() {
        *target += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn map() -> WorldMap {
        let mut m = WorldMap::empty(Rectangle::new(-16000., -16000., 32000., 32000.));
        m.zoom = 0.1;
        m
    }
    #[test]
    fn click_toggle_drag_and_zoom_limits() {
        let mut m = map();
        let mut g = MapGesture::default();
        let mut p = MapPointer {
            at: Some(Vec2::new(740., 400.)),
            pressed: true,
            left: true,
            ..default()
        };
        gesture(&mut m, &mut g, &p);
        p.pressed = false;
        p.left = false;
        p.released = true;
        gesture(&mut m, &mut g, &p);
        assert!(m.marked);
        p.pressed = true;
        p.left = true;
        p.released = false;
        gesture(&mut m, &mut g, &p);
        p.pressed = false;
        p.left = false;
        p.released = true;
        gesture(&mut m, &mut g, &p);
        assert!(!m.marked);
        p.released = false;
        p.pressed = true;
        p.left = true;
        gesture(&mut m, &mut g, &p);
        p.pressed = false;
        p.delta = Vec2::new(30., 0.);
        gesture(&mut m, &mut g, &p);
        p.delta = Vec2::ZERO;
        p.left = false;
        p.released = true;
        gesture(&mut m, &mut g, &p);
        assert!(!m.marked, "drag must not place a mark");
        p.released = false;
        p.wheel = 1000.;
        gesture(&mut m, &mut g, &p);
        assert_eq!(m.zoom, MAP_ZOOM_MAX);
        p.wheel = -1000.;
        gesture(&mut m, &mut g, &p);
        assert_eq!(m.zoom, MAP_ZOOM_MIN);
        p.wheel = 0.;
        p.right = true;
        p.delta = Vec2::splat(1e6);
        gesture(&mut m, &mut g, &p);
        assert_eq!(m.at, Vec2::splat(-16000.));
    }
    #[test]
    fn outside_click_and_lost_pointer_do_not_mark() {
        let mut m = map();
        let mut g = MapGesture::default();
        let mut p = MapPointer {
            at: Some(Vec2::ZERO),
            pressed: true,
            left: true,
            ..default()
        };
        gesture(&mut m, &mut g, &p);
        p.left = false;
        p.pressed = false;
        p.released = true;
        gesture(&mut m, &mut g, &p);
        assert!(!m.marked);
        p.at = Some(Vec2::new(500., 300.));
        p.pressed = true;
        p.left = true;
        p.released = false;
        gesture(&mut m, &mut g, &p);
        p.at = None;
        p.released = true;
        gesture(&mut m, &mut g, &p);
        assert!(!g.pressing);
        assert!(!m.marked);
    }
    #[test]
    fn landing_selects_ground_under_high_roofs_and_top_of_blocked_ground() {
        use crate::physics::EarthPhysicsPlugin;
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, EarthPhysicsPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                1. / 60.,
            )));
        app.finish();
        app.cleanup();
        let at = Vec2::new(100., 100.);
        let y = walk_height(at.x, at.y);
        let roof = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(10., 0.5, 10.),
                Transform::from_xyz(at.x, y + 5., at.y),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let sample = |world: &mut World, exclude: Entity| {
            let mut state = SystemState::<CharacterPhysics>::new(world);
            landing(&state.get(world).unwrap(), at, exclude)
        };
        assert!((sample(app.world_mut(), Entity::PLACEHOLDER).y - y).abs() < 0.01);
        app.world_mut().entity_mut(roof).insert((
            Transform::from_xyz(at.x, y + 1., at.y),
            Position(Vec3::new(at.x, y + 1., at.y)),
        ));
        for _ in 0..3 {
            app.update();
        }
        assert!(
            (sample(app.world_mut(), Entity::PLACEHOLDER).y - y - 1.25).abs() < 0.01,
            "low roof leaves no room"
        );
        app.world_mut().despawn(roof);
        let body = app
            .world_mut()
            .spawn((RigidBody::Static, Transform::from_xyz(at.x, y, at.y)))
            .id();
        app.world_mut().spawn((
            Collider::cuboid(3., 3., 3.),
            Transform::from_xyz(0., 1.5, 0.),
            ChildOf(body),
        ));
        for _ in 0..3 {
            app.update();
        }
        {
            let mut state = SystemState::<CharacterPhysics>::new(app.world_mut());
            let physics = state.get(app.world()).unwrap();
            let centre = Vec3::new(at.x, y + 0.95, at.y);
            assert!(!physics.overlap_capsule_excluding(centre, 0.3, 1.8, body));
            assert!(physics.overlap_capsule_excluding(centre, 0.3, 1.8, Entity::PLACEHOLDER));
        }
        assert!(
            (sample(app.world_mut(), body).y - y).abs() < 0.01,
            "exclude compound child colliders too"
        );
        assert!(
            (sample(app.world_mut(), Entity::PLACEHOLDER).y - y - 3.).abs() < 0.01,
            "solid obstruction lands on top"
        );
    }
}
