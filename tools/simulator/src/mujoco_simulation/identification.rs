use std::{any::TypeId, fmt};

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};
use mujoco_rs::wrappers::mj_editing::{MjsBody, MjsGeom, MjsMaterial, MjsMesh, SpecObject};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<MujocoItems>()
        .register_type::<MujocoItemOf>()
        .register_type::<MujocoItemIndex>()
        .add_plugins((
            scope_plugin::<MjsMesh>,
            scope_plugin::<MjsMaterial>,
            scope_plugin::<MjsBody>,
            scope_plugin::<MjsGeom>,
        ));

    fn scope_plugin<Scope: SpecObject + 'static>(app: &mut App) {
        app.world_mut()
            .register_component_hooks::<MujocoItemName<Scope>>()
            .on_insert(MujocoItemName::<Scope>::on_inserted)
            .on_discard(MujocoItemName::<Scope>::on_discarded);
        app.world_mut()
            .register_component_hooks::<MujocoItemId<Scope>>()
            .on_insert(MujocoItemId::<Scope>::on_inserted)
            .on_discard(MujocoItemId::<Scope>::on_discarded);
    }
}

/// All entities that are part of a single Mujoco simulation instance.
#[derive(Component, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = MujocoItemOf, linked_spawn)]
pub struct MujocoItems(Vec<Entity>);

#[derive(Component, Reflect, Deref, Clone, PartialEq, Eq, Debug)]
#[reflect(Component)]
#[relationship(relationship_target = MujocoItems)]
pub struct MujocoItemOf(pub Entity);

#[derive(Component, Reflect, Debug, Deref, DerefMut, Clone, PartialEq, Eq, Hash)]
#[require(MujocoItemOf = Required)]
#[component(immutable)]
#[reflect(Component)]
pub struct MujocoItemName<Scope: SpecObject>(
    #[deref] String,
    #[reflect(ignore)] std::marker::PhantomData<fn() -> Scope>,
);

#[derive(Component, Reflect, Debug, Deref, DerefMut, Clone, Copy, PartialEq, Eq, Hash)]
#[require(MujocoItemOf = Required)]
#[component(immutable)]
#[reflect(Component)]
pub struct MujocoItemId<Scope: SpecObject>(
    #[deref] usize,
    #[reflect(ignore)] std::marker::PhantomData<fn() -> Scope>,
);

impl<Scope: SpecObject> MujocoItemName<Scope> {
    pub fn new(name: String) -> Self {
        Self(name, std::marker::PhantomData)
    }
}

impl<Scope: SpecObject> MujocoItemId<Scope> {
    pub fn new(id: usize) -> Self {
        Self(id, std::marker::PhantomData)
    }
}

impl<Scope: SpecObject> fmt::Debug for MujocoItemName<Scope> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MujocoItemName")
            .field(&ShortName::of::<Scope>())
            .field(&self.0)
            .finish()
    }
}

impl<Scope: SpecObject> fmt::Debug for MujocoItemId<Scope> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MujocoItemId")
            .field(&ShortName::of::<Scope>())
            .field(&self.0)
            .finish()
    }
}

#[derive(Component, Reflect, Debug, Default)]
pub struct MujocoItemIndex {
    by_name: HashMap<(TypeId, String), Entity>,
    by_id: HashMap<(TypeId, usize), Entity>,
}

impl MujocoItemIndex {
    fn remove_name<Scope: SpecObject + 'static>(&mut self, name: &str, entity: Entity) {
        let key = (TypeId::of::<Scope>(), name.to_owned());
        if self.by_name.get(&key) == Some(&entity) {
            self.by_name.remove(&key);
        }
    }

    fn remove_id<Scope: SpecObject + 'static>(&mut self, id: usize, entity: Entity) {
        let key = (TypeId::of::<Scope>(), id);
        if self.by_id.get(&key) == Some(&entity) {
            self.by_id.remove(&key);
        }
    }

    pub fn get_by_name<Scope: SpecObject + 'static>(&self, name: &str) -> Option<Entity> {
        self.by_name
            .get(&(TypeId::of::<Scope>(), name.to_owned()))
            .copied()
    }

    pub fn get_by_id<Scope: SpecObject + 'static>(&self, id: usize) -> Option<Entity> {
        self.by_id.get(&(TypeId::of::<Scope>(), id)).copied()
    }

    pub fn iter_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.by_name.values().copied()
    }
}

// impl MujocoItemIndex {
//     #[allow(clippy::type_complexity)]
//     fn on_index_inserted(
//         e: On<Insert, MujocoItemIndex>,
//         mut index: Query<&mut MujocoItemIndex>,
//         meshes: Query<(
//             Entity,
//             &MujocoItemName<MjsMesh>,
//             Option<&MujocoItemId<MjsMesh>>,
//         )>,
//         bodies: Query<(
//             Entity,
//             &MujocoItemName<MjsBody>,
//             Option<&MujocoItemId<MjsBody>>,
//         )>,
//         geoms: Query<(
//             Entity,
//             &MujocoItemName<MjsGeom>,
//             Option<&MujocoItemId<MjsGeom>>,
//         )>,
//     ) -> Result {
//         let mut index = index.get_mut(e.entity)?;

//         fn insert_index<T: SpecObject + 'static>(
//             index: &mut MujocoItemIndex,
//             query: &Query<(Entity, &MujocoItemName<T>, Option<&MujocoItemId<T>>)>,
//         ) {
//             for (entity, name, id) in query.iter() {
//                 index
//                     .by_name
//                     .insert((TypeId::of::<T>(), name.0.clone()), entity);
//                 if let Some(id) = id {
//                     index.by_id.insert((TypeId::of::<T>(), id.0), entity);
//                 }
//             }
//         }

//         insert_index::<MjsMesh>(&mut index, &meshes);
//         insert_index::<MjsBody>(&mut index, &bodies);
//         insert_index::<MjsGeom>(&mut index, &geoms);

//         Ok(())
//     }
// }

impl<Scope: SpecObject + 'static> MujocoItemName<Scope> {
    fn on_inserted(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (name, item) = {
            let (name, item) = world
                .entity(entity)
                .get_components::<(&Self, &MujocoItemOf)>()
                .expect("Invariant violated: MujocoItemName entity should have MujocoItemOf");

            (name.0.clone(), item.0)
        };

        let mut e = world.entity_mut(item);
        let mut index = e
            .get_components_mut::<&mut MujocoItemIndex>()
            .expect("MujocoItemOf entity should have MujocoItemIndex");

        index.by_name.insert((TypeId::of::<Scope>(), name), entity);
    }

    fn on_discarded(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (name, item) = {
            let (name, item) = world
                .entity(entity)
                .get_components::<(&Self, &MujocoItemOf)>()
                .expect("Invariant violated: MujocoItemName entity should have MujocoItemOf");

            (name.0.clone(), item.0)
        };

        // Linked despawn can remove the target index before this item's hooks run.
        if let Some(mut index) = world.get_mut::<MujocoItemIndex>(item) {
            index.remove_name::<Scope>(&name, entity);
        }
    }

    // fn on_inserted(
    //     e: On<Insert, MujocoItemName<Scope>>,
    //     name: Query<(&MujocoItemName<Scope>, &MujocoItemOf)>,
    //     mut index: Query<&mut MujocoItemIndex>,
    // ) -> Result {
    //     let (name, item) = name.get(e.entity)?;
    //     let mut index = index.get_mut(item.0)?;

    //     index
    //         .by_name
    //         .insert((TypeId::of::<Scope>(), name.0.clone()), e.entity);

    //     Ok(())
    // }

    // fn on_removed(
    //     e: On<Remove, MujocoItemName<Scope>>,
    //     name: Query<(&MujocoItemName<Scope>, &MujocoItemOf)>,
    //     mut index: Query<&mut MujocoItemIndex>,
    // ) -> Result {
    //     let (name, item) = name.get(e.entity)?;
    //     let mut index = index.get_mut(item.0)?;

    //     index
    //         .by_name
    //         .remove(&(TypeId::of::<Scope>(), name.0.clone()));

    //     Ok(())
    // }
}

impl<Scope: SpecObject + 'static> MujocoItemId<Scope> {
    fn on_inserted(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (id, item) = {
            let (id, item) = world
                .entity(entity)
                .get_components::<(&Self, &MujocoItemOf)>()
                .expect("Invariant violated: MujocoItemId entity should have MujocoItemOf");

            (id.0, item.0)
        };

        let mut e = world.entity_mut(item);
        let mut index = e
            .get_components_mut::<&mut MujocoItemIndex>()
            .expect("MujocoItemOf entity should have MujocoItemIndex");

        index.by_id.insert((TypeId::of::<Scope>(), id), entity);
    }

    fn on_discarded(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (id, item) = {
            let (id, item) = world
                .entity(entity)
                .get_components::<(&Self, &MujocoItemOf)>()
                .expect("Invariant violated: MujocoItemId entity should have MujocoItemOf");

            (id.0, item.0)
        };

        // Linked despawn can remove the target index before this item's hooks run.
        if let Some(mut index) = world.get_mut::<MujocoItemIndex>(item) {
            index.remove_id::<Scope>(id, entity);
        }
    }

    // fn on_inserted(
    //     e: On<Insert, MujocoItemId<Scope>>,
    //     id: Query<(&MujocoItemId<Scope>, &MujocoItemOf)>,
    //     mut index: Query<&mut MujocoItemIndex>,
    // ) -> Result {
    //     let (id, item) = id.get(e.entity)?;
    //     let mut index = index.get_mut(item.0)?;

    //     index.by_id.insert((TypeId::of::<Scope>(), id.0), e.entity);

    //     Ok(())
    // }

    // fn on_removed(
    //     e: On<Remove, MujocoItemId<Scope>>,
    //     id: Query<(&MujocoItemId<Scope>, &MujocoItemOf)>,
    //     mut index: Query<&mut MujocoItemIndex>,
    // ) -> Result {
    //     let (id, item) = id.get(e.entity)?;
    //     let mut index = index.get_mut(item.0)?;

    //     index.by_id.remove(&(TypeId::of::<Scope>(), id.0));

    //     Ok(())
    // }
}

/// Panics if the required component is missing.
pub struct Required;

impl Required {
    pub fn into<T: Component>(self) -> T {
        panic!("{} component is required", std::any::type_name::<T>())
    }
}
