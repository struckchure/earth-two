//! Connect game/cull_people.go's existing distance rules to live characters.
use crate::{
    character::{Body, CharacterController, Player},
    landfall::cull_people::{PeopleCull, PeopleCuller, Person, PersonDetail},
    presentation::{
        cloth::{Clothed, Rigid},
        distant::Distant,
        outfit::Garment,
        verlet::Cloth,
    },
};
use bevy::prelude::*;
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detail(pub PersonDetail);
struct CharacterCull;
pub struct PeoplePlugin;
impl Plugin for PeoplePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PeopleCuller(Box::new(CharacterCull)));
        #[cfg(feature = "viewer")]
        app.add_systems(
            PostUpdate,
            render_detail.after(bevy::transform::TransformSystems::Propagate),
        );
    }
}
impl PeopleCull for CharacterCull {
    fn people(&self, world: &mut World) -> Vec<Person> {
        let players: std::collections::HashSet<_> = world
            .query_filtered::<Entity, With<Player>>()
            .iter(world)
            .collect();
        world
            .query_filtered::<(Entity, &GlobalTransform, &ChildOf), With<Body>>()
            .iter(world)
            .map(|(body, at, parent)| Person {
                body,
                at: at.translation(),
                player: players.contains(&parent.parent()),
            })
            .collect()
    }
    fn apply(&self, world: &mut World, person: &Person, detail: &PersonDetail) {
        let Some(parent) = world.get::<ChildOf>(person.body) else {
            return;
        };
        let root = parent.parent();
        let old = world.get::<Detail>(person.body).copied();
        world.entity_mut(person.body).insert(Detail(*detail));
        if detail.distant {
            world.entity_mut(person.body).insert(Distant);
        } else {
            world.entity_mut(person.body).remove::<Distant>();
        }
        if let Some(mut controller) = world.get_mut::<CharacterController>(root) {
            controller.every = detail.every;
        }
        if detail.cloth {
            world.entity_mut(root).remove::<Rigid>();
        } else {
            world.entity_mut(root).insert(Rigid);
        }
        if old.is_none_or(|d| d.0.cloth != detail.cloth) {
            let garments: Vec<_> = world
                .query_filtered::<(Entity, &ChildOf), With<Garment>>()
                .iter(world)
                .filter_map(|(e, p)| (p.parent() == person.body).then_some(e))
                .collect();
            for garment in garments {
                world.entity_mut(garment).remove::<Clothed>();
                if !detail.cloth {
                    world.entity_mut(garment).remove::<Cloth>();
                }
            }
        }
    }
}

// Set layers on meshes, not just the model root (Bevy layers aren't inherited).
// Inspect new descendants too: GLB and wardrobe scenes arrive asynchronously.
// Keep authored hidden skin parts and seated visibility intact.
#[cfg(feature = "viewer")]
#[allow(clippy::type_complexity)]
fn render_detail(
    mut commands: Commands,
    bodies: Query<(Entity, &Detail), With<Body>>,
    children: Query<&Children>,
    meshes: Query<
        (
            Option<&bevy::camera::visibility::RenderLayers>,
            Has<bevy::light::NotShadowCaster>,
            Has<crate::shading::plugin::OutlineHull>,
        ),
        With<Mesh3d>,
    >,
) {
    use crate::landfall::{
        cull::Drawn,
        render::{SEEN_LAYER, SHADOW_LAYER},
    };
    use bevy::{camera::visibility::RenderLayers, light::NotShadowCaster};
    for (body, detail) in &bodies {
        let d = detail.0;
        for e in children.iter_descendants(body) {
            let Ok((current, no_shadow, outline)) = meshes.get(e) else {
                continue;
            };
            let layer = if outline && d.distant {
                RenderLayers::none()
            } else {
                match d.drawn {
                    Drawn::Seen => RenderLayers::layer(SEEN_LAYER),
                    Drawn::ShadowOnly => RenderLayers::layer(SHADOW_LAYER),
                    Drawn::Unseen => RenderLayers::none(),
                }
            };
            if current != Some(&layer) {
                commands.entity(e).insert(layer);
            }
            if !outline && no_shadow == d.casts {
                if d.casts {
                    commands.entity(e).remove::<NotShadowCaster>();
                } else {
                    commands.entity(e).insert(NotShadowCaster);
                }
            }
        }
    }
}

#[cfg(all(test, feature = "viewer"))]
mod tests {
    use super::*;
    use crate::{landfall::cull::Drawn, shading::plugin::OutlineHull};
    use bevy::{camera::visibility::RenderLayers, light::NotShadowCaster};
    #[test]
    fn late_meshes_receive_detail_without_revealing_hidden_skin() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, render_detail);
        let body = app
            .world_mut()
            .spawn((
                Body,
                Detail(PersonDetail {
                    drawn: Drawn::Unseen,
                    distant: true,
                    casts: false,
                    cloth: false,
                    every: 6,
                }),
            ))
            .id();
        app.update(); // Detail was already chosen before the model arrived.
        let mesh = app
            .world_mut()
            .spawn((ChildOf(body), Mesh3d::default(), Visibility::Hidden))
            .id();
        let outline = app
            .world_mut()
            .spawn((
                ChildOf(mesh),
                Mesh3d::default(),
                OutlineHull,
                NotShadowCaster,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<RenderLayers>(mesh),
            Some(&RenderLayers::none())
        );
        assert_eq!(
            app.world().get::<RenderLayers>(outline),
            Some(&RenderLayers::none())
        );
        assert!(app.world().get::<NotShadowCaster>(mesh).is_some());
        app.world_mut()
            .entity_mut(body)
            .insert(Detail(PersonDetail {
                drawn: Drawn::ShadowOnly,
                distant: false,
                casts: true,
                ..default()
            }));
        app.update();
        assert_eq!(
            app.world().get::<RenderLayers>(mesh),
            Some(&RenderLayers::layer(crate::landfall::render::SHADOW_LAYER))
        );
        assert!(app.world().get::<NotShadowCaster>(mesh).is_none());
        assert!(app.world().get::<NotShadowCaster>(outline).is_some());
        app.world_mut()
            .entity_mut(body)
            .insert(Detail(PersonDetail::default()));
        app.update();
        assert_eq!(
            app.world().get::<RenderLayers>(outline),
            Some(&RenderLayers::layer(0))
        );
        assert_eq!(
            app.world().get::<Visibility>(mesh),
            Some(&Visibility::Hidden)
        );
    }
}
