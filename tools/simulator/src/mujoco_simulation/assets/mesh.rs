use bevy::{
    asset::{AssetPath, RenderAssetUsages},
    mesh::PrimitiveTopology,
    prelude::*,
};
use mujoco_rs::wrappers::{SpecItem, mj_editing::MjsMesh};

use crate::{
    mesh_loaders::MeshLoaderSettings,
    mujoco_simulation::{
        MujocoSimulation, MujocoSimulationData, MujocoSpec,
        asset_loader::resolve_resource_path,
        conversion::MujocoToBevy,
        identification::{MujocoItemId, MujocoItemOf},
        simulation_sync::MujocoSyncData,
    },
};

/// See: https://mujoco.readthedocs.io/en/stable/APIreference/APItypes.html#mjsmesh
#[derive(Component, Reflect, Clone)]
#[component(immutable)]
pub struct MujocoMeshAsset {
    pub data: MujocoMeshData,
    pub meta: MujocoMeshMetaData,
}

#[derive(Reflect, Clone)]
pub enum MujocoMeshData {
    /// Mesh directly loaded from a file. Needs to apply all MuJoCo conversions
    /// that usually happen during compilation.
    File {
        /// Resolved resource path in the spec's Bevy asset source.
        file: AssetPath<'static>,
        content_type: Option<String>,
        /// XML `refpos`, before reference rotation and scaling.
        ref_pos: Vec3,
        /// XML `refquat`; its inverse rotates the raw vertices and normals.
        ref_quat: Quat,
        /// XML `scale`, applied after the reference pose inverse.
        scale: Vec3,
        /// Compiled `mesh_pos`, in the scaled reference frame.
        compiler_pos: Vec3,
        /// Compiled `mesh_quat`, mapping compiled mesh axes to the scaled reference axes.
        compiler_quat: Quat,
    },
    /// Mesh loaded from MuJoCo's compiled model
    User(MujocoUserMeshData),
}

#[derive(Reflect, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MujocoUserMeshData {
    pub vert: Vec<[f32; 3]>,
    pub normal: Vec<[f32; 3]>,
    pub face: Vec<[i32; 3]>,
    /// Per-corner indices into `normal`, not normal vectors.
    pub face_normal: Vec<[i32; 3]>,
    pub uv: Option<MujocoMeshUvData>,
    // user_node: Option<Vec<[f64; 3]>>, Likely not relevant?
    // user_tet: Option<Vec<[i32; 4]>>, Likely not relevant?
}

/// A complete compiled UV set; coordinates and their corner indices coexist.
#[derive(Reflect, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MujocoMeshUvData {
    pub tex_coord: Vec<[f32; 2]>,
    /// Per-corner indices into `tex_coord`.
    pub face_texcoord: Vec<[i32; 3]>,
}

#[derive(Reflect, Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct MujocoMeshMetaData {
    pub smooth_normal: bool,
    pub material: Option<String>,
    //
    // inertia: Not relevant outside of MuJoCo
    // need_sdf: bool, Likely not relevant??
    // max_hull_vert: Option<usize>, Not relevant outside of MuJoCo
    // plugin: Not relevant outside of MuJoCo
    // octree_maxdepth: Option<usize>, Not relevant outside of MuJoCo
}

// impl MujocoMeshMetaData {
//     fn hash(&self) -> u64 {
//         use std::hash::{DefaultHasher, Hash, Hasher};

//         let mut s = DefaultHasher::new();

//         self.smooth_normal.hash(&mut s);
//         self.material.hash(&mut s);

//         s.finish()
//     }
// }

impl MujocoMeshAsset {
    pub(super) fn plugin(app: &mut App) {
        app.register_type::<MujocoMeshAsset>()
            .register_type::<super::MujocoAssetHandle<Mesh>>()
            .register_type::<MujocoMeshData>()
            .register_type::<MujocoMeshMetaData>()
            .register_type::<MujocoUserMeshData>()
            .register_type::<MujocoMeshUvData>()
            .add_observer(Self::extract_mesh)
            .add_observer(Self::build_mesh_asset);
    }

    fn extract_mesh(
        e: On<MujocoSyncData<MjsMesh>>,
        mesh: Query<(&MujocoItemId<MjsMesh>, &MujocoItemOf)>,
        simulation: Query<(&MujocoSimulationData, &MujocoSimulation)>,
        mut spec_assets: ResMut<Assets<MujocoSpec>>,
        mut commands: Commands,
    ) -> Result {
        let (id, &MujocoItemOf(sim_entity)) = mesh.get(e.entity())?;
        let id = **id;

        let (data, sim) = simulation.get(sim_entity)?;

        let model = data.model();

        let mut spec_asset = spec_assets
            .get_mut(&sim.0)
            .ok_or("Simulation spec asset not loaded")?;
        let asset_source = spec_asset.asset_source();
        let spec = spec_asset.spec();
        let spec_mesh = spec
            .mesh_iter()
            .find(|mesh| mesh.id() == Some(id))
            .ok_or("Compiled mesh ID does not exist in spec")?;

        fn mesh_data<T: Clone>(buffer: &[T], address: i32, count: i32) -> Result<Vec<T>> {
            let start = usize::try_from(address)?;
            let count = usize::try_from(count)?;
            let end = start.checked_add(count).ok_or("Mesh data range overflow")?;
            let data = buffer
                .get(start..end)
                .ok_or("Mesh data range out of bounds")?;
            Ok(data.to_vec())
        }

        let mesh = MujocoMeshAsset {
            meta: MujocoMeshMetaData {
                smooth_normal: false, // TODO: I think we dont need smooth_normals, already handled by MuJoCo
                material: None,       // TODO: Handled by MuJoCo
            },
            data: match spec_mesh.file() {
                "" => {
                    // UV presence is per mesh. Without UVs, its address can be
                    // -1 and its face-UV entries are only compiler placeholders.
                    let uv = if model.mesh_texcoordnum()[id] > 0 {
                        Some(MujocoMeshUvData {
                            tex_coord: mesh_data(
                                model.mesh_texcoord(),
                                model.mesh_texcoordadr()[id],
                                model.mesh_texcoordnum()[id],
                            )?,
                            face_texcoord: mesh_data(
                                model.mesh_facetexcoord(),
                                model.mesh_faceadr()[id],
                                model.mesh_facenum()[id],
                            )?,
                        })
                    } else {
                        None
                    };

                    MujocoMeshData::User(MujocoUserMeshData {
                        vert: mesh_data(
                            model.mesh_vert(),
                            model.mesh_vertadr()[id],
                            model.mesh_vertnum()[id],
                        )?,
                        normal: mesh_data(
                            model.mesh_normal(),
                            model.mesh_normaladr()[id],
                            model.mesh_normalnum()[id],
                        )?,
                        face: mesh_data(
                            model.mesh_face(),
                            model.mesh_faceadr()[id],
                            model.mesh_facenum()[id],
                        )?,
                        face_normal: mesh_data(
                            model.mesh_facenormal(),
                            model.mesh_faceadr()[id],
                            model.mesh_facenum()[id],
                        )?,
                        uv,
                    })
                }
                file => MujocoMeshData::File {
                    file: AssetPath::from(resolve_resource_path(
                        spec,
                        spec.compiler().meshdir(),
                        file,
                    ))
                    .with_source(asset_source),
                    content_type: match spec_mesh.content_type() {
                        "" => None,
                        content_type => Some(content_type.to_string()),
                    },
                    ref_pos: spec_mesh.refpos().to_bevy(),
                    ref_quat: spec_mesh.refquat().to_bevy(),
                    scale: spec_mesh.scale().to_bevy(),
                    compiler_pos: model.mesh_pos()[id].to_bevy(),
                    compiler_quat: model.mesh_quat()[id].to_bevy(),
                },
            },
        };

        commands.entity(e.entity()).insert(mesh);

        Ok(())
    }

    #[allow(clippy::complexity)]
    fn build_mesh_asset(
        e: On<Insert<MujocoMeshAsset>>,
        query: Query<(&MujocoMeshAsset, Option<&super::MujocoAssetHandle<Mesh>>)>,
        mut assets: ResMut<Assets<Mesh>>,
        asset_server: Res<AssetServer>,
        mut commands: Commands,
    ) -> Result {
        let (MujocoMeshAsset { data, meta }, handle) = query.get(e.entity).unwrap();

        match data {
            MujocoMeshData::File {
                file,
                content_type,
                ref_pos,
                ref_quat,
                scale,
                compiler_pos,
                compiler_quat,
            } => {
                fn inverse_pose(position: Vec3, rotation: Quat) -> Transform {
                    let inverse_rotation = rotation.normalize().conjugate();
                    Transform {
                        translation: -(inverse_rotation * position),
                        rotation: inverse_rotation,
                        scale: Vec3::ONE,
                    }
                }

                let settings = MeshLoaderSettings {
                    transform: Some(inverse_pose(*ref_pos, *ref_quat)),
                    scale: Some(*scale),
                    post_transform: Some(inverse_pose(*compiler_pos, *compiler_quat)),
                    smooth_normals: Some(meta.smooth_normal),
                };

                //TODO: use content_type if provided. The AssetServer API does not
                //currently support specifying content type.
                let _ = content_type;

                let handle: Handle<Mesh> = asset_server
                    .load_builder()
                    .override_unapproved()
                    .with_settings(move |s: &mut MeshLoaderSettings| {
                        let settings = settings.clone();
                        *s = settings;
                    })
                    .load(file);

                commands
                    .entity(e.entity)
                    .insert(super::MujocoAssetHandle(handle));
            }
            MujocoMeshData::User(data) => {
                let mesh = Self::build_user_mesh(data)?;

                if let Some(existing_handle) = handle
                    && let Some(mut m) = assets.get_mut(&existing_handle.0)
                {
                    *m = mesh;
                } else {
                    let handle = assets.add(mesh);
                    commands
                        .entity(e.entity)
                        .insert(super::MujocoAssetHandle(handle));
                }
            }
        }

        Ok(())
    }

    /// Build geometry from compiled MuJoCo data with independent corner indices.
    /// Faces, including any generated convex hull, are provided by the compiler.
    /// Compiled normals are always supplied; only the complete UV set is optional.
    fn build_user_mesh(data: &MujocoUserMeshData) -> Result<Mesh, BevyError> {
        fn expand<const N: usize>(
            values: &[[f32; N]],
            faces: &[[i32; 3]],
            face_count: usize,
            attribute: &str,
        ) -> Result<Vec<[f32; N]>, BevyError> {
            // Vec lengths and bounds remain runtime invariants, including for
            // data edited through reflection or deserialized outside extraction.
            if faces.len() != face_count {
                return Err(format!("Mesh {attribute} indices must cover every triangle").into());
            }
            faces
                .iter()
                .flatten()
                .map(|&index| {
                    let index = usize::try_from(index)?;
                    values.get(index).copied().ok_or_else(|| {
                        format!("Mesh {attribute} index out of bounds: {index}").into()
                    })
                })
                .collect()
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        let positions = expand(&data.vert, &data.face, data.face.len(), "position")?;

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);

        let corner_normals = expand(&data.normal, &data.face_normal, data.face.len(), "normal")?;
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, corner_normals);

        if let Some(uv) = &data.uv {
            let corner_uvs = expand(&uv.tex_coord, &uv.face_texcoord, data.face.len(), "UV")?;

            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, corner_uvs);
        }

        Ok(mesh)
    }
}
