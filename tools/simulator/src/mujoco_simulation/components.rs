use std::{ops, sync::Arc};

use bevy::{asset::Handle, prelude::*};
use mujoco_rs::{
    prelude::*,
    wrappers::traits::{ModelType, ModelTypeMut},
};

use crate::mujoco_simulation::identification::MujocoItemIndex;

use super::asset_loader::MujocoSpec;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<MujocoSimulation>()
        .register_type::<MujocoSimulationData>()
        .register_type::<ModelHandle>();
}

#[derive(Component, Reflect, Deref, DerefMut)]
#[reflect(Component, from_reflect = false)]
#[require(Transform, Visibility, MujocoItemIndex)]
pub struct MujocoSimulation(pub Handle<MujocoSpec>);

impl bevy::asset::AsAssetId for MujocoSimulation {
    type Asset = MujocoSpec;

    fn as_asset_id(&self) -> bevy::asset::AssetId<MujocoSpec> {
        self.0.id()
    }
}

#[derive(Component, Reflect, Deref, DerefMut)]
#[reflect(Component, from_reflect = false)]
pub struct MujocoSimulationData {
    #[reflect(ignore)]
    #[deref]
    pub data: MjData<ModelHandle>,
    pub revision: usize,
}

/// A wrapper around `Arc<MjModel>` that also implements ModelTypeMut, allowing
/// mutable access to the model if the Arc is uniquely owned.
///
/// # Panics
/// Panics if mutable access is attempted while the Arc is owned by multiple
/// references. Uses [Arc::get_mut] internally.
#[derive(Clone, Debug, Reflect)]
#[reflect(opaque)]
pub struct ModelHandle(Arc<MjModel>);

impl ModelHandle {
    pub fn new(model: MjModel) -> Self {
        Self(Arc::new(model))
    }
}

impl ops::Deref for ModelHandle {
    type Target = MjModel;

    fn deref(&self) -> &MjModel {
        &self.0
    }
}

impl ops::DerefMut for ModelHandle {
    fn deref_mut(&mut self) -> &mut MjModel {
        Arc::get_mut(&mut self.0).expect("mutable model access requires exclusive ownership")
    }
}

// SAFETY: Each handle refers to one fixed, live model.
unsafe impl ModelType for ModelHandle {}

// SAFETY: Successful mutable access refers to that same model,
// and Arc::get_mut guarantees exclusive access.
unsafe impl ModelTypeMut for ModelHandle {}

// #[derive(Component, Reflect)]
// #[reflect(Component)]
// #[require(Transform, Visibility)]
// pub struct MujocoBody {
//     pub name: String,
// }

// #[derive(Component, Reflect, Clone)]
// #[reflect(Component)]
// #[require(Transform, Visibility)]
// #[component(immutable)]
// pub struct MujocoGeometry {
//     pub name: String,
//     pub type_: MujocoGeometryType,
//     pub material: MujocoMaterial,
// }

// #[derive(Reflect, Clone)]
// pub enum MujocoGeometryType {
//     Plane,
//     HeightField(String),
//     Sphere,
//     Capsule,
//     Ellipsoid,
//     Cylinder,
//     Box,
//     Mesh(String),
//     // Sdf,

//     // non-regular geom types not used. See https://mujoco.readthedocs.io/en/stable/APIreference/APItypes.html#mjtgeom
// }

// // impl MujocoGeometry {
// //     pub fn from_geom(geom: &MjGeom) -> Self
// // }

// #[derive(Reflect, Clone)]
// pub enum MujocoMaterial {
//     Rgba(LinearRgba),
//     Material(String),
// }

// #[derive(Component, Reflect)]
// #[reflect(Component)]
// pub struct MujocoId(usize);

// pub struct MujocoMaterials {}

// fn on_geometry_inserted(
//     e: On<Insert, MujocoGeometry>,
//     query: Query<&MujocoGeometry>,
//     assets: Res<AssetServer>,
//     mut commands: Commands,
//     mut materials: ResMut<Assets<StandardMaterial>>,
//     mut meshes: ResMut<Assets<Mesh>>,
// ) {
//     let geom = query
//         .get(e.entity)
//         .expect("Entity should exist and have MujocoGeometry component");

//     // let mesh = match &geom.type_ {
//     //     MujocoGeometryType::Mesh(mesh_file) => assets.load(mesh_file),
//     //     MujocoGeometryType::Box => {
//     //         meshes.get_or_insert_with(|| Mesh::new_box(Vec3::splat(1.0), Vec3::splat(1.0)))
//     //     }
//     // };

//     // commands.entity(e.entity).insert((
//     //     Mesh3d(assets.load(&geom.mesh_name)),
//     //     MeshMaterial3d(materials.add(StandardMaterial {
//     //         base_color: Color::WHITE,
//     //         ..default()
//     //     })),
//     // ));
// }

// // const CUBOID_ID: AssetId<Mesh> = AssetId::new("bevy", "mesh/cuboid");
