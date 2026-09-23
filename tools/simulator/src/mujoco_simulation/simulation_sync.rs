use std::collections::BTreeSet;
use std::sync::LazyLock;

use bevy::{asset::AssetEventSystems, prelude::*};
use mujoco_rs::prelude::*;
use mujoco_rs::wrappers::SpecItem;
use mujoco_rs::wrappers::mj_editing::{MjsBody, MjsGeom, MjsMaterial, MjsMesh};

use crate::mujoco_simulation::identification::{
    MujocoItemId, MujocoItemIndex, MujocoItemName, MujocoItemOf,
};

use super::{
    asset_loader::{MujocoSpec, MujocoSpecId},
    components::*,
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        check_spec_asset_modified.after(AssetEventSystems),
        // (init_simulation, sync_simulation_models)
        //     .chain()
        //     .after(AssetEventSystems),
    )
    .add_observer(on_simulation_inserted)
    .add_observer(recompile_spec)
    .add_observer(sync_simulation_hierarchy);
    // .add_systems(FixedUpdate, sync_simulation_data);
}

// Triggered for every item entity that needs to be updated after a MujocoSpec
// is recompiled.
#[derive(EntityEvent, Reflect, Debug, Deref, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MujocoSyncData<Scope: SpecItem>(
    #[event_target]
    #[deref]
    pub Entity,
    #[reflect(ignore)] std::marker::PhantomData<fn() -> Scope>,
);

impl<Scope: SpecItem> MujocoSyncData<Scope> {
    pub fn new(entity: Entity) -> Self {
        Self(entity, std::marker::PhantomData)
    }
}

/// Triggered right after successful (re-)compilation and forward computation.
#[derive(EntityEvent, Reflect, Debug, Deref, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MujocoSpecRecompiled(pub Entity);

#[derive(EntityEvent, Reflect, Debug, Deref, Clone, Copy, PartialEq, Eq, Hash)]
struct Recompile(pub Entity);

/// Tracks the current identity of referenced MujocoSpec, to track source
/// replacement
#[derive(Component, PartialEq, Eq)]
struct SpecIdentity(MujocoSpecId);

fn on_simulation_inserted(
    e: On<Insert<MujocoSimulation>>,
    spec_assets: Res<Assets<MujocoSpec>>,
    sim: Query<&MujocoSimulation>,
    mut commands: Commands,
) {
    let sim = sim.get(e.entity).unwrap();
    if spec_assets.contains(&sim.0) {
        commands.trigger(Recompile(e.entity));
    }
}

#[allow(clippy::type_complexity)]
fn check_spec_asset_modified(
    sims: Query<
        (Entity, &MujocoSimulation),
        Or<(Changed<MujocoSimulation>, AssetChanged<MujocoSimulation>)>,
    >,
    spec_assets: Res<Assets<MujocoSpec>>,
    mut commands: Commands,
) {
    for (entity, sim) in sims.iter() {
        if spec_assets.contains(&sim.0) {
            commands.trigger(Recompile(entity));
        }
    }
}

fn recompile_spec(
    e: On<Recompile>,
    mut sims: Query<(
        &MujocoSimulation,
        Option<&mut MujocoSimulationData>,
        Option<&SpecIdentity>,
    )>,
    mut spec_assets: ResMut<Assets<MujocoSpec>>,
    mut commands: Commands,
) -> Result {
    let (sim, data, compiled_source) = sims.get_mut(e.0).unwrap();
    let spec = spec_assets
        .get_mut_untracked(&sim.0)
        .ok_or("Spec asset does not exist")?;

    let source = SpecIdentity(spec.instance_id());

    static EMPTY_DATA: LazyLock<MjData<ModelHandle>> = LazyLock::new(|| {
        MjData::new(ModelHandle::new(
            MjModel::from_xml_string("<mujoco></mujoco>").unwrap(),
        ))
    });

    if let Some(mut data) = data.filter(|_| compiled_source == Some(&source)) {
        if data.revision == spec.revision() {
            info!("Spec revision has not changed, skipping recompile");
            return Ok(());
        }

        let tmp_data = std::mem::replace(&mut data.data, EMPTY_DATA.clone());

        match spec.recompile(tmp_data) {
            Ok(mut tmp_data) => {
                // mj_recompile restores integration state without refreshing
                // derived poses. Compute them before publishing the new data.
                tmp_data.forward();

                data.data = tmp_data;
                data.revision = spec.revision();
            }
            Err(err) => {
                error!("Failed to recompile MujocoSpec: {:?}", err);
                // MjData and MjModel have been deleted
                commands.entity(e.entity()).remove::<MujocoSimulationData>();
                return Ok(());
            }
        }
    } else {
        info!("Compiling MujocoSpec for simulation {:?}", sim.0);
        let model = match spec.compile() {
            Ok(model) => model,
            Err(e) => {
                error!("Failed to compile MujocoSpec: {:?}", e);
                return Ok(());
            }
        };

        let mut data = MjData::new(ModelHandle::new(model));
        data.forward();

        commands.entity(e.entity()).insert((
            MujocoSimulationData {
                data,
                revision: spec.revision(),
            },
            source,
        ));
    }

    commands.trigger(MujocoSpecRecompiled(e.entity()));

    Ok(())
}

fn sync_simulation_hierarchy(
    e: On<MujocoSpecRecompiled>,
    sim: Query<(&MujocoSimulation, &MujocoItemIndex)>,
    mut spec_assets: ResMut<Assets<MujocoSpec>>,
    mut commands: Commands,
) -> Result {
    info!("Syncing simulation hierarchy for {:?}", e.entity());

    let Ok((sim, index)) = sim.get(e.entity()) else {
        error!("Simulation entity should have MujocoSimulationData after recompile");
        return Ok(());
    };

    let spec = spec_assets
        .get_mut_untracked(&sim.0)
        .ok_or("Spec asset does not exist")?
        .spec();

    let mut entities_to_retain = BTreeSet::new();

    for mesh in spec.mesh_iter() {
        if let Some(entity) = index.get_by_name::<MjsMesh>(mesh.name()) {
            entities_to_retain.insert(entity);
            commands
                .entity(entity)
                .trigger(MujocoSyncData::<MjsMesh>::new);
        } else {
            let mut entity_commands = commands.spawn((
                MujocoItemName::<MjsMesh>::new(mesh.name().to_owned()),
                MujocoItemOf(e.entity()),
                ChildOf(e.entity()),
                Name::new(format!("{} (Asset type: MjsMesh)", mesh.name())),
            ));
            if let Some(id) = mesh.id() {
                entity_commands.insert(MujocoItemId::<MjsMesh>::new(id));
            }
            entities_to_retain.insert(entity_commands.id());
            entity_commands.trigger(MujocoSyncData::<MjsMesh>::new);
        }
    }

    for material in spec.material_iter() {
        if let Some(entity) = index.get_by_name::<MjsMaterial>(material.name()) {
            entities_to_retain.insert(entity);
            commands
                .entity(entity)
                .trigger(MujocoSyncData::<MjsMaterial>::new);
        } else {
            let mut entity_commands = commands.spawn((
                MujocoItemName::<MjsMaterial>::new(material.name().to_owned()),
                MujocoItemOf(e.entity()),
                ChildOf(e.entity()),
                Name::new(format!("{} (Asset type: MjsMaterial)", material.name())),
            ));
            if let Some(id) = material.id() {
                entity_commands.insert(MujocoItemId::<MjsMaterial>::new(id));
            }
            entities_to_retain.insert(entity_commands.id());
            entity_commands.trigger(MujocoSyncData::<MjsMaterial>::new);
        }
    }

    fn recurse(
        body: &MjsBody,
        parent: Entity,
        sim: Entity,
        index: &MujocoItemIndex,
        commands: &mut Commands,
        entities_to_retain: &mut BTreeSet<Entity>,
    ) {
        let body_entity = if let Some(entity) = index.get_by_name::<MjsBody>(body.name()) {
            entities_to_retain.insert(entity);
            commands
                .entity(entity)
                .insert(ChildOf(parent))
                .trigger(MujocoSyncData::<MjsBody>::new);
            entity
        } else {
            let mut entity_commands = commands.spawn((
                MujocoItemName::<MjsBody>::new(body.name().to_owned()),
                MujocoItemOf(sim),
                ChildOf(parent),
                Name::new(format!("{} (MjsBody)", body.name())),
            ));
            if let Some(id) = body.id() {
                entity_commands.insert(MujocoItemId::<MjsBody>::new(id));
            }
            entities_to_retain.insert(entity_commands.id());
            entity_commands.trigger(MujocoSyncData::<MjsBody>::new);
            entity_commands.id()
        };

        for child in body.body_iter(false) {
            recurse(child, body_entity, sim, index, commands, entities_to_retain);
        }

        for geom in body.geom_iter(false) {
            if let Some(entity) = index.get_by_name::<MjsGeom>(geom.name()) {
                entities_to_retain.insert(entity);
                commands
                    .entity(entity)
                    .insert(ChildOf(body_entity))
                    .trigger(MujocoSyncData::<MjsGeom>::new);
            } else {
                let mut entity_commands = commands.spawn((
                    MujocoItemName::<MjsGeom>::new(geom.name().to_owned()),
                    MujocoItemOf(sim),
                    ChildOf(body_entity),
                    Name::new(format!("{} (MjsGeom)", geom.name())),
                ));
                if let Some(id) = geom.id() {
                    entity_commands.insert(MujocoItemId::<MjsGeom>::new(id));
                }
                entities_to_retain.insert(entity_commands.id());
                entity_commands.trigger(MujocoSyncData::<MjsGeom>::new);
            }
        }
    }

    recurse(
        spec.world_body(),
        e.entity(),
        e.entity(),
        index,
        &mut commands,
        &mut entities_to_retain,
    );

    for entity in index.iter_entities() {
        if !entities_to_retain.contains(&entity) {
            info!(
                "Despawning entity {:?} because it is no longer in the spec",
                entity
            );
            commands.entity(entity).despawn();
        }
    }

    Ok(())
}

// #[allow(clippy::complexity)]
// fn init_simulation(
//     query: Query<
//         (Entity, &MujocoSimulation, Option<&MujocoSimulationData>),
//         (
//             // Without<MujocoSimulationData>,
//             Or<(Changed<MujocoSimulation>, AssetChanged<MujocoSimulation>)>,
//         ),
//     >,
//     mut spec_assets: ResMut<Assets<MujocoSpec>>,
//     mut commands: Commands,
// ) {
//     for (entity, sim, data) in query.iter() {
//         // Compilation needs exclusive access but is not a user edit to the asset.
//         let Some(spec) = spec_assets.get_mut_untracked(&sim.0) else {
//             continue;
//         };

//         if let Some(_data) = data {
//             // TODO: Handle re-compiling
//             info!(
//                 "MujocoSpec asset changed, should re-compile now: {:?}",
//                 sim.0
//             );
//             continue;
//         } else {
//             let model = match spec.compile() {
//                 Ok(model) => model,
//                 Err(e) => {
//                     error!("Failed to compile MujocoSpec: {:?}", e);
//                     continue;
//                 }
//             };

//             commands.entity(entity).insert((MujocoSimulationData {
//                 data: MjData::new(ModelHandle::new(model)),
//                 revision: 0,
//             },));
//         }
//     }
// }

// #[allow(clippy::complexity)]
// fn sync_simulation_models(
//     simulations: Query<
//         (Entity, &MujocoSimulation),
//         Or<(Changed<MujocoSimulation>, AssetChanged<MujocoSimulation>)>,
//     >,
//     mut spec_assets: ResMut<Assets<MujocoSpec>>,
//     children: Query<&Children>,
//     items: Query<(Option<&MujocoBody>, Option<&MujocoGeometry>)>,
//     mut commands: Commands,
// ) {
//     for (entity, sim) in &simulations {
//         // SyncCell requires exclusive access even for reading the spec. Do not
//         // emit Modified here: that would trigger another sync next frame.
//         let Some(spec) = spec_assets.get_mut_untracked(&sim.0) else {
//             continue;
//         };

//         let spec = spec.spec();

//         sync_model_children(
//             entity,
//             entity,
//             std::iter::once(spec.world_body()),
//             spec.world_body().geom_iter(false),
//             &children,
//             &items,
//             spec,
//             &mut commands,
//         );
//     }
// }

// #[derive(PartialEq, Eq, Hash)]
// enum ModelItem {
//     Body(String),
//     Geometry(String),
// }

// /// Match within each parent by kind and name. Unnamed siblings are matched in
// /// order, since the spec provides no persistent name for them. Renaming or
// /// moving an item to another parent creates a new entity.
// #[allow(clippy::too_many_arguments)]
// fn sync_model_children<'a>(
//     simulation_entity: Entity,
//     parent: Entity,
//     bodies: impl IntoIterator<Item = &'a MjsBody>,
//     geometries: impl IntoIterator<Item = &'a MjsGeom>,
//     children: &Query<&Children>,
//     items: &Query<(Option<&MujocoBody>, Option<&MujocoGeometry>)>,
//     spec: &MjSpec,
//     commands: &mut Commands,
// ) {
//     let mut existing: HashMap<ModelItem, std::collections::VecDeque<Entity>> = HashMap::new();
//     if let Ok(children) = children.get(parent) {
//         for child in children.iter() {
//             let Ok((body, geometry)) = items.get(child) else {
//                 continue;
//             };
//             let key = if let Some(body) = body {
//                 ModelItem::Body(body.name.clone())
//             } else if let Some(geometry) = geometry {
//                 ModelItem::Geometry(geometry.name.clone())
//             } else {
//                 // These entities are not owned by the spec mirror.
//                 continue;
//             };
//             existing.entry(key).or_default().push_back(child);
//         }
//     }

//     for body in bodies {
//         let name = body.name().to_owned();
//         let entity = existing
//             .get_mut(&ModelItem::Body(name.clone()))
//             .and_then(|entities| entities.pop_front())
//             .unwrap_or_else(|| {
//                 let pos = body.pos().to_bevy();

//                 commands
//                     .spawn((
//                         ChildOf(parent),
//                         MujocoSimulationItem(simulation_entity),
//                         Transform::from_translation(pos),
//                     ))
//                     .id()
//             });

//         commands
//             .entity(entity)
//             .insert((Name::new(name.clone()), MujocoBody { name }));

//         sync_model_children(
//             simulation_entity,
//             entity,
//             body.body_iter(false),
//             body.geom_iter(false),
//             children,
//             items,
//             spec,
//             commands,
//         );
//     }

//     for geometry in geometries {
//         let name = geometry.name().to_owned();

//         let Some(mesh) = spec.mesh(geometry.meshname()) else {
//             continue;
//         };

//         let mesh_file = mesh.file().to_owned();

//         let entity = existing
//             .get_mut(&ModelItem::Geometry(name.clone()))
//             .and_then(|entities| entities.pop_front())
//             .unwrap_or_else(|| {
//                 commands
//                     .spawn((ChildOf(parent), MujocoSimulationItem(simulation_entity)))
//                     .id()
//             });

//         if let Err(e) = sync_geom(geometry, spec, commands.entity(entity)) {
//             error!("Failed to sync geometry {}: {:?}", name, e);
//         }
//     }

//     // Despawning a body also removes its descendants through ChildOf.
//     for entity in existing.into_values().flatten() {
//         commands.entity(entity).despawn();
//     }
// }

// fn sync_geom(geom: &MjsGeom, spec: &MjSpec, mut entity: EntityCommands) -> Result<()> {
//     let type_ = match geom.type_() {
//         MjtGeom::mjGEOM_PLANE => MujocoGeometryType::Plane,
//         MjtGeom::mjGEOM_HFIELD => MujocoGeometryType::HeightField(
//             spec.mesh(geom.meshname())
//                 .ok_or("Height field mesh not found")?
//                 .file()
//                 .to_owned(),
//         ),
//         MjtGeom::mjGEOM_SPHERE => MujocoGeometryType::Sphere,
//         MjtGeom::mjGEOM_CAPSULE => MujocoGeometryType::Capsule,
//         MjtGeom::mjGEOM_ELLIPSOID => MujocoGeometryType::Ellipsoid,
//         MjtGeom::mjGEOM_CYLINDER => MujocoGeometryType::Cylinder,
//         MjtGeom::mjGEOM_BOX => MujocoGeometryType::Box,
//         MjtGeom::mjGEOM_MESH => MujocoGeometryType::Mesh(
//             spec.mesh(geom.meshname())
//                 .ok_or("Mesh not found")?
//                 .file()
//                 .to_owned(),
//         ),
//         t => Err(format!("Unsupported geometry type: {:?}", t))?,
//     };

//     // spec.world_body().geom_iter().last().unwrap().user_value_mut(key);
//     entity.insert((
//         Name::new(geom.name().to_owned()),
//         Transform {
//             translation: geom.pos().to_bevy(),
//             rotation: geom.quat().to_bevy(),
//             scale: geom.size().to_bevy(),
//         },
//         MujocoGeometry {
//             name: geom.name().to_owned(),
//             type_,
//             material: if geom.material().is_empty() {
//                 MujocoMaterial::Rgba(geom.rgba().to_bevy())
//             } else {
//                 warn!(
//                     "Mujoco materials are not supported yet. Material used: {}",
//                     geom.material()
//                 );
//                 MujocoMaterial::Material(geom.material().to_owned())
//             },
//         },
//     ));

//     Ok(())
// }

// fn sync_simulation_data(
//     sim_data: Query<&MujocoSimulationData>,
//     mut sim_bodies: Query<(&MujocoBody, &MujocoSimulationItem, &mut Transform)>,
// ) {
//     for (body, &MujocoSimulationItem(data_entity), mut transform) in &mut sim_bodies {
//         let Some(data) = sim_data.get(data_entity).ok() else {
//             continue;
//         };

//         // let body = data.body(&body.name).unwrap().view(data);

//         // let pos = body.xpos.to_bevy();
//         // let rot = body.xquat.to_bevy();

//         // transform.translation = pos;
//         // transform.rotation = rot;

//         info!("Syncing body {} with data", body.name);

//         let id = data.body(&body.name).unwrap().id;
//         let parent_id = data.model().body_parentid()[id] as usize;

//         let position: DVec3 = data.xpos()[id].to_bevy();
//         let parent_position: DVec3 = data.xpos()[parent_id].to_bevy();

//         let rotation = |id: usize| -> DQuat { data.xquat()[id].to_bevy() };

//         let parent_inverse = rotation(parent_id).inverse();

//         transform.translation = (parent_inverse * (position - parent_position)).as_vec3();
//         transform.rotation = (parent_inverse * rotation(id)).as_quat();
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn app() -> App {
//         let mut app = App::new();
//         app.add_plugins((MinimalPlugins, AssetPlugin::default()))
//             .init_asset::<MujocoSpec>()
//             .add_systems(PostUpdate, sync_simulation_models.after(AssetEventSystems));
//         app
//     }

//     fn named_child(world: &World, parent: Entity, name: &str) -> Entity {
//         world
//             .get::<Children>(parent)
//             .unwrap()
//             .iter()
//             .find(|&entity| {
//                 world
//                     .get::<Name>(entity)
//                     .is_some_and(|n| n.as_str() == name)
//             })
//             .unwrap()
//     }

//     #[test]
//     fn waits_for_asset_and_initializes_each_simulation_independently() {
//         let mut app = app();
//         let handle = app
//             .world()
//             .resource::<Assets<MujocoSpec>>()
//             .reserve_handle();
//         let first = app.world_mut().spawn(MujocoSimulation(handle.clone())).id();
//         app.update();
//         assert!(app.world().get::<Children>(first).is_none());

//         app.world_mut()
//             .resource_mut::<Assets<MujocoSpec>>()
//             .insert(
//                 handle.id(),
//                 MujocoSpec::from_test_xml(
//                     r#"<mujoco><worldbody>
//                     <geom name="floor" type="plane" size="1 1 0.1"/>
//                     <body name="parent"><body name="child">
//                         <geom type="sphere" size="0.1"/>
//                         <geom type="sphere" size="0.2"/>
//                     </body></body>
//                 </worldbody></mujoco>"#,
//                 ),
//             )
//             .unwrap();
//         app.update();

//         let first_world = named_child(app.world(), first, "world");
//         let floor = named_child(app.world(), first_world, "floor");
//         // assert_eq!(
//         //     app.world().get::<MujocoGeometry>(floor).unwrap().mesh_name,
//         //     ""
//         // );
//         let parent = named_child(app.world(), first_world, "parent");
//         let child = named_child(app.world(), parent, "child");
//         let geometries = app.world().get::<Children>(child).unwrap();
//         assert_eq!(geometries.len(), 2);
//         assert_ne!(geometries[0], geometries[1]);
//         for entity in geometries.iter() {
//             assert_eq!(app.world().get::<MujocoGeometry>(entity).unwrap().name, "");
//         }

//         // A new component must initialize even after the asset event was consumed.
//         app.update();
//         let second = app.world_mut().spawn(MujocoSimulation(handle)).id();
//         app.update();
//         let second_world = named_child(app.world(), second, "world");
//         assert_ne!(first_world, second_world);
//         assert_eq!(named_child(app.world(), first, "world"), first_world);
//         assert!(app.world().get::<MujocoSimulationData>(first).is_none());
//         assert!(app.world().get::<MujocoSimulationData>(second).is_none());
//     }

//     #[test]
//     fn reconciles_changes_without_replacing_matching_or_unrelated_entities() {
//         let mut app = app();
//         let handle =
//             app.world_mut()
//                 .resource_mut::<Assets<MujocoSpec>>()
//                 .add(MujocoSpec::from_test_xml(
//                     r#"<mujoco><worldbody>
//                 <body name="keep">
//                     <geom name="visual" type="sphere" size="0.1"/>
//                     <geom type="sphere" size="0.1"/>
//                     <geom type="sphere" size="0.2"/>
//                     <geom name="remove_geom" type="sphere" size="0.1"/>
//                 </body>
//                 <body name="remove"><body name="descendant"/></body>
//             </worldbody></mujoco>"#,
//                 ));
//         let root = app.world_mut().spawn(MujocoSimulation(handle.clone())).id();
//         let unrelated = app
//             .world_mut()
//             .spawn((Name::new("unrelated"), ChildOf(root)))
//             .id();
//         app.update();
//         let world_body = named_child(app.world(), root, "world");
//         let keep = named_child(app.world(), world_body, "keep");
//         let visual = named_child(app.world(), keep, "visual");
//         let unnamed: Vec<_> = app
//             .world()
//             .get::<Children>(keep)
//             .unwrap()
//             .iter()
//             .filter(|&e| {
//                 app.world()
//                     .get::<MujocoGeometry>(e)
//                     .unwrap()
//                     .name
//                     .is_empty()
//             })
//             .collect();
//         let remove_geom = named_child(app.world(), keep, "remove_geom");
//         let remove = named_child(app.world(), world_body, "remove");
//         let descendant = named_child(app.world(), remove, "descendant");
//         let attachment = app.world_mut().spawn(ChildOf(keep)).id();

//         app.world_mut()
//             .resource_mut::<Assets<MujocoSpec>>()
//             .insert(
//                 handle.id(),
//                 MujocoSpec::from_test_xml(
//                     r#"<mujoco>
//                 <asset><mesh name="replacement" vertex="0 0 0 1 0 0 0 1 0 0 0 1"/></asset>
//                 <worldbody><body name="keep">
//                     <geom name="visual" type="mesh" mesh="replacement"/>
//                     <geom type="sphere" size="0.3"/>
//                     <geom type="sphere" size="0.4"/>
//                     <body name="added"/>
//                 </body></worldbody>
//             </mujoco>"#,
//                 ),
//             )
//             .unwrap();
//         app.update();

//         assert_eq!(named_child(app.world(), root, "world"), world_body);
//         assert_eq!(named_child(app.world(), world_body, "keep"), keep);
//         assert_eq!(named_child(app.world(), keep, "visual"), visual);
//         // assert_eq!(
//         //     app.world().get::<MujocoGeometry>(visual).unwrap().mesh_name,
//         //     "replacement"
//         // );
//         assert!(
//             app.world()
//                 .get::<MujocoBody>(named_child(app.world(), keep, "added"))
//                 .is_some()
//         );
//         for entity in unnamed.into_iter().chain([unrelated, attachment]) {
//             assert!(app.world().get_entity(entity).is_ok());
//         }
//         for entity in [remove, descendant, remove_geom] {
//             assert!(app.world().get_entity(entity).is_err());
//         }

//         // Reading the asset for synchronization must not mark it modified again.
//         let changed = app
//             .world()
//             .entity(keep)
//             .get_ref::<MujocoBody>()
//             .unwrap()
//             .last_changed();
//         app.update();
//         assert_eq!(
//             app.world()
//                 .entity(keep)
//                 .get_ref::<MujocoBody>()
//                 .unwrap()
//                 .last_changed(),
//             changed
//         );
//     }
// }
