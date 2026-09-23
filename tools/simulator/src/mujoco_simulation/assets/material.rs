use bevy::prelude::*;
use mujoco_rs::wrappers::mj_editing::MjsMaterial;

use crate::mujoco_simulation::{
    MujocoSimulationData,
    conversion::MujocoToBevy,
    identification::{MujocoItemId, MujocoItemOf},
    simulation_sync::MujocoSyncData,
};

/// See: https://mujoco.readthedocs.io/en/stable/APIreference/APItypes.html#mjsmaterial
#[derive(Component, Reflect)]
#[component(immutable)]
pub struct MujocoMaterialAsset {
    textures: Vec<String>, // TODO: Implement
    texuniform: bool,
    texrepeat: [f32; 2],
    /// Scalar multiplier for material RGB to obtain legacy emission RGB.
    emission: f32,
    /// Legacy white specular intensity; not a physical Fresnel reflectance.
    specular: f32,
    /// Legacy highlight exponent divided by 128, not PBR roughness.
    shininess: f32,
    /// Planar mirror strength for MuJoCo's reflective plane/box rendering pass.
    /// This is unrelated to `StandardMaterial::reflectance`.
    reflectance: f32,
    /// PBR coefficient, or `None` when MuJoCo stores a negative sentinel.
    metallic: Option<f32>,
    /// PBR coefficient, or `None` when MuJoCo stores a negative sentinel.
    roughness: Option<f32>,
    /// Material color and opacity, interpreted as linear for the Bevy renderer.
    rgba: LinearRgba,
}

impl MujocoMaterialAsset {
    pub(super) fn plugin(app: &mut App) {
        app.register_type::<MujocoMaterialAsset>()
            .register_type::<super::MujocoAssetHandle<StandardMaterial>>()
            .add_observer(Self::extract_material)
            .add_observer(Self::build_material_asset);
    }

    fn extract_material(
        e: On<MujocoSyncData<MjsMaterial>>,
        materials: Query<(&MujocoItemId<MjsMaterial>, &MujocoItemOf)>,
        simulation: Query<&MujocoSimulationData>,
        mut commands: Commands,
    ) -> Result {
        let (id, &MujocoItemOf(sim_entity)) = materials.get(e.entity())?;
        let id = **id;
        let data = simulation.get(sim_entity)?;
        let model = data.model();

        let material = MujocoMaterialAsset {
            textures: vec![], // TODO: Get textures
            texuniform: model.mat_texuniform()[id],
            texrepeat: model.mat_texrepeat()[id],
            emission: model.mat_emission()[id],
            specular: model.mat_specular()[id],
            shininess: model.mat_shininess()[id],
            reflectance: model.mat_reflectance()[id],
            metallic: (model.mat_metallic()[id] >= 0.0).then_some(model.mat_metallic()[id]),
            roughness: (model.mat_roughness()[id] >= 0.0).then_some(model.mat_roughness()[id]),
            rgba: model.mat_rgba()[id].to_bevy(),
        };

        commands.entity(e.entity()).insert(material);

        Ok(())
    }

    fn build_material_asset(
        e: On<Insert<MujocoMaterialAsset>>,
        query: Query<(
            &MujocoMaterialAsset,
            Option<&super::MujocoAssetHandle<StandardMaterial>>,
        )>,
        mut assets: ResMut<Assets<StandardMaterial>>,
        mut commands: Commands,
    ) {
        let (material, maybe_handle) = query.get(e.entity).unwrap();

        if let Some(mut m) = maybe_handle.and_then(|handle| assets.get_mut(&handle.0)) {
            material.apply_bevy_approximation(&mut m);
        } else {
            let mut m = StandardMaterial::default();
            material.apply_bevy_approximation(&mut m);
            let handle = assets.add(m);
            commands
                .entity(e.entity)
                .insert(super::MujocoAssetHandle(handle));
        }
    }

    /// Apply the simulator's uniform-color approximation using Bevy's PBR shader.
    /// This is an appearance policy, not an equivalent MuJoCo shading model.
    ///
    /// (AI Slop)
    fn apply_bevy_approximation(&self, m: &mut StandardMaterial) {
        let [r, g, b, a] = self.rgba.to_f32_array();

        m.base_color = self.rgba.into();
        m.alpha_mode = if a < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };

        m.emissive = LinearRgba::from_f32_array([
            r * self.emission,
            g * self.emission,
            b * self.emission,
            1.0,
        ]);
        m.emissive_exposure_weight = 0.0;

        m.specular_transmission = 0.0;
        m.anisotropy_strength = 0.0;

        // Deliberate highlight-control mapping; MuJoCo's planar reflectance is unsupported.
        m.reflectance = self.specular.clamp(0.0, 1.0);

        m.metallic = self.metallic.unwrap_or(0.0).clamp(0.0, 1.0);

        m.perceptual_roughness = if let Some(roughness) = self.roughness {
            roughness.clamp(0.089, 1.0)
        } else {
            // Approximate the legacy highlight exponent with GGX perceptual roughness.
            let exponent = 128.0 * self.shininess.clamp(0.0, 1.0);
            (2.0 / (exponent + 2.0)).powf(0.25).clamp(0.089, 1.0)
        };
    }
}
