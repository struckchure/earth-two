//! Each entity's children: character/family.go. Bevy keeps
//! Children itself, but the systems that walk a character's body and clothes
//! every frame for everyone read this one index, refreshed in PreUpdate, so they
//! see one consistent snapshot: what's spawned during the frame shows next
//! frame, what's despawned is left out as it's handed over.

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource, Default, Debug)]
pub struct Family {
    kids: HashMap<Entity, Vec<Entity>>,
    alive: std::collections::HashSet<Entity>,
}

impl Family {
    /// children of parent still alive.
    pub fn children(&self, parent: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.kids
            .get(&parent)
            .into_iter()
            .flatten()
            .copied()
            .filter(move |c| self.alive.contains(c))
    }

    /// each_child calls f for each of parent's children still alive.
    pub fn each_child(&self, parent: Entity, mut f: impl FnMut(Entity)) {
        for c in self.children(parent) {
            f(c);
        }
    }

    /// each_descendant calls f for every descendant of parent still alive,
    /// parents before their children.
    pub fn each_descendant(&self, parent: Entity, f: &mut impl FnMut(Entity)) {
        for c in self.children(parent) {
            f(c);
            self.each_descendant(c, f);
        }
    }

    /// rebuild makes the index afresh from the parent of every child.
    pub fn rebuild(&mut self, pairs: impl IntoIterator<Item = (Entity, Entity)>) {
        self.kids.clear();
        self.alive.clear();
        for (child, parent) in pairs {
            self.kids.entry(parent).or_default().push(child);
            self.alive.insert(child);
        }
    }

    /// forget drops a child that's gone since the index was built.
    pub fn forget(&mut self, child: Entity) {
        self.alive.remove(&child);
    }
}

/// Refresh the frame's snapshot only when the hierarchy changes. Animation
/// moves transforms every frame but does not change any parent relationships.
pub fn index_family(
    mut fam: ResMut<Family>,
    children: Query<(Entity, &ChildOf)>,
    changed: Query<(), Changed<ChildOf>>,
    mut removed: RemovedComponents<ChildOf>,
    mut initialized: Local<bool>,
) {
    let had_removals = removed.read().count() > 0;
    if !*initialized || !changed.is_empty() || had_removals {
        fam.rebuild(children.iter().map(|(e, c)| (e, c.parent())));
        *initialized = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_tracks_reparent_removal_and_despawn() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Family>()
            .add_systems(PreUpdate, index_family);
        let a = app.world_mut().spawn_empty().id();
        let b = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(a)).id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<Family>()
                .children(a)
                .collect::<Vec<_>>(),
            vec![child]
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<Family>()
                .children(a)
                .collect::<Vec<_>>(),
            vec![child]
        );
        app.world_mut().entity_mut(child).insert(ChildOf(b));
        app.update();
        assert_eq!(app.world().resource::<Family>().children(a).count(), 0);
        assert_eq!(
            app.world()
                .resource::<Family>()
                .children(b)
                .collect::<Vec<_>>(),
            vec![child]
        );
        app.world_mut().entity_mut(child).remove::<ChildOf>();
        app.update();
        assert_eq!(app.world().resource::<Family>().children(b).count(), 0);
        app.world_mut().entity_mut(child).insert(ChildOf(a));
        app.update();
        app.world_mut().entity_mut(child).despawn();
        app.update();
        assert_eq!(app.world().resource::<Family>().children(a).count(), 0);
    }

    #[test]
    fn descendants_parents_first_and_dead_left_out() {
        let mut fam = Family::default();
        let (a, b, c, d) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
            Entity::from_raw_u32(4).unwrap(),
        );
        fam.rebuild([(b, a), (c, b), (d, a)]);
        let mut seen = vec![];
        fam.each_descendant(a, &mut |e| seen.push(e));
        assert_eq!(seen, vec![b, c, d]);
        fam.forget(b);
        let mut seen = vec![];
        fam.each_descendant(a, &mut |e| seen.push(e));
        assert_eq!(seen, vec![d]);
    }
}
