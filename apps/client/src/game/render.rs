//! Platform shell, asynchronous content, and the first playable menu/HUD.
use super::{
    GameSet, MenuAction, Screen, Session, StartAt,
    camera::{GameCamera, Orbit},
};
use crate::{
    character::Body,
    landfall::{LandfallSet, render::SHADOW_LAYER},
    presentation::{
        content,
        mesh::ModelStore,
        viewer::{Content, Loading, MeshEntities},
    },
    vehicle::{Driving, Prompt},
    world::{Lamp, LayoutRoot, PieceModel, Placed, Wheel},
};
use bevy::{
    asset::{AssetLoader, AssetPlugin, LoadContext, io::Reader},
    camera::visibility::RenderLayers,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowResolution},
};

pub fn run() {
    let mut app = App::new();
    let plugins = DefaultPlugins.build();
    #[cfg(target_arch = "wasm32")]
    let plugins = plugins.disable::<bevy::audio::AudioPlugin>();
    app.add_plugins(
        plugins
            .set(AssetPlugin {
                file_path: asset_root(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Earth Two".into(),
                    resolution: if cfg!(target_arch = "wasm32") {
                        WindowResolution::new(1280, 720).with_scale_factor_override(1.)
                    } else {
                        WindowResolution::new(1280, 720)
                    },
                    canvas: Some("#earth-two".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
    );
    app.insert_resource(start_at())
        .add_plugins(super::GamePlugin)
        .run();
}
fn asset_root() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "assets".into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("EARTH_TWO_ASSET_ROOT").unwrap_or_else(|_| crate::physics::source_assets())
    }
}
fn start_at() -> StartAt {
    #[cfg(not(target_arch = "wasm32"))]
    let value = std::env::args()
        .find_map(|a| a.strip_prefix("--at=").map(str::to_owned))
        .unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    let value = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
        .and_then(|p| p.get("at"))
        .unwrap_or_default();
    match value.as_str() {
        "hull" => StartAt::Hull,
        "buggy" => StartAt::Buggy,
        "bike" => StartAt::Bike,
        "crowd" => StartAt::Crowd,
        "injury" => StartAt::Injury,
        "fatal" => StartAt::Fatal,
        "traversal" => StartAt::Traversal,
        _ => StartAt::Arrival,
    }
}
#[derive(Asset, TypePath)]
struct WardrobeText(String);
#[derive(Default, TypePath)]
struct WardrobeLoader;
impl AssetLoader for WardrobeLoader {
    type Asset = WardrobeText;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        String::from_utf8(bytes)
            .map(WardrobeText)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
#[derive(Resource)]
struct WardrobeRequest(Handle<WardrobeText>);
#[derive(Component)]
pub(super) struct MenuRoot;
#[derive(Component)]
struct Hud;
#[derive(Component, Clone, Copy)]
pub(super) struct Choice {
    pub action: MenuAction,
    pub focus: usize,
}
use super::menu::{Menu, choices};

pub struct GameRenderPlugin;
impl Plugin for GameRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<WardrobeText>()
            .init_asset_loader::<WardrobeLoader>()
            .add_systems(Startup, (setup, super::wardrobe_render::fonts))
            .add_systems(Update, (load_wardrobe, attach_models, readiness).chain())
            .add_observer(buttons)
            .add_observer(hover)
            .add_systems(Update, pointer.before(GameSet::Input))
            .add_systems(
                Update,
                (
                    menus,
                    super::wardrobe_render::draw,
                    hud,
                    injury_panel,
                    crowd_panel,
                )
                    .after(GameSet::Camera),
            )
            .add_systems(
                Update,
                sync_lighting
                    .after(crate::sky::SkySystems)
                    .before(LandfallSet::Cull),
            );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, capture);
    }
}
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(WardrobeRequest(assets.load(content::WARDROBE)));
    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 45f32.to_radians(),
            near: 0.2,
            far: 20000.,
            ..default()
        }),
        if cfg!(target_arch = "wasm32") {
            Msaa::Off
        } else {
            Msaa::Sample4
        },
        Transform::from_xyz(9807., 3., -1765.9),
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font_size: px(18).into(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            bottom: px(20),
            ..default()
        },
    ));
}
fn load_wardrobe(
    mut commands: Commands,
    request: Option<Res<WardrobeRequest>>,
    texts: Res<Assets<WardrobeText>>,
    assets: Res<AssetServer>,
    mut session: ResMut<Session>,
) {
    let Some(request) = request else { return };
    if let bevy::asset::LoadState::Failed(error) = assets.load_state(request.0.id()) {
        session.error = Some(format!("Wardrobe: {error}"));
        return;
    }
    if let Some(text) = texts.get(&request.0) {
        if let Err(error) =
            crate::presentation::outfit::load_wardrobe(&text.0, &content::people(), |_| None)
        {
            session.error = Some(format!("Wardrobe: {error}"));
            return;
        }
        commands.insert_resource(Content {
            models: content::people(),
            wardrobe: Some(text.0.clone()),
        });
        commands.remove_resource::<WardrobeRequest>();
    }
}
#[allow(clippy::type_complexity)]
fn attach_models(
    mut commands: Commands,
    assets: Res<AssetServer>,
    models: Query<(Entity, &PieceModel), Added<PieceModel>>,
    visible: Query<Entity, Or<(Added<LayoutRoot>, Added<Placed>, Added<Wheel>, Added<Lamp>)>>,
) {
    for e in &visible {
        commands.entity(e).insert(Visibility::Inherited);
    }
    for (e, model) in &models {
        if !model.0.is_empty() {
            let scene: Handle<WorldAsset> = assets.load(format!("{}#Scene0", model.0));
            commands
                .entity(e)
                .insert(Visibility::Inherited)
                .with_child(WorldAssetRoot(scene));
        }
    }
}
fn readiness(
    mut session: ResMut<Session>,
    loading: Res<Loading>,
    store: Res<ModelStore>,
    assets: Res<AssetServer>,
    roots: Query<&WorldAssetRoot>,
    bodies: Query<&MeshEntities, With<Body>>,
) {
    if session.ready || session.player.is_none() || loading.gltfs.is_empty() {
        return;
    }
    for (path, handle) in &loading.gltfs {
        if let Some(error) = asset_error(&assets, handle.id().untyped()) {
            session.error = Some(format!("{path}: {error}"));
            return;
        }
    }
    for root in &roots {
        if let Some(error) = asset_error(&assets, root.0.id().untyped()) {
            session.error = Some(error);
            return;
        }
    }
    let ready =
        loading.gltfs.iter().all(|(path, h)| {
            assets.is_loaded_with_dependencies(h.id()) && store.get(path).is_some()
        }) && !roots.is_empty()
            && roots
                .iter()
                .all(|r| assets.is_loaded_with_dependencies(r.0.id()))
            && !bodies.is_empty();
    if ready {
        session.ready = true;
        info!(
            "Playable ready: {} model scenes; player and Landfall loaded",
            roots.iter().count()
        );
    }
}
fn asset_error(assets: &AssetServer, id: bevy::asset::UntypedAssetId) -> Option<String> {
    if let bevy::asset::LoadState::Failed(error) = assets.load_state(id) {
        return Some(error.to_string());
    }
    if let bevy::asset::RecursiveDependencyLoadState::Failed(error) =
        assets.recursive_dependency_load_state(id)
    {
        return Some(error.to_string());
    }
    None
}
fn pointer(
    mut cursors: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    motion: Res<AccumulatedMouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    screen: Res<State<Screen>>,
    mut orbit: ResMut<Orbit>,
    mut actions: MessageWriter<MenuAction>,
) {
    let Ok((window, mut cursor)) = cursors.single_mut() else {
        return;
    };
    orbit.aspect = window.width() / window.height().max(1.0);
    orbit.menu_fraction = (if *screen.get() == Screen::Dressing {
        448. * super::wardrobe_render::scale(window.width(), window.height())
    } else {
        430.
    } / window.width().max(1.0))
    .min(0.6);
    let playing = *screen.get() == Screen::Playing;
    if playing && !window.focused {
        actions.write(MenuAction::Pause);
    }
    let held = playing && window.focused;
    cursor.grab_mode = if held && !cfg!(target_arch = "wasm32") {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !held || cfg!(target_arch = "wasm32");
    if held
        && (!cfg!(target_arch = "wasm32")
            || buttons.any_pressed([MouseButton::Left, MouseButton::Right]))
    {
        orbit.turn(motion.delta);
    }
}
fn buttons(
    event: On<bevy::ui_widgets::Activate>,
    choices: Query<&Choice>,
    mut actions: MessageWriter<MenuAction>,
) {
    if let Ok(choice) = choices.get(event.entity) {
        actions.write(MenuAction::Focus(choice.focus));
        actions.write(choice.action);
    }
}

fn hover(
    event: On<bevy::picking::events::PointerMove>,
    choices: Query<&Choice>,
    mut menu: ResMut<Menu>,
) {
    if let Ok(choice) = choices.get(event.entity) {
        menu.set_focus(choice.focus);
    }
}

fn menus(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    menu: Res<Menu>,
    roots: Query<
        Entity,
        (
            With<MenuRoot>,
            Without<super::wardrobe_render::WardrobeRoot>,
        ),
    >,
    mut previous: Local<Option<(Screen, usize, Option<String>)>>,
) {
    let key = (*screen.get(), menu.focus(), session.error.clone());
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    if matches!(*screen.get(), Screen::Playing | Screen::Dressing) {
        return;
    }
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(40),
                top: percent(18),
                width: px(390),
                padding: UiRect::all(px(28)),
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.075, 0.10, 0.94)),
        ))
        .with_children(|c| {
            c.spawn((
                Text::new(match screen.get() {
                    Screen::Loading => "Loading Earth Two",
                    Screen::Paused => "Paused",
                    _ => "EARTH TWO",
                }),
                TextFont {
                    font_size: px(36).into(),
                    ..default()
                },
                TextColor(Color::srgb(0.89, 0.81, 0.64)),
            ));
            if let Some(error) = &session.error {
                c.spawn((
                    Text::new(format!("Unable to load the world\n{error}")),
                    TextFont {
                        font_size: px(16).into(),
                        ..default()
                    },
                ));
            }
            for (i, (label, action)) in choices(*screen.get()).into_iter().enumerate() {
                c.spawn((
                    bevy::ui_widgets::Button,
                    Choice { action, focus: i },
                    bevy::ui_widgets::ActivateOnPress,
                    Node {
                        padding: UiRect::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(if i == menu.focus() {
                        Color::srgb(0.36, 0.23, 0.18)
                    } else {
                        Color::srgb(0.18, 0.12, 0.13)
                    }),
                ))
                .with_child((
                    Text::new(label),
                    TextFont {
                        font_size: px(24).into(),
                        ..default()
                    },
                ));
            }
        });
}
fn hud(
    screen: Res<State<Screen>>,
    driving: Res<Driving>,
    prompt: Res<Prompt>,
    health: Query<&crate::character::Health, With<crate::character::Player>>,
    mut text: Query<&mut Text, With<Hud>>,
) {
    for mut t in &mut text {
        t.0 = if *screen.get() != Screen::Playing {
            String::new()
        } else if health
            .single()
            .is_ok_and(|h| h.state != crate::character::LifeState::Healthy)
        {
            "Esc: menu".into()
        } else if driving.active() {
            format!(
                "{:.0} km/h  ·  {}  ·  gear {}\n{} {}  {}",
                driving.speed.abs() * 3.6,
                driving.name,
                driving.gear,
                prompt.key,
                prompt.text,
                prompt.noting()
            )
        } else {
            format!(
                "WASD: walk · Shift: run · Space: jump/traverse · Ctrl: slide · C: crouch · R: roll\n{} {}   Esc: menu",
                prompt.key, prompt.text
            )
        };
    }
}
fn sync_lighting(
    mut commands: Commands,
    suns: Query<Entity, With<crate::sky::Sun>>,
    sun: Res<crate::sky::SunLight>,
    ambient: Res<crate::sky::Ambient>,
    haze: Res<crate::sky::Haze>,
    mut painted: ResMut<crate::shading::Daylight>,
    mut fog: ResMut<crate::shading::Haze>,
) {
    for e in &suns {
        commands
            .entity(e)
            .insert(RenderLayers::from_layers(&[0, SHADOW_LAYER]));
    }
    painted.sun_color = [sun.color.r, sun.color.g, sun.color.b];
    painted.sun_brightness = sun.brightness;
    painted.ambient_color = [ambient.color.r, ambient.color.g, ambient.color.b];
    painted.ambient_brightness = ambient.brightness;
    fog.color = [haze.color.r, haze.color.g, haze.color.b];
    fog.distance = haze.distance;
    fog.end = haze.end;
    fog.veil = haze.veil;
}
#[cfg(not(target_arch = "wasm32"))]
fn capture(
    mut commands: Commands,
    session: Res<Session>,
    screen: Res<State<Screen>>,
    time: Res<Time<Real>>,
    mut ready_since: Local<Option<f32>>,
    mut requested: Local<bool>,
) {
    let Ok(path) = std::env::var("EARTH_TWO_CAPTURE") else {
        return;
    };
    if !session.ready || *screen.get() == Screen::Loading || *requested {
        return;
    }
    let since = *ready_since.get_or_insert(time.elapsed_secs());
    if time.elapsed_secs() - since < 3. {
        return;
    }
    use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    *requested = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

#[derive(Component)]
struct InjuryPanel;
fn injury_panel(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    players: Query<&crate::character::Health, With<crate::character::Player>>,
    panels: Query<Entity, With<InjuryPanel>>,
    mut previous: Local<Option<(Screen, Option<crate::character::Health>)>>,
    mut fonts: ResMut<Assets<Font>>,
    mut font: Local<Option<Handle<Font>>>,
) {
    let key = (*screen.get(), players.single().ok().copied());
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for e in &panels {
        commands.entity(e).despawn();
    }
    if *screen.get() != Screen::Playing {
        return;
    }
    let Some(text) = key.1.as_ref().and_then(super::injuries::injury_text) else {
        return;
    };
    // The Go font contains the middle dot; Bevy's subset default does not.
    let font = font
        .get_or_insert_with(|| {
            fonts.add(Font::from_bytes(
                include_bytes!("../../../../game/fonts/Inter-SemiBold.ttf").to_vec(),
            ))
        })
        .clone();
    commands
        .spawn((
            InjuryPanel,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                top: percent(30),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: px(380),
                        padding: UiRect::all(px(16)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.075, 0.10, 0.94)),
                ))
                .with_children(|panel| {
                    for (line, value) in text.lines().enumerate() {
                        panel.spawn((
                            Text::new(value),
                            TextFont {
                                font: font.clone().into(),
                                font_size: px(if line == 0 { 24 } else { 14 }).into(),
                                ..default()
                            },
                            TextColor(Color::srgb(0.89, 0.81, 0.64)),
                        ));
                    }
                });
        });
}

#[derive(Component)]
struct CrowdPanel;
fn crowd_panel(
    mut commands: Commands,
    crowd: Res<super::crowd::TestCrowd>,
    residents: Res<super::residents::Residents>,
    panels: Query<Entity, With<CrowdPanel>>,
    mut previous: Local<String>,
) {
    let text = if !crowd.shown {
        String::new()
    } else if crowd.editing {
        format!(
            "NPC population: {}_\nEnter: apply   Esc: cancel",
            crowd.digits
        )
    } else {
        format!(
            "F8  NPCs: {}\nF9  Population: {} ({} placed, {} active)\nF1  Hide",
            if crowd.enabled { "On" } else { "Off" },
            crowd.population,
            crowd.live,
            residents.near
        )
    };
    if *previous == text {
        return;
    }
    *previous = text.clone();
    for panel in &panels {
        commands.entity(panel).despawn();
    }
    if text.is_empty() {
        return;
    }
    commands.spawn((
        CrowdPanel,
        Node {
            position_type: PositionType::Absolute,
            right: px(20),
            top: px(20),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.075, 0.10, 0.94)),
        children![(
            Text::new(text),
            TextFont {
                font_size: px(16).into(),
                ..default()
            },
            TextColor(Color::srgb(0.89, 0.81, 0.64))
        )],
    ));
}
