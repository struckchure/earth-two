//! Retained Bevy UI drawing of game/maps.go. Footprints reuse nodes between frames;
//! labels use measured Bevy font widths, and the map viewport clips rotated shapes.
use super::{
    Screen, Session,
    camera::{GameCamera, Orbit},
    navigation,
    paper::Fonts,
};
use crate::{
    character::Body,
    landfall::{
        ground::Rectangle,
        maps::{self, MapFrame, Tier, WorldMap},
    },
};
use bevy::{prelude::*, text::TextLayoutInfo, window::PrimaryWindow};
use earth_two_world::terrain::{ROADS, ground_height};
use std::collections::HashMap;
const TEXT: Color = Color::srgb_u8(245, 240, 232);
const MUTED: Color = Color::srgb_u8(178, 168, 156);
const ACCENT: Color = Color::srgb_u8(255, 184, 82);
const PANEL: Color = Color::srgba_u8(16, 13, 11, 214);
const DEST: Color = Color::srgb_u8(120, 220, 255);
fn rgba(c: [u8; 4]) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], c[3])
}
fn rect(r: Rectangle) -> Rect {
    Rect::new(r.x, r.y, r.x + r.width, r.y + r.height)
}
#[derive(Component)]
pub(super) struct MapShape;
#[derive(Component)]
pub(super) struct MapLabel;
#[derive(Default)]
struct Pool {
    root: Option<Entity>,
    shapes: Vec<Entity>,
    labels: Vec<Entity>,
}
#[derive(Default)]
pub struct Drawing {
    map: Pool,
    overlay: Pool,
    backdrop: Pool,
    widths: HashMap<String, Vec2>,
}
struct Shape {
    r: Rect,
    color: Color,
    angle: f32,
    round: bool,
}
struct Label {
    at: Vec2,
    value: String,
    size: f32,
    color: Color,
    heading: bool,
}
#[derive(Default)]
struct Paint {
    shapes: Vec<Shape>,
    labels: Vec<Label>,
}
impl Paint {
    fn rect(&mut self, r: Rect, color: Color) {
        self.shapes.push(Shape {
            r,
            color,
            angle: 0.,
            round: false,
        });
    }
    fn square(&mut self, at: Vec2, size: f32, color: Color, angle: f32) {
        self.shapes.push(Shape {
            r: Rect::from_center_size(at, Vec2::splat(size)),
            color,
            angle,
            round: false,
        });
    }
    fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) {
        let d = b - a;
        self.shapes.push(Shape {
            r: Rect::from_center_size((a + b) / 2., Vec2::new(d.length(), width)),
            color,
            angle: d.y.atan2(d.x),
            round: false,
        });
    }
    fn text(&mut self, at: Vec2, value: impl Into<String>, size: f32, color: Color) {
        self.labels.push(Label {
            at,
            value: value.into(),
            size,
            color,
            heading: false,
        });
    }
    fn shadow(&mut self, at: Vec2, value: &str, size: f32, color: Color, sc: f32) {
        self.text(
            at + Vec2::splat(sc.max(1.)),
            value,
            size,
            Color::srgba_u8(0, 0, 0, 200),
        );
        self.text(at, value, size, color);
    }
    fn diamond(&mut self, at: Vec2, size: f32) {
        for (s, c) in [
            (1.35, Color::srgba_u8(0, 0, 0, 220)),
            (1., DEST),
            (0.3, Color::srgba_u8(0, 0, 0, 220)),
        ] {
            self.square(at, size * s, c, std::f32::consts::FRAC_PI_4);
        }
    }
    fn you(&mut self, at: Vec2, dir: Vec2, size: f32) {
        let dir = dir.normalize_or_zero();
        let side = Vec2::new(-dir.y, dir.x);
        let tip = at + dir * size;
        let back = at - dir * size * 0.6;
        let l = back + side * size * 0.7;
        let r = back - side * size * 0.7;
        self.shapes.push(Shape {
            r: Rect::from_center_size(at, Vec2::splat(size * 1.9)),
            color: Color::srgba_u8(0, 0, 0, 120),
            angle: 0.,
            round: true,
        });
        for (a, b) in [(l, tip), (r, tip), (l, at), (r, at)] {
            self.line(a, b, (size * 0.32).max(2.), ACCENT);
        }
    }
}
/// Cached logical text size at a one-point font size. The first frame uses a conservative estimate.
fn measure(widths: &HashMap<String, Vec2>, value: &str, size: f32) -> Vec2 {
    widths
        .get(value)
        .copied()
        .unwrap_or(Vec2::new(value.chars().count() as f32 * 0.56, 1.2))
        * size
}
fn map(
    p: &mut Paint,
    m: &WorldMap,
    f: MapFrame,
    sc: f32,
    full: bool,
    widths: &HashMap<String, Vec2>,
) {
    p.rect(rect(f.screen), rgba(maps::MAP_FRINGE));
    let dir = f.screen_dir(Vec2::X);
    let angle = dir.y.atan2(dir.x);
    for mk in f.visible_marks(&f.marks(m)) {
        let size = Vec2::new(mk.r.width, mk.r.height) * f.scale;
        let at = f.to_screen(Vec2::new(
            mk.r.x + mk.r.width / 2.,
            mk.r.y + mk.r.height / 2.,
        ));
        p.shapes.push(Shape {
            r: Rect::from_center_size(at, size.max(Vec2::ONE)),
            color: rgba(mk.c),
            angle,
            round: false,
        });
    }
    for road in &ROADS {
        if !road.paint {
            continue;
        }
        for points in road.points.windows(2) {
            if let Some((a, b)) =
                maps::clip_segment(f.to_screen(points[0]), f.to_screen(points[1]), f.screen)
            {
                p.line(
                    a,
                    b,
                    (2. * road.half * f.scale).max((2. * sc).max(2.)),
                    rgba(maps::MAP_ROAD),
                );
            }
        }
    }
    let sizes = if full {
        maps::FULL_MAP_LABELS
    } else {
        maps::MINIMAP_LABELS
    };
    let mark = sizes[2] * 0.4 * sc;
    let mut taken: Vec<Rect> = vec![];
    for tier in [Tier::Seat, Tier::Route, Tier::Spot] {
        for l in maps::PLACES.iter().filter(|l| l.tier == tier) {
            let pos = f.to_screen(l.at);
            if !f.screen.contains(pos) {
                continue;
            }
            let size = sizes[tier as usize] * sc;
            let measured = measure(widths, l.name, size);
            let mut at = pos - measured / 2.;
            if tier != Tier::Route {
                at.y = pos.y - mark - measured.y;
            }
            let bounds = rect(f.screen);
            at = at
                .max(bounds.min)
                .min((bounds.max - measured).max(bounds.min));
            let r = Rect::from_corners(at, at + measured);
            if taken.iter().any(|other| {
                r.min.x < other.max.x
                    && r.max.x > other.min.x
                    && r.min.y < other.max.y
                    && r.max.y > other.min.y
            }) {
                continue;
            }
            taken.push(r);
            if tier != Tier::Route {
                let s = mark * if tier == Tier::Seat { 1.4 } else { 1. };
                p.square(pos, s + 2., Color::srgba_u8(0, 0, 0, 200), 0.);
                p.square(pos, s, if tier == Tier::Seat { ACCENT } else { TEXT }, 0.);
            }
            p.shadow(
                at,
                l.name,
                size,
                if tier == Tier::Route { MUTED } else { TEXT },
                sc,
            );
        }
    }
}
fn route(p: &mut Paint, m: &WorldMap, f: MapFrame, at: Vec2, sc: f32, full: bool) {
    if !m.marked {
        return;
    }
    let mut dest = f.to_screen(m.dest);
    if let Some((a, b)) = maps::clip_segment(f.to_screen(at), dest, f.screen) {
        p.line(
            a,
            b,
            if full {
                (3. * sc).max(2.)
            } else {
                (2. * sc).max(1.5)
            },
            Color::srgba_u8(120, 220, 255, 150),
        );
    }
    let size = if full { 16. * sc } else { 10. * sc };
    if !maps::within(f.screen, dest, size) {
        dest = maps::edge_point(f.screen, (dest - f.middle()).normalize_or_zero(), size);
    }
    p.diamond(dest, size);
}
fn compass(
    p: &mut Paint,
    bearing: f32,
    size: Vec2,
    sc: f32,
    dest: Option<Vec2>,
    widths: &HashMap<String, Vec2>,
) {
    let w = maps::COMPASS_WIDTH * sc;
    let h = 34. * sc;
    let at = Vec2::new((size.x - w) / 2., 14. * sc);
    let mid = at.x + w / 2.;
    p.rect(Rect::from_corners(at, at + Vec2::new(w, h)), PANEL);
    let offset = |b: f32| (b - bearing + 180.).rem_euclid(360.) - 180.;
    for d in (0..360).step_by(15) {
        let off = offset(d as f32);
        if off.abs() > 86. {
            continue;
        }
        let x = mid + off * w / maps::COMPASS_SPAN;
        if d % 45 == 0 {
            let name = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][d / 45];
            let font = if d % 90 == 0 { 16. } else { 13. } * sc;
            let m = measure(widths, name, font);
            p.text(
                Vec2::new(x - m.x / 2., at.y + (h - m.y) / 2.),
                name,
                font,
                if d == 0 {
                    ACCENT
                } else if d % 90 == 0 {
                    TEXT
                } else {
                    MUTED
                },
            );
        } else {
            p.rect(
                Rect::from_center_size(
                    Vec2::new(x, at.y + h * 0.5),
                    Vec2::new((1.5 * sc).max(1.), h * 0.3),
                ),
                MUTED,
            );
        }
    }
    if let Some(d) = dest {
        let off = offset(maps::heading(Vec3::new(d.x, 0., d.y))).clamp(-84., 84.);
        p.diamond(
            Vec2::new(mid + off * w / maps::COMPASS_SPAN, at.y + h * 0.72),
            9. * sc,
        );
    }
    p.rect(
        Rect::new(
            mid - sc.max(1.),
            at.y + h - 5. * sc,
            mid + sc.max(1.),
            at.y + h + 4. * sc,
        ),
        ACCENT,
    );
    let value = format!("{bearing:03.0}");
    let m = measure(widths, &value, 13. * sc);
    p.text(
        Vec2::new(mid - m.x / 2., at.y + h + 6. * sc),
        value,
        13. * sc,
        MUTED,
    );
}
fn destination(
    p: &mut Paint,
    m: &WorldMap,
    player: Vec3,
    cam: &Transform,
    size: Vec2,
    sc: f32,
    widths: &HashMap<String, Vec2>,
) {
    let point = Vec3::new(
        m.dest.x,
        ground_height(m.dest.x, m.dest.y).max(player.y - 1.) + 2.,
        m.dest.y,
    );
    let d = point - cam.translation;
    let forward = d.dot(*cam.forward());
    let right = d.dot(*cam.right());
    let up = d.dot(*cam.up());
    let screen = Rectangle::new(0., 0., size.x, size.y);
    let factor = size.y / (2. * 22.5f32.to_radians().tan());
    let mut pos = size / 2. + Vec2::new(right, -up) * factor / forward.max(0.001);
    if forward <= 0.5 || !maps::within(screen, pos, 40. * sc) {
        let mut dir = Vec2::new(right, -up);
        if forward <= 0.5 {
            dir.y = dir.y.max(dir.x.abs() * 0.5 + 1.);
        }
        if dir.length() < 1e-4 {
            dir = Vec2::Y;
        }
        pos = maps::edge_point(screen, dir.normalize(), 40. * sc);
    }
    p.diamond(pos, 14. * sc);
    let value = maps::distance(player.xz().distance(m.dest));
    let m = measure(widths, &value, 14. * sc);
    p.shadow(
        pos + Vec2::new(-m.x / 2., 14. * sc),
        &value,
        14. * sc,
        TEXT,
        sc,
    );
}
// Pools avoid recreating thousands of footprint entities on every orbit or pan.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn apply(
    commands: &mut Commands,
    pool: &mut Pool,
    paint: Paint,
    viewport: Rect,
    z: i32,
    fonts: &Fonts,
    shapes: &mut Query<(&mut Node, &mut UiTransform, &mut BackgroundColor), With<MapShape>>,
    labels: &mut Query<
        (
            &mut Node,
            &mut Text,
            &mut TextFont,
            &mut TextColor,
            Option<&TextLayoutInfo>,
        ),
        (With<MapLabel>, Without<MapShape>),
    >,
) {
    let root = *pool.root.get_or_insert_with(|| {
        commands
            .spawn((
                super::paper::node(
                    viewport.min.x,
                    viewport.min.y,
                    viewport.width(),
                    viewport.height(),
                    1.,
                ),
                GlobalZIndex(z),
                Pickable::IGNORE,
            ))
            .id()
    });
    let mut node = super::paper::node(
        viewport.min.x,
        viewport.min.y,
        viewport.width(),
        viewport.height(),
        1.,
    );
    node.overflow = Overflow::clip();
    commands.entity(root).insert(node);
    for (i, s) in paint.shapes.iter().enumerate() {
        let mut node = super::paper::node(
            s.r.min.x - viewport.min.x,
            s.r.min.y - viewport.min.y,
            s.r.width(),
            s.r.height(),
            1.,
        );
        if s.round {
            node.border_radius = BorderRadius::MAX;
        }
        let tr = UiTransform::from_rotation(Rot2::radians(s.angle));
        let bg = BackgroundColor(s.color);
        if let Some(&e) = pool.shapes.get(i) {
            if let Ok((mut n, mut t, mut c)) = shapes.get_mut(e) {
                n.set_if_neq(node);
                t.set_if_neq(tr);
                c.set_if_neq(bg);
            }
        } else {
            let e = commands
                .spawn((MapShape, node, tr, bg, Pickable::IGNORE, ChildOf(root)))
                .id();
            pool.shapes.push(e);
        }
    }
    for &e in &pool.shapes[paint.shapes.len()..] {
        if let Ok((mut n, _, _)) = shapes.get_mut(e) {
            n.display = Display::None;
        }
    }
    for (i, l) in paint.labels.iter().enumerate() {
        let node = Node {
            position_type: PositionType::Absolute,
            left: px(l.at.x - viewport.min.x),
            top: px(l.at.y - viewport.min.y),
            ..default()
        };
        let font = TextFont {
            font: if l.heading {
                fonts.heading.clone()
            } else {
                fonts.label.clone()
            }
            .into(),
            font_size: px(l.size).into(),
            ..default()
        };
        if let Some(&e) = pool.labels.get(i) {
            if let Ok((mut n, mut t, mut f, mut c, _)) = labels.get_mut(e) {
                n.set_if_neq(node);
                t.set_if_neq(Text::new(&l.value));
                f.set_if_neq(font);
                c.set_if_neq(TextColor(l.color));
            }
        } else {
            let e = commands
                .spawn((
                    node,
                    MapLabel,
                    Text::new(&l.value),
                    font,
                    TextColor(l.color),
                    TextLayout::no_wrap(),
                    ZIndex(1),
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .id();
            pool.labels.push(e);
        }
    }
    for &e in &pool.labels[paint.labels.len()..] {
        if let Ok((mut n, _, _, _, _)) = labels.get_mut(e) {
            n.display = Display::None;
        }
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    map_data: Option<Res<WorldMap>>,
    session: Res<Session>,
    orbit: Res<Orbit>,
    windows: Query<&Window, With<PrimaryWindow>>,
    transforms: Query<(&Transform, Option<&Children>)>,
    bodies: Query<&Transform, With<Body>>,
    cameras: Query<&Transform, With<GameCamera>>,
    fonts: Res<Fonts>,
    mut drawing: Local<Drawing>,
    mut shapes: Query<(&mut Node, &mut UiTransform, &mut BackgroundColor), With<MapShape>>,
    mut labels: Query<
        (
            &mut Node,
            &mut Text,
            &mut TextFont,
            &mut TextColor,
            Option<&TextLayoutInfo>,
        ),
        (With<MapLabel>, Without<MapShape>),
    >,
) {
    let full = *screen.get() == Screen::Mapping;
    if !full && *screen.get() != Screen::Playing {
        for e in [
            drawing.map.root,
            drawing.overlay.root,
            drawing.backdrop.root,
        ]
        .into_iter()
        .flatten()
        {
            commands.entity(e).insert(Node {
                display: Display::None,
                ..default()
            });
        }
        return;
    }
    let Some(m) = map_data else { return };
    let Ok(w) = windows.single() else { return };
    let Some((player, children)) = session.player.and_then(|e| transforms.get(e).ok()) else {
        return;
    };
    drawing.widths.retain(|name, _| {
        maps::PLACES.iter().any(|l| l.name == name)
            || ["N", "NE", "E", "SE", "S", "SW", "W", "NW"].contains(&name.as_str())
    });
    for (_, t, f, _, layout) in &labels {
        if let (bevy::text::FontSize::Px(size), Some(l)) = (f.font_size, layout)
            && size > 0.
            && l.size.x > 0.
        {
            drawing
                .widths
                .insert(t.0.clone(), l.size / (l.scale_factor * size));
        }
    }
    let body = children
        .and_then(|c| c.iter().find_map(|e| bodies.get(e).ok()))
        .map_or(Quat::IDENTITY, |b| b.rotation);
    let facing = maps::facing_of(player.rotation, body);
    let at = player.translation.xz();
    let size = Vec2::new(w.width(), w.height());
    let sc = navigation::scale(size);
    let viewport = Rect::from_corners(Vec2::ZERO, size);
    let cam = cameras.single().ok();
    let forward = cam.map_or(orbit.forward(), |c| *c.forward());
    let frame = if full {
        navigation::full_frame(&m, size)
    } else {
        MapFrame {
            screen: Rectangle::new(
                16. * sc,
                16. * sc,
                maps::MINIMAP_SIZE * sc,
                maps::MINIMAP_SIZE * sc,
            ),
            at,
            scale: maps::MINIMAP_SIZE * sc / maps::MINIMAP_RANGE,
            up: forward.xz().normalize_or_zero(),
        }
    };
    let mut background = Paint::default();
    let mut drawing_map = Paint::default();
    let mut overlay = Paint::default();
    if full {
        background.rect(viewport, Color::srgba_u8(0, 0, 0, 160));
    }
    background.rect(
        rect(frame.screen).inflate(if full { 8. * sc } else { (3. * sc).max(1.) }),
        PANEL,
    );
    map(&mut drawing_map, &m, frame, sc, full, &drawing.widths);
    route(&mut overlay, &m, frame, at, sc, full);
    let you = frame.to_screen(at);
    if frame.screen.contains(you) {
        overlay.you(
            you,
            frame.screen_dir(facing),
            if full { 7. * sc } else { 5. * sc },
        );
    }
    if full {
        overlay.labels.push(Label {
            at: Vec2::new(frame.screen.x + 16. * sc, frame.screen.y + 12. * sc),
            value: "The Fringe".into(),
            size: 30. * sc,
            color: TEXT,
            heading: true,
        });
        if m.marked {
            overlay.text(
                Vec2::new(frame.screen.x + 16. * sc, frame.screen.y + 52. * sc),
                format!("Destination: {}", maps::distance(at.distance(m.dest))),
                16. * sc,
                DEST,
            );
        }
    } else {
        let north = maps::edge_point(
            frame.screen,
            frame.screen_dir(maps::NORTH).normalize_or_zero(),
            9. * sc,
        );
        overlay.square(north, 16. * sc, Color::srgba_u8(0, 0, 0, 170), 0.);
        let msize = measure(&drawing.widths, "N", 12. * sc);
        overlay.text(north - msize / 2., "N", 12. * sc, ACCENT);
        compass(
            &mut overlay,
            maps::heading(forward),
            size,
            sc,
            m.marked.then_some(m.dest - at),
            &drawing.widths,
        );
        if m.marked
            && let Some(cam) = cam
        {
            destination(
                &mut overlay,
                &m,
                player.translation,
                cam,
                size,
                sc,
                &drawing.widths,
            );
        }
    }
    apply(
        &mut commands,
        &mut drawing.backdrop,
        background,
        viewport,
        2,
        &fonts,
        &mut shapes,
        &mut labels,
    );
    apply(
        &mut commands,
        &mut drawing.map,
        drawing_map,
        rect(frame.screen),
        3,
        &fonts,
        &mut shapes,
        &mut labels,
    );
    apply(
        &mut commands,
        &mut drawing.overlay,
        overlay,
        viewport,
        4,
        &fonts,
        &mut shapes,
        &mut labels,
    );
}
