use bevy::prelude::*;
use mujoco_rs::wrappers::{
    MjtGeom,
    mj_editing::{MjsGeom, MjsHfield, MjsMaterial, MjsMesh},
};

use crate::mujoco_simulation::{
    MujocoSimulationData,
    assets::MujocoAssetHandle,
    conversion::MujocoToBevy,
    identification::{MujocoItemId, MujocoItemIndex, MujocoItemName, MujocoItemOf, Required},
    simulation_sync::MujocoSyncData,
};

#[derive(Component, Reflect, Debug, PartialEq)]
pub enum MujocoGeomType {
    Plane {
        /// Local X/Y half-extents. Zero means infinite along that axis.
        half_extents: Vec2,
        /// Spacing of the rendering subdivisions, not a thickness.
        grid_spacing: f32,
    },
    HeightField(Entity),
    Sphere {
        radius: f32,
    },
    Capsule {
        radius: f32,
        /// Half-length of the cylinder part along local Z, excluding the caps.
        half_length: f32,
    },
    Ellipsoid {
        /// Radii along local X/Y/Z.
        radii: Vec3,
    },
    Cylinder {
        radius: f32,
        /// Half-length along local Z.
        half_length: f32,
    },
    Box {
        /// Half-extents along local X/Y/Z.
        half_extents: Vec3,
    },
    Mesh(Entity),
    Sdf,
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(MujocoItemName<MjsGeom> = Required, Transform, Visibility)]
#[component(immutable)]
pub struct MujocoGeom {
    type_: MujocoGeomType,
    material: Option<Entity>,
    rgba: LinearRgba,
}

impl MujocoGeom {
    pub(super) fn plugin(app: &mut App) {
        app.register_type::<Self>()
            .add_observer(Self::extract_geom)
            .add_observer(Self::build_geom_entity);
    }

    fn extract_geom(
        e: On<MujocoSyncData<MjsGeom>>,
        geom: Query<(&MujocoItemId<MjsGeom>, &MujocoItemOf)>,
        simulation: Query<(&MujocoSimulationData, &MujocoItemIndex)>,
        mut commands: Commands,
    ) -> Result {
        let (id, &MujocoItemOf(sim_entity)) = geom.get(e.entity())?;
        let id = **id;

        let (data, index) = simulation.get(sim_entity)?;

        let m = data.model();
        let size: Vec3 = m.geom_size()[id].to_bevy();

        commands.entity(e.entity()).insert((
            MujocoGeom {
                type_: match m.geom_type()[id] {
                    MjtGeom::mjGEOM_PLANE => MujocoGeomType::Plane {
                        half_extents: size.truncate(),
                        grid_spacing: size.z,
                    },
                    MjtGeom::mjGEOM_HFIELD => MujocoGeomType::HeightField(
                        if let dataid = m.geom_dataid()[id]
                            && dataid >= 0
                        {
                            index
                                .get_by_id::<MjsHfield>(dataid as usize)
                                .ok_or("No entity found for heightfield")?
                        } else {
                            return Err("mjGEOM_HFIELD should have valid dataid".into());
                        },
                    ),
                    MjtGeom::mjGEOM_SPHERE => MujocoGeomType::Sphere { radius: size.x },
                    MjtGeom::mjGEOM_CAPSULE => MujocoGeomType::Capsule {
                        radius: size.x,
                        half_length: size.y,
                    },
                    MjtGeom::mjGEOM_ELLIPSOID => MujocoGeomType::Ellipsoid { radii: size },
                    MjtGeom::mjGEOM_CYLINDER => MujocoGeomType::Cylinder {
                        radius: size.x,
                        half_length: size.y,
                    },
                    MjtGeom::mjGEOM_BOX => MujocoGeomType::Box { half_extents: size },
                    MjtGeom::mjGEOM_MESH => MujocoGeomType::Mesh(
                        if let dataid = m.geom_dataid()[id]
                            && dataid >= 0
                        {
                            index
                                .get_by_id::<MjsMesh>(dataid as usize)
                                .ok_or("No entity found for mesh")?
                        } else {
                            return Err("mjGEOM_MESH should have valid dataid".into());
                        },
                    ),
                    MjtGeom::mjGEOM_SDF => MujocoGeomType::Sdf,
                    _ => return Err("Geom has unknown type".into()),
                },
                material: if let matid = m.geom_matid()[id]
                    && matid >= 0
                {
                    Some(
                        index
                            .get_by_id::<MjsMaterial>(matid as usize)
                            .ok_or("No entity found for material")?,
                    )
                } else {
                    None
                },
                rgba: m.geom_rgba()[id].to_bevy(),
            },
            Transform {
                translation: m.geom_pos()[id].to_bevy(),
                rotation: m.geom_quat()[id].to_bevy(),
                scale: Vec3::ONE,
            },
        ));

        Ok(())
    }

    fn build_geom_entity(
        e: On<Insert<MujocoGeom>>,
        query: Query<&MujocoGeom>,
        mesh_assets: Query<&MujocoAssetHandle<Mesh>>,
        material_assets: Query<&MujocoAssetHandle<StandardMaterial>>,
        mut m_assets: ResMut<Assets<StandardMaterial>>,
        mut commands: Commands,
    ) -> Result {
        let geom = query.get(e.entity)?;

        let mut entity_commands = commands.entity(e.entity);

        match &geom.type_ {
            MujocoGeomType::Plane { .. }
            | MujocoGeomType::Box { .. }
            | MujocoGeomType::Sphere { .. }
            | MujocoGeomType::Capsule { .. }
            | MujocoGeomType::Ellipsoid { .. }
            | MujocoGeomType::Cylinder { .. }
            | MujocoGeomType::HeightField(_)
            | MujocoGeomType::Sdf => {
                warn!("TODO: Build primitive geom entity: {:?}", geom.type_);
            }
            MujocoGeomType::Mesh(asset_entity) => {
                let handle = mesh_assets.get(*asset_entity)?;
                entity_commands.insert(Mesh3d(handle.0.clone()));
            }
        }

        if let Some(material_entity) = geom.material {
            let handle = material_assets.get(material_entity)?;

            entity_commands.insert(MujocoAssetHandle::<StandardMaterial>(handle.0.clone()));
        } else {
            warn!("TODO: Use RGBA instead: {:?}", geom.rgba);

            entity_commands.insert(MeshMaterial3d(m_assets.add(StandardMaterial {
                base_color: geom.rgba.into(),
                ..Default::default()
            })));
        }

        Ok(())
    }
}
