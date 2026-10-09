//! Each entity's children, once a frame: character/family.go. Bevy keeps
//! Children itself, but the systems that walk a character's body and clothes
//! every frame for everyone read this one index, built in PreUpdate, so they
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

/// index_family builds the Family afresh: every child, under its parent.
pub fn index_family(mut fam: ResMut<Family>, children: Query<(Entity, &ChildOf)>) {
    fam.rebuild(children.iter().map(|(e, c)| (e, c.parent())));
}

#[cfg(test)]
mod tests {
    use super::*;

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
