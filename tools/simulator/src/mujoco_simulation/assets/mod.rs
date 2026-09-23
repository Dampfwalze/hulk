use bevy::prelude::*;

pub mod material;
pub mod mesh;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        material::MujocoMaterialAsset::plugin,
        mesh::MujocoMeshAsset::plugin,
    ));
}

#[derive(Component, Reflect)]
pub struct MujocoAssetHandle<A: Asset>(pub Handle<A>);
