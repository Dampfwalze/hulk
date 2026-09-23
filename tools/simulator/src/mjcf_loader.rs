// use std::convert::Infallible;

// use bevy::{
//     asset::{AssetLoader, AsyncReadExt, LoadContext, io::Reader},
//     math::DVec3,
//     prelude::*,
// };
// use mujoco_rs::wrappers::{
//     MjModel, MjSpec, MjVfs, SpecItem,
//     mj_editing::{MjsBody, MjsHfield, MjsMesh, MjsSkin, MjsTexture},
// };

// use crate::{
//     entity_layout::{MjBodyBinding, MjGeomBinding},
//     mujoco_spec::SendSyncSpec,
// };

// pub fn plugin(app: &mut App) {
//     app.init_asset::<MyAsset>()
//         .register_type::<MjServerWorld>()
//         .preregister_asset_loader::<MjcfLoader>(&["xml"])
//         .init_asset_loader::<MjcfLoader>();
// }

// #[derive(TypePath, Default)]
// pub struct MjcfLoader;

// #[derive(Asset, TypePath, Default)]
// pub struct MyAsset;

// impl AssetLoader for MjcfLoader {
//     type Asset = WorldAsset;

//     type Settings = ();

//     type Error = Infallible;

//     fn extensions(&self) -> &[&str] {
//         &["xml"]
//     }

//     async fn load(
//         &self,
//         reader: &mut dyn Reader,
//         settings: &Self::Settings,
//         load_context: &mut LoadContext<'_>,
//     ) -> Result<Self::Asset, Self::Error> {
//         let mut spec_src = String::new();
//         reader
//             .read_to_string(&mut spec_src)
//             .await
//             .expect("TODO: Handle IO error");

//         let mut vfs = MjVfs::new();
//         vfs.add_from_buffer(load_context.path().path(), spec_src.as_bytes())
//             .expect("Should not error");

//         let (deps, spec) = {
//             let spec = MjSpec::from_xml_vfs(load_context.path().path(), &vfs)
//                 .expect("TODO: Handle parse error"); // <model> references should fail here, because VFS does not contain other files yet.

//             let mut deps = Vec::<String>::new();

//             deps.extend(spec.mesh_iter().map(MjsMesh::file).map(ToOwned::to_owned));
//             deps.extend(
//                 spec.hfield_iter()
//                     .map(MjsHfield::file)
//                     .map(ToOwned::to_owned),
//             );
//             deps.extend(spec.skin_iter().map(MjsSkin::file).map(ToOwned::to_owned));
//             deps.extend(
//                 spec.texture_iter()
//                     .map(MjsTexture::file)
//                     .map(ToOwned::to_owned),
//             );

//             for mesh in spec.mesh_iter() {
//                 info!("Mesh: {}", mesh.name());
//             }

//             (deps, spec)
//         };

//         // SAFETY: Never cloned yet (so <model> still allowed), and nothing attached yet
//         let spec = unsafe { spec.into_sendable() };

//         // let mut lc = load_context.begin_labeled_asset();

//         for dep in deps.iter() {
//             info!("Dependency: {dep}");
//         }

//         // Load all dependencies
//         for dep in deps {
//             info!("Loading dependency: {dep}");
//             let dep_bytes = load_context.read_asset_bytes(&dep).await.unwrap();

//             let res = vfs.add_from_buffer(&dep, &dep_bytes);
//             use mujoco_rs::error::MjVfsError::*;
//             match res {
//                 Ok(_) => {}
//                 Err(AlreadyExists) => {
//                     warn!("Dependency {dep} already exists in VFS, skipping");
//                 }
//                 Err(InvalidUtf8Path) => unreachable!("InvalidUtf8Path"),
//                 Err(NotFound) => unreachable!("NotFound"),
//                 Err(e) => {
//                     panic!("Failed to add dependency {dep} to VFS: {e:?}");
//                     // return Err(e.into());
//                 }
//             }
//         }

//         let mut spec = spec.take();

//         for body in spec.body_iter() {
//             info!("Body: {}", body.name());
//         }

//         let world = spec.body("world").expect("world always exists");
//         for b in world.body_iter(false) {
//             info!("sub-Body: {}", b.name());
//         }

//         let (mut world, root_entity) = create_scene(load_context, &spec);

//         let model = spec
//             .compile_with_vfs(&vfs)
//             .expect("TODO: Handle compile error");

//         // world.entity_mut(root_entity).insert(MjServerWorld {
//         //     // SAFETY: Never cloned yet (so <model> still allowed), and nothing attached yet
//         //     spec: unsafe { SendSyncSpec::new(spec) },
//         //     model,
//         // });

//         // load_context.add_loaded_labeled_asset("Spec", lc.finish(spec));

//         // let mut model = spec.compile().expect("TODO: Handle compile error");
//         // let x = model.mesh("name").unwrap().view_mut(&mut model).;

//         Ok(WorldAsset::new(world))
//     }
// }

// #[derive(Component, Reflect, Default, Debug, PartialEq, Eq)]
// #[reflect(Component)]
// #[relationship_target(relationship = MjChildOf, linked_spawn)]
// pub struct MjChildren(Vec<Entity>);

// #[derive(Component, Reflect, FromTemplate, Clone, PartialEq, Eq, Debug)]
// #[reflect(Component)]
// #[relationship(relationship_target = MjChildren)]
// pub struct MjChildOf(pub Entity);

// #[derive(Component, Reflect, Debug)]
// #[reflect(from_reflect = false)]
// #[reflect(Component)]
// pub struct MjServerWorld {
//     #[reflect(ignore)]
//     spec: SendSyncSpec,
//     #[reflect(ignore)]
//     model: MjModel,
// }

// fn create_scene(load_context: &mut LoadContext<'_>, spec: &MjSpec) -> (World, Entity) {
//     let mut world = World::new();

//     let root_entity = world.spawn((Name::new("World"), Transform::default())).id();

//     let world_body = spec.body("world").expect("'world' body always exists");

//     fn recurse(
//         body: &MjsBody,
//         world: &mut World,
//         spec: &MjSpec,
//         lc: &mut LoadContext<'_>,
//         parent: Option<Entity>,
//     ) {
//         for body in body.body_iter(false) {
//             let pos = body.pos().to_bevy();

//             let entity = world
//                 .spawn((
//                     MjBodyBinding,
//                     Name::new(body.name().to_owned()),
//                     Transform::from_translation(pos),
//                 ))
//                 .with_children(|spawner| {
//                     for geom in body.geom_iter(false) {
//                         info!("Mesh name: {}", geom.meshname());
//                         let Some(mesh) = spec.mesh(geom.meshname()) else {
//                             continue;
//                         };

//                         let mesh_file = mesh.file().to_owned();

//                         spawner.spawn((
//                             Name::new(geom.name().to_owned()),
//                             MjGeomBinding { mesh: mesh_file },
//                         ));
//                     }
//                 })
//                 .id();

//             if let Some(parent) = parent {
//                 world.entity_mut(entity).insert(ChildOf(parent));
//             }

//             recurse(body, world, spec, lc, Some(entity));
//         }
//     }

//     recurse(
//         world_body,
//         &mut world,
//         spec,
//         load_context,
//         Some(root_entity),
//     );

//     (world, root_entity)
// }
