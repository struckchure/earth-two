//! The folded parchment pointer from game/paper_cursor.go.
use super::{
    Screen,
    paper::{node, scale},
};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
#[derive(Component)]
pub struct PaperCursor;
fn pixels() -> Vec<u8> {
    let shape = [
        Vec2::new(3., 3.),
        Vec2::new(3., 29.),
        Vec2::new(10., 23.),
        Vec2::new(15., 35.),
        Vec2::new(20., 33.),
        Vec2::new(15., 22.),
        Vec2::new(28., 21.),
    ];
    let inside = |p: Vec2| {
        let mut inside = false;
        for i in 0..shape.len() {
            let a = shape[i];
            let b = shape[(i + 1) % shape.len()];
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    };
    let mut pixels = Vec::with_capacity(108 * 126 * 4);
    for y in 0..126 {
        for x in 0..108 {
            let p = Vec2::new((x as f32 + 0.5) / 3., (y as f32 + 0.5) / 3.);
            let mut color = if inside(p - Vec2::splat(1.8)) {
                [0, 0, 0, 70]
            } else {
                [0, 0, 0, 0]
            };
            if inside(p) {
                let mut edge = f32::INFINITY;
                for i in 0..shape.len() {
                    let a = shape[i];
                    let d = shape[(i + 1) % shape.len()] - a;
                    edge = edge.min(
                        p.distance(a + d * ((p - a).dot(d) / d.length_squared()).clamp(0., 1.)),
                    );
                }
                color = if edge < 0.8 {
                    [46, 48, 43, 255]
                } else if p.x > 16. && p.y > 13. && p.y < 22. {
                    [211, 195, 160, 255]
                } else if (x / 3 * 13 + y / 3 * 7) % 23 == 0 {
                    [237, 226, 195, 255]
                } else {
                    [246, 237, 211, 255]
                };
            }
            pixels.extend_from_slice(&color);
        }
    }
    pixels
}
pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new(
        Extent3d {
            width: 108,
            height: 126,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands.spawn((
        PaperCursor,
        ImageNode::new(image),
        node(0., 0., 36., 42., 1.),
        GlobalZIndex(1000),
        Pickable::IGNORE,
        Visibility::Hidden,
    ));
}
pub fn draw(
    window: Query<&Window, With<PrimaryWindow>>,
    screen: Res<State<Screen>>,
    mut cursors: Query<(&mut Node, &mut Visibility), With<PaperCursor>>,
) {
    let Ok(w) = window.single() else { return };
    let pos = w
        .cursor_position()
        .filter(|p| p.x >= 0. && p.y >= 0. && p.x < w.width() && p.y < w.height());
    for (mut n, mut visible) in &mut cursors {
        if let Some(p) = pos
            && (*screen.get() != Screen::Playing || cfg!(target_arch = "wasm32"))
        {
            let sc = scale(w.width(), w.height()) * 0.75;
            *n = node(p.x - 3. * sc, p.y - 3. * sc, 36. * sc, 42. * sc, 1.);
            *visible = Visibility::Visible;
        } else {
            *visible = Visibility::Hidden;
        }
    }
}
