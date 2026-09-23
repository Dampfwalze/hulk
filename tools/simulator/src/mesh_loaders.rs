use std::io::Cursor;

use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    mesh::Indices,
    prelude::*,
};

pub fn plugin(app: &mut App) {
    app.init_asset::<Mesh>().init_asset_loader::<StlLoader>();
}

#[derive(Reflect, Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
// TODO: Make transform  better and better agnostic to Mujoco
pub struct MeshLoaderSettings {
    /// Optional transform applied to the mesh after loading.
    pub transform: Option<Transform>,

    /// Optional axis scale applied after `transform` and before `post_transform`.
    ///
    /// This is different from `transform.scale` in that `transform.scale` is
    /// applied before rotation, while `scale` is applied after `transform`.
    pub scale: Option<Vec3>,

    /// Optional pose applied after both `transform` and `scale`.
    /// MuJoCo uses this to center and align raw file vertices after XML transforms.
    #[serde(default)]
    pub post_transform: Option<Transform>,

    /// - Some(true): Generate smooth normals.
    /// - Some(false): Generate flat normals.
    /// - None: Use normals from the file. If the file does not contain normals,
    ///   generate flat normals.
    pub smooth_normals: Option<bool>,
}

#[derive(TypePath, Default)]
pub struct StlLoader;

impl MeshLoaderSettings {
    pub(crate) fn apply_to(&self, mut mesh: Mesh) -> Result<Mesh, BevyError> {
        if let Some(transform) = self.transform {
            mesh = mesh.checked_transformed_by(transform)?;
        }
        if let Some(scale) = self.scale {
            mesh = mesh.checked_scaled_by(scale)?;
        }
        if let Some(transform) = self.post_transform {
            mesh = mesh.checked_transformed_by(transform)?;
        }
        Ok(mesh)
    }
}

impl AssetLoader for StlLoader {
    type Asset = Mesh;

    type Settings = MeshLoaderSettings;

    type Error = BevyError;

    fn extensions(&self) -> &[&str] {
        &["stl"]
    }

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await?;

        let stl = stl_io::read_stl(&mut Cursor::new(buf))?;

        let vertex_count = stl.faces.len() * 3;

        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut indices = Vec::with_capacity(vertex_count);

        for (i, face) in stl.faces.iter().enumerate() {
            for j in 0..3 {
                let vertex = stl.vertices[face.vertices[j]];
                positions.push([vertex[0], vertex[1], vertex[2]]);
                normals.push([face.normal[0], face.normal[1], face.normal[2]]);
                indices.push((i * 3 + j) as u32);
            }
        }

        let mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_indices(Indices::U32(indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

        let mesh = settings.apply_to(mesh)?;

        // TODO: Make work
        // if let Some(smooth_normals) = settings.smooth_normals {
        //     if smooth_normals {
        //         mesh = mesh.generate_smooth_normals();
        //     } else {
        //         mesh = mesh.generate_flat_normals();
        //     }
        // }

        Ok(mesh)
    }
}

pub trait MeshTransformationExt: Sized {
    fn checked_transformed_by(self, transform: Transform) -> Result<Self, BevyError>;

    fn checked_scaled_by(self, scale: Vec3) -> Result<Self, BevyError>;
}

impl MeshTransformationExt for Mesh {
    fn checked_transformed_by(self, transform: Transform) -> Result<Self, BevyError> {
        let affine = transform.compute_affine();
        if !affine.is_finite() {
            return Err("mesh transform must be finite".into());
        }

        let determinant = affine.matrix3.determinant();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err("mesh transform must be invertible".into());
        }

        Ok(self.try_transformed_by(transform)?)
    }

    fn checked_scaled_by(self, scale: Vec3) -> Result<Self, BevyError> {
        if !scale.is_finite() {
            return Err("mesh scale must be finite".into());
        }

        if scale.x * scale.y * scale.z < 0.0 {
            warn!(
                "mesh scale contains a reflection; normals and winding order might be incorrect."
            );
        }

        Ok(self.try_scaled_by(scale)?)
    }
}
